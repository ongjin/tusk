# Week 6 — Manual Verification Checklist

## Setup

- [ ] `pnpm install`
- [ ] `docker compose -f infra/postgres/docker-compose.yml up -d`
- [ ] Seed:
      CREATE EXTENSION IF NOT EXISTS vector;
      DROP TABLE IF EXISTS w6_items CASCADE;
      CREATE TABLE w6_items (id serial primary key, label text, embedding vector(384));
      INSERT INTO w6_items (label, embedding)
      SELECT 'item ' || g, array_fill(random()::float4, ARRAY[384])::vector
      FROM generate_series(1, 5000) g;
      CREATE INDEX ON w6_items USING hnsw (embedding vector_cosine_ops) WITH (m=16, ef_construction=64);
      DROP TABLE IF EXISTS w6_items_unindexed CASCADE;
      CREATE TABLE w6_items_unindexed (id serial primary key, embedding vector(384));
      INSERT INTO w6_items_unindexed (embedding)
      SELECT array_fill(random()::float4, ARRAY[384])::vector FROM generate_series(1,500);
- [ ] `pnpm tauri dev`
- [ ] Connect to `127.0.0.1:55432 / tusk_test / tusk / tusk`.

## Schema tree

- [ ] Expand `public` → `w6_items` → `embedding` shows `vec(384)` badge, no ⚠.
- [ ] Expand `w6_items_unindexed` → `embedding` shows `vec(384)` + ⚠ tooltip "No HNSW/IVFFlat index".

## Cell rendering

- [ ] `SELECT id, embedding FROM w6_items LIMIT 50;` → embedding column shows tiny sparkline + `[384d, ‖v‖=...]` text.
- [ ] Hover a vector cell → tooltip shows `dim=384, ‖v‖=...`.
- [ ] Double-click a vector cell → modal shows full `[...]` and Copy button.

## Find similar

- [ ] Right-click an embedding cell → "Find similar rows" → modal with operator dropdown + LIMIT.
- [ ] Change operator from cosine to L2 → SQL preview updates.
- [ ] Click Run → new tab opens with the ANN SQL, results sorted by distance ascending.

## Visualize (UMAP)

- [ ] Right-click `embedding` column under `w6_items` → "Visualize (UMAP)" → new tab labeled "UMAP · public.w6_items.embedding".
- [ ] Status header progresses sampling → computing → ready within 60 s.
- [ ] Scatter renders with ≤5000 points; wheel-zoom and drag-pan work.
- [ ] Click a point → red highlight + right panel shows full row JSON.
- [ ] Change "n_neighbors" slider, hit Re-run → new layout renders.

## Vector indexes panel

- [ ] Right-click `w6_items` → "Vector indexes" → side panel.
- [ ] One row listed: HNSW with `m=16 · ef_construction=64 · vector_cosine_ops` and a non-zero size.
- [ ] Open Create index form → choose `embedding` + IVFFlat + lists=50 → preview SQL valid.
- [ ] Click "Insert into editor" → SQL appears in the active editor tab.

## Non-pgvector connection

- [ ] Connect to a Postgres without pgvector (or drop the extension on a side database) → schema tree loads cleanly with no badges, no toast.

## Regression

- [ ] Cmd+Enter run still works.
- [ ] Cmd+Shift+E EXPLAIN still works.
- [ ] Cmd+K still works.
- [ ] Inline cell edit still works on non-vector columns.

## Cleanup

- [ ] `docker compose -f infra/postgres/docker-compose.yml down`.
