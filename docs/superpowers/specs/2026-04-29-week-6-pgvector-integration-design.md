# Week 6 — pgvector Integration Design

**Date**: 2026-04-29
**Status**: approved (brainstorm)
**Predecessor**: Week 5 (`2026-04-29-week-5-explain-visualization-design.md`)

---

## 1. Goal

Make Tusk visibly the best Postgres client for pgvector users by treating vectors as first-class:

- Detect vector columns automatically per connection.
- Show dimension + index-presence signal directly in the schema tree.
- Render vector cells with a sparkline + L2 norm + raw modal.
- Right-click cell → "Find similar rows" with operator/limit picker, runs ANN query in a new tab.
- Right-click column → "Visualize (UMAP)" — new tab with 2D scatter, parameter controls, click-to-inspect row.
- Right-click table → "Vector indexes" panel with parameter readout + Create index helper.

### Success criteria

1. Connecting to a database with pgvector + a `vector` column shows a `vec(N)` badge in the schema tree within ≤500 ms after connect.
2. Connecting without pgvector loads the schema tree as before; no error toast, no broken UI.
3. Right-clicking a vector cell and choosing "Find similar rows" opens a modal with a syntactically correct ANN SQL preview matching the chosen operator + LIMIT.
4. Hitting "Run" in the modal opens a new editor tab pre-loaded with the ANN SQL and auto-executes it.
5. Right-clicking a vector column and choosing "Visualize (UMAP)" opens a new tab. The 2D scatter renders within 60 s for ≤10 000 vectors of 1536 dim on a typical laptop.
6. Clicking a UMAP point shows the full row in a side panel.
7. Right-clicking a table and choosing "Vector indexes" lists every HNSW/IVFFlat index on that table with parsed `m`, `ef_construction`, `lists`, size.
8. The "Create index" helper inside the panel produces a valid `CREATE INDEX ... USING hnsw (col vector_cosine_ops) WITH (m=16, ef_construction=64);` and inserts it into the active editor tab.
9. All existing Week 1–5 features continue to work; the schema tree column rendering does not regress for non-vector columns.
10. Manual verification checklist (`docs/superpowers/plans/manual-verification-week-6.md`) passes end-to-end on a fresh checkout against the docker-compose Postgres with pgvector enabled.

---

## 2. Architecture

Backend exposes three introspection-only commands; ANN SQL building, UMAP execution, and all UI happen in the frontend. This mirrors the Week 5 split (Rust = trusted introspection, frontend = interactive composition).

```
Rust (#[tauri::command]):
  list_vector_columns(connection_id) -> Vec<VectorColumn>
  list_vector_indexes(connection_id, schema, table) -> Vec<VectorIndex>
  sample_vectors(connection_id, schema, table, vec_col, pk_cols, limit) -> SampledVectors

Frontend (TS):
  zustand store useVectorMeta — per-connection cache of VectorColumn[]
  lib/vector/annSql.ts — pure SQL builder
  lib/vector/cellRender.ts — sparkline + L2 + raw modal hook
  lib/vector/umapWorker.ts — Web Worker wrapping umap-js
  features/sidebar/SchemaTree.tsx — vector badges + ⚠ + context menus (modify)
  features/results/ResultsGrid.tsx — vector cell renderer (modify)
  features/vector/FindSimilarModal.tsx
  features/vector/VectorIndexPanel.tsx
  features/vector/UmapTab.tsx + UmapScatter.tsx + UmapControls.tsx
  store/tabs.ts — add 'umap' tab kind (modify)
```

New npm dependency: `umap-js` (pure JS, ≈50 kB).
No new Rust crates.

---

## 3. Components

### 3.1 Rust

#### `commands/vector.rs`

```rust
#[tauri::command]
pub async fn list_vector_columns(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
) -> TuskResult<Vec<VectorColumn>>;

#[tauri::command]
pub async fn list_vector_indexes(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
    schema: String,
    table: String,
) -> TuskResult<Vec<VectorIndex>>;

#[tauri::command]
pub async fn sample_vectors(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
    schema: String,
    table: String,
    vec_col: String,
    pk_cols: Vec<String>,
    limit: u32,
) -> TuskResult<SampledVectors>;
```

Return shapes (serde-derived):

```rust
#[derive(Serialize)]
pub struct VectorColumn {
    pub schema: String,
    pub table: String,
    pub column: String,
    pub dim: i32,        // -1 if pgvector typmod missing (rare)
    pub has_index: bool, // any HNSW/IVFFlat referencing this column
}

#[derive(Serialize)]
pub struct VectorIndex {
    pub name: String,
    pub schema: String,
    pub table: String,
    pub column: String,
    pub method: String,            // "hnsw" | "ivfflat"
    pub params: VectorIndexParams, // parsed reloptions
    pub size_bytes: i64,
    pub definition: String,        // pg_get_indexdef
}

#[derive(Serialize, Default)]
pub struct VectorIndexParams {
    pub m: Option<i32>,
    pub ef_construction: Option<i32>,
    pub lists: Option<i32>,
    pub ops: Option<String>, // "vector_cosine_ops" etc.
}

#[derive(Serialize)]
pub struct SampledVectors {
    pub rows: Vec<SampledVectorRow>,
    pub total_rows: i64, // pg_class.reltuples estimate (i64 cast)
}

#[derive(Serialize)]
pub struct SampledVectorRow {
    pub pk_json: serde_json::Value, // object: {pk_col: value}
    pub vec: Vec<f32>,
}
```

#### `db/vector_introspect.rs`

SQL helpers used by `commands/vector.rs`.

- `vector_columns_query()` — joins `pg_attribute` + `pg_type` (`typname = 'vector'`) + `pg_class` + `pg_namespace`; uses `format_type(atttypid, atttypmod)` to get `vector(N)`, regex `vector\((\d+)\)` for dim. `has_index` via LATERAL exists subquery against `pg_index`+`pg_am`.
- `vector_indexes_query(schema, table)` — joins `pg_index` + `pg_class i` (the index) + `pg_class t` (the table) + `pg_am`. `pg_get_indexdef(i.oid)` for definition. `pg_relation_size(i.oid)` for size. `pg_class.reloptions` (string array like `{m=16,ef_construction=64}`) parsed by Rust into `VectorIndexParams`.
- `sample_vectors_sql(schema, table, vec_col, pk_cols, limit)` — generates `SELECT <pk_cols>, <vec_col> FROM <schema>.<table> ORDER BY random() LIMIT $1` with proper identifier quoting. Caller binds limit via `$1`.

All identifiers escaped via existing `quote_ident` (or new helper if absent — in that case, add to `db/sql_util.rs` and unit-test it).

#### Statement timeout

`sample_vectors` uses the same connection pool as other queries. Apply `SET LOCAL statement_timeout = '30s'` inside a transaction so a slow large-table sample fails predictably.

### 3.2 Frontend

#### `lib/vector/types.ts`

```ts
export interface VectorColumn {
  schema: string;
  table: string;
  column: string;
  dim: number;
  hasIndex: boolean;
}

export interface VectorIndexParams {
  m?: number;
  efConstruction?: number;
  lists?: number;
  ops?: string;
}

export interface VectorIndex {
  name: string;
  schema: string;
  table: string;
  column: string;
  method: "hnsw" | "ivfflat";
  params: VectorIndexParams;
  sizeBytes: number;
  definition: string;
}

export interface SampledVectorRow {
  pkJson: Record<string, unknown>;
  vec: number[];
}

export interface SampledVectors {
  rows: SampledVectorRow[];
  totalRows: number;
}

export type AnnOperator = "<=>" | "<->" | "<#>";
```

#### `lib/vector/annSql.ts`

```ts
export function buildAnnSql(args: {
  schema: string;
  table: string;
  vecCol: string;
  pkCols: string[];
  queryVector: number[];
  op: AnnOperator;
  limit: number;
}): string;
```

Pure function. Result form:

```sql
SELECT <pk_cols_csv>,
       <vec_col> <op> '[v1,v2,...]'::vector AS distance,
       *
FROM <schema>.<table>
ORDER BY distance
LIMIT <limit>;
```

`limit` clamped to `Math.max(1, Math.min(10_000, Math.floor(limit)))`. Identifiers quoted via local `escIdent()` helper (mirrors the one in `IndexCandidates.tsx`).

#### `lib/vector/cellRender.ts`

- `l2Norm(v: number[]): number`
- `renderSparkline(canvas: HTMLCanvasElement, v: number[]): void` — first 32 dims, normalized to [0,1] inside the canvas height, single-color polyline.
- `formatVectorSummary(v: number[]): string` → `"[1536d, ‖v‖=0.892]"`.

#### `store/useVectorMeta.ts`

```ts
interface VectorMetaState {
  byConn: Record<string, VectorColumn[]>;
  loading: Record<string, boolean>;
  refresh(connId: string): Promise<void>;
  hasVectorAt(
    connId: string,
    schema: string,
    table: string,
    column: string,
  ): VectorColumn | null;
}
```

Called once on connect (hook into existing `useConnections.connect` success path) and on manual refresh button.

#### `features/sidebar/SchemaTree.tsx` (modify)

- Column row: if `useVectorMeta.hasVectorAt(...)`, append `vec(N)` badge. If `!hasIndex`, append `⚠` with tooltip "No HNSW/IVFFlat index — sequential scan only."
- Column row context menu: "Visualize (UMAP)" (only when vector). Existing menu items preserved.
- Table row context menu: "Vector indexes" (only when the table has at least one vector column).

#### `features/results/ResultsGrid.tsx` (modify)

- Cell renderer: if column type is `vector`, mount sparkline canvas + summary text.
- Cell hover: tooltip with full summary.
- Cell double-click: existing edit modal flow short-circuits to a read-only "Vector value" modal (raw `[...]` + Copy + close).
- Cell context menu: "Find similar rows" (only for vector cells with non-null value and the row carrying detectable PK columns — reuse existing PK detection logic from Week 3).

#### `features/vector/FindSimilarModal.tsx`

Props: `{ open, onClose, schema, table, vecCol, pkCols, queryVector, connId }`.
UI:

- Operator dropdown (`<=> cosine`, `<-> L2`, `<#> inner product`)
- LIMIT input (default 20, range 1–10 000)
- SQL preview (`buildAnnSql` output in a Monaco read-only block or simple `<pre>`)
- Run button — `useTabs.newTab(connId)` → `updateSql(newId, sql)` → `setActive(newId)` → fire existing `run` action via a small helper that takes a tab id.
- Cancel button — close modal.

#### `features/vector/VectorIndexPanel.tsx`

Mounted as a side panel triggered by the table context menu. Calls `list_vector_indexes(connId, schema, table)`. Renders a table:

| Name | Column | Method | m / ef_construction / lists / ops | Size | Definition |

Below the table: a "Create index" form (collapsed by default):

- Column dropdown (vector columns of this table)
- Method dropdown (HNSW / IVFFlat)
- Params (HNSW: m=16, ef_construction=64; IVFFlat: lists=100)
- Operator class dropdown (`vector_cosine_ops`, `vector_l2_ops`, `vector_ip_ops`) — default cosine
- "Insert into editor" button (no auto-run; index builds are expensive)

#### `features/vector/UmapTab.tsx`

Tab kind: extend existing `Tab` type with discriminator. Either:

- Add `kind: 'sql' | 'umap'` and existing fields move to `kind === 'sql'`, OR
- Add an optional `umap?: UmapTabState` and route in `EditorPane`/main content area.

Decision: **add `kind` discriminator** for cleanliness. Implementation in plan.

`UmapTab` props: `{ tabId, connId, schema, table, vecCol, pkCols, dim }`.

State (in `Tab.umap`):

```ts
interface UmapTabState {
  sample: number; // default 10000
  nNeighbors: number; // default 15
  minDist: number; // default 0.1
  status: "idle" | "sampling" | "computing" | "ready" | "error";
  progress: number; // 0..1 during computing
  error?: string;
  points?: { x: number; y: number; pkJson: Record<string, unknown> }[];
  selectedIdx?: number;
}
```

Layout: left controls panel, center canvas, right row-detail panel (lazy fetched on point click).

#### `features/vector/UmapScatter.tsx`

`<canvas>` with imperative drawing. Wheel = zoom around cursor; drag = pan. Hover = pixel-pick nearest point (kd-tree or spatial hash, but simple linear scan is fine up to 10k). Click = `onSelect(idx)`. Resize via `ResizeObserver`.

#### `features/vector/UmapControls.tsx`

Sliders: n_neighbors (2–100), min_dist (0.0–0.99), sample size (100–50 000 input). "Re-run" button, disabled during computing.

#### `lib/vector/umapWorker.ts` + `umapWorker.entry.ts`

Vite worker (`new Worker(new URL("./umapWorker.entry.ts", import.meta.url), { type: "module" })`).

Worker contract:

```ts
// in
{ kind: "run", vecs: Float32Array, dim: number, count: number, nNeighbors: number, minDist: number }
// out (multi)
{ kind: "progress", value: number }     // 0..1
{ kind: "done", coords: Float32Array }  // [x0, y0, x1, y1, ...]
{ kind: "error", message: string }
```

Worker imports `umap-js`, builds a `UMAP` instance, runs `fitAsync` (it supports a per-epoch callback for progress), posts coords back as a transferable `Float32Array.buffer`.

### 3.3 Schema tree integration

`SchemaTree` is the only existing file we materially modify. Vector badges and ⚠ are pure additions. Context menu items are pushed into the existing menu component conditionally. We do NOT refactor SchemaTree; the new behavior reads from `useVectorMeta`.

---

## 4. Data flow

### 4.1 Connect → vector meta cache

```
useConnections.connect(connId) success
  → useVectorMeta.refresh(connId)
    → invoke('list_vector_columns', { connectionId: connId })
    → setState({ byConn: { [connId]: [...] } })
```

Failure (pgvector not installed): Rust returns `Vec<VectorColumn>` empty (the JOIN against `pg_type WHERE typname='vector'` yields zero rows). Frontend stores `[]`. No UI noise.

### 4.2 Find similar

```
ResultsGrid cell context menu → "Find similar rows"
  → open FindSimilarModal with extracted queryVector + PK + table coords
  → user picks operator + limit
  → buildAnnSql → preview
  → Run → useTabs.newTab(connId) + updateSql(newId, sql) + setActive(newId) + runOnTab(newId)
```

`runOnTab(newId)` wraps the existing `run` callback so it works against an arbitrary tab id (the current one is hardcoded to `activeTab`). Either pass `tabId` through or temporarily set active before invoking — implementation in plan.

### 4.3 UMAP

```
SchemaTree column context menu → "Visualize (UMAP)"
  → useTabs.newUmapTab({ connId, schema, table, vecCol, pkCols, dim })
  → UmapTab mounts, status = "sampling"
  → invoke('sample_vectors', { ... limit: sample })
  → on result: pack vecs into Float32Array, post to umapWorker
  → status = "computing", progress streams
  → on "done": coords stored on tab.umap.points
  → UmapScatter draws

User clicks point:
  → onSelect(idx)
  → invoke('execute_query', { sql: SELECT * FROM <table> WHERE <pk> = <values> LIMIT 1 })
  → side panel renders the row
```

PK lookup uses the same identifier escaping as `buildAnnSql`. Composite PK = `WHERE k1 = $1 AND k2 = $2`.

### 4.4 Vector index panel

```
SchemaTree table context menu → "Vector indexes"
  → open VectorIndexPanel(connId, schema, table)
  → invoke('list_vector_indexes', ...)
  → render table

Create index form:
  → user picks column/method/params/ops
  → preview SQL
  → "Insert into editor" → updateSql(activeTab, current + "\n" + sql)
```

### 4.5 Error handling

| Site                                | Failure                                                                                                             | UI                                            |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------- | --------------------------------------------- |
| `list_vector_columns` errors        | network/pool dead                                                                                                   | toast; keep cached `[]`; no badges shown      |
| pgvector not installed              | empty result                                                                                                        | silent — vector entry points naturally absent |
| `sample_vectors` timeout            | toast "Sampling timed out (30s) — try a smaller sample size"; UMAP tab status="error"                               |
| `sample_vectors` other error        | toast; UMAP tab status="error" with retry button                                                                    |
| `umapWorker` error                  | UMAP tab status="error"; retry button                                                                               |
| `list_vector_indexes` errors        | toast; panel shows empty + retry button                                                                             |
| `buildAnnSql` invalid input (no PK) | "Find similar rows" menu item disabled with tooltip "Result has no primary key" (mirrors Week 3 inline-edit gating) |

---

## 5. Testing

### 5.1 Rust integration (`tests/vector.rs`)

Requires Postgres with pgvector. The docker compose stack must include the extension (the existing `infra/postgres/docker-compose.yml` may need an init script or the `pgvector/pgvector:pg16` image — implementation will pick the smaller diff).

Tests:

1. `list_vector_columns_returns_dim_and_index_flag`
2. `list_vector_indexes_parses_hnsw_params`
3. `list_vector_indexes_parses_ivfflat_lists`
4. `sample_vectors_returns_pk_and_vector` (single-PK)
5. `sample_vectors_handles_composite_pk`

### 5.2 Rust unit

- `vector_introspect.rs::parse_reloptions` — input `["m=16","ef_construction=64"]` → `{m:16, ef_construction:64}`.
- `db/sql_util.rs::quote_ident` — if added, edge cases (lowercase identity, mixed case quoted, embedded double-quote escape).

### 5.3 Frontend unit (vitest)

- `lib/vector/annSql.test.ts`
  - single PK + cosine
  - composite PK (2 cols)
  - all three operators
  - identifier with quotes/uppercase
  - limit clamp (negative → 1, 50000 → 10000)
- `lib/vector/cellRender.test.ts`
  - `l2Norm([3,4])` → 5
  - `formatVectorSummary` shape
- `features/vector/FindSimilarModal.test.tsx`
  - operator change updates preview
  - Run calls `newTab`+`updateSql`+`setActive` with expected SQL
- `features/vector/UmapControls.test.tsx`
  - slider/input changes invoke `onChange` with the right value

UMAP scatter and worker are not unit-tested; they're covered in manual verification.

### 5.4 Manual verification

`docs/superpowers/plans/manual-verification-week-6.md`:

1. Setup: docker compose with pgvector, seed `w6_items(id serial pk, embedding vector(384))` × 5 000, half indexed with HNSW.
2. Schema tree shows `vec(384)` + ⚠ on the unindexed half.
3. Run `SELECT * FROM w6_items LIMIT 50;` → cells show sparkline + L2; double-click → raw modal.
4. Right-click vector cell → Find similar → modal preview → Run → new tab with results sorted by distance.
5. Right-click column → Visualize → UMAP tab opens, controls slider, scatter renders within 60 s, click point → row detail panel.
6. Right-click table → Vector indexes → list shows HNSW with `m=16, ef_construction=64` parsed.
7. Create index form → Insert into editor → resulting SQL valid (manual `Cmd+Enter` succeeds).
8. Connect to a non-pgvector database → schema tree loads cleanly; no badges; no toast.

### 5.5 Gates

- `pnpm typecheck && pnpm lint && pnpm format:check`
- `pnpm test`
- `pnpm build`
- `pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml -- --include-ignored` (with docker postgres + pgvector up)

---

## 6. Out of scope

- Embedding generation in-app (would require model + cost decisions; users supply pre-embedded vectors).
- Editing vector cells (Week 3 already excludes vector from inline edit; we keep that).
- Other ANN index types (`pg_embedding`, `lantern`) — only HNSW/IVFFlat shipped with pgvector ≥0.5.
- Streaming UMAP for >50 k vectors — sample cap is the user's lever.
- Persisted UMAP results — re-computed each tab open.

---

## 7. Risks

1. **pgvector not installed but extension available** — JOIN against `pg_type` returns nothing, so it's safe; no special detection needed.
2. **Composite PK with non-trivial types** (timestamptz, uuid) — `buildAnnSql` and the row-fetch lookup must format these correctly. Mitigation: reuse the same parameter binding path as existing query execution (avoid string interpolation for PK values; use `$N` placeholders bound from the PK JSON).
3. **UMAP performance on 10 k × 1536** — measured ~30–60 s in `umap-js` README benchmarks. We display a progress bar and let the user reduce sample size. Worker prevents UI freeze.
4. **Schema cache staleness** — Week 6 scope: manual refresh button only. Week 7 polish can revisit auto-invalidation on DDL.
5. **Docker image change for pgvector** — switching to `pgvector/pgvector:pg16` is a small diff but affects existing weeks' tests; the new image is a superset (includes pgvector + base postgres) so existing tests must keep passing. Smoke test all weeks' integration tests after the swap before merging.

---

## 8. Open implementation questions (to decide in plan, not blocking design)

- Does `Tab` get a `kind` discriminator now (cleaner) or do we keep `Tab` SQL-only and create `useVectorTabs` (less intrusive)? Plan picks one with a brief rationale.
- Where does the "Vector indexes" panel mount — a Sheet/Drawer (Radix) or an inline right-side pane? Plan picks one.
- pgvector docker image switch vs init-script `CREATE EXTENSION` — plan picks based on diff size.
