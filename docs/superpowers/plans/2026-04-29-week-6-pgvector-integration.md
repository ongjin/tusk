# Week 6 — pgvector Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Tusk visibly the best Postgres client for pgvector users — auto-detect vector columns, show dim/index status in the schema tree, render vector cells with sparkline + L2, ship "Find similar rows" + "Visualize (UMAP)" + "Vector indexes" panel.

**Architecture:** Rust exposes 3 introspection commands (`list_vector_columns`, `list_vector_indexes`, `sample_vectors`). Frontend handles ANN SQL composition, UMAP execution (Web Worker + `umap-js`), and all UI. New tabs render UMAP via an optional `umap?: UmapTabState` field on the existing `Tab` (no discriminator refactor).

**Tech Stack:** sqlx + tokio-postgres, Tauri 2, React + TypeScript, Tailwind, zustand, `umap-js` (new dep), Vitest, cargo test.

---

## Spec reference

`docs/superpowers/specs/2026-04-29-week-6-pgvector-integration-design.md`. Re-read before starting.

---

## Conventions used by existing weeks

- Identifiers escaped with `quote_ident` (defined in `commands/export.rs` and `db/schema_embed.rs`; we'll add a private one in `commands/vector.rs` to keep diff small — Week 7 can DRY).
- All cargo integration tests live under `src-tauri/tests/*.rs` and use `#[ignore]` only when explicitly slow; otherwise the docker compose Postgres is assumed available.
- Frontend tests use Vitest + Testing Library. Shared selectors prefer role/text over class names.
- Commits: conventional format `feat(week6): …`. No `Co-Authored-By` trailer.

---

## Task 0: Prerequisite verification + pgvector docker swap

**Goal:** Postgres in docker compose has pgvector; all Week 1–5 tests still pass.

**Files:**
- Modify: `infra/postgres/docker-compose.yml`

**Steps:**

- [ ] **Step 1: Verify clean working tree**

```bash
git status
git log --oneline -3
```

Expected: clean, latest commit is `docs(week6): pgvector integration design`.

- [ ] **Step 2: Swap postgres image to pgvector-enabled**

Edit `infra/postgres/docker-compose.yml`:

```yaml
services:
  postgres:
    image: pgvector/pgvector:pg16
    environment:
      POSTGRES_USER: tusk
      POSTGRES_PASSWORD: tusk
      POSTGRES_DB: tusk_test
    ports:
      - "55432:5432"
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U tusk -d tusk_test"]
      interval: 1s
      timeout: 3s
      retries: 30
```

- [ ] **Step 3: Recreate the container with the new image**

```bash
docker compose -f infra/postgres/docker-compose.yml down
docker compose -f infra/postgres/docker-compose.yml up -d
docker compose -f infra/postgres/docker-compose.yml exec postgres psql -U tusk -d tusk_test -c "CREATE EXTENSION IF NOT EXISTS vector; SELECT extname, extversion FROM pg_extension WHERE extname='vector';"
```

Expected last line: `vector | 0.x.x`.

- [ ] **Step 4: Run all existing weeks' tests**

```bash
pnpm test
pnpm typecheck && pnpm lint
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml -- --include-ignored
```

Expected: all green. If any test fails because of timing / extension changes, stop and investigate before proceeding.

- [ ] **Step 5: Commit the docker change**

```bash
git add infra/postgres/docker-compose.yml
git commit -m "chore(week6): switch postgres image to pgvector/pgvector:pg16"
```

---

## Task 1: Rust — `commands/vector.rs` types skeleton

**Goal:** Define the public types in their own module so `db/vector_introspect.rs` (next task) can use them. No commands yet.

**Files:**
- Create: `src-tauri/src/commands/vector.rs`
- Modify: `src-tauri/src/commands/mod.rs`

**Steps:**

- [ ] **Step 1: Create `src-tauri/src/commands/vector.rs`**

```rust
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct VectorColumn {
    pub schema: String,
    pub table: String,
    pub column: String,
    pub dim: i32,
    pub has_index: bool,
}

#[derive(Debug, Serialize, Default, Clone)]
pub struct VectorIndexParams {
    pub m: Option<i32>,
    pub ef_construction: Option<i32>,
    pub lists: Option<i32>,
    pub ops: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct VectorIndex {
    pub name: String,
    pub schema: String,
    pub table: String,
    pub column: String,
    pub method: String,
    pub params: VectorIndexParams,
    pub size_bytes: i64,
    pub definition: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct SampledVectorRow {
    pub pk_json: serde_json::Value,
    pub vec: Vec<f32>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SampledVectors {
    pub rows: Vec<SampledVectorRow>,
    pub total_rows: i64,
}

pub(crate) fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_ident_doubles_embedded_quotes() {
        assert_eq!(quote_ident("plain"), "\"plain\"");
        assert_eq!(quote_ident("we\"ird"), "\"we\"\"ird\"");
    }
}
```

- [ ] **Step 2: Register the module**

Edit `src-tauri/src/commands/mod.rs`. After `pub mod transactions;` add:

```rust
pub mod vector;
```

- [ ] **Step 3: Compile**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --lib commands::vector::tests
```

Expected: green; `quote_ident_doubles_embedded_quotes` passes.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/vector.rs src-tauri/src/commands/mod.rs
git commit -m "feat(week6): commands/vector types + quote_ident"
```

---

## Task 2: Rust — `db/vector_introspect.rs`

**Goal:** Pure SQL builders + reloption parser. TDD with unit tests for the parser.

**Files:**
- Create: `src-tauri/src/db/vector_introspect.rs`
- Modify: `src-tauri/src/db/mod.rs`

**Steps:**

- [ ] **Step 1: Write the failing parser tests**

Create `src-tauri/src/db/vector_introspect.rs`:

```rust
use crate::commands::vector::{quote_ident, VectorIndexParams};

/// Static SQL for `list_vector_columns`. Bind: none (uses no parameters).
pub const SQL_LIST_VECTOR_COLUMNS: &str = r#"
SELECT n.nspname AS schema,
       c.relname AS table,
       a.attname AS column,
       (regexp_match(format_type(a.atttypid, a.atttypmod), 'vector\((\d+)\)'))[1]::int AS dim,
       EXISTS (
           SELECT 1
           FROM pg_index ix
           JOIN pg_class i ON i.oid = ix.indexrelid
           JOIN pg_am am ON am.oid = i.relam
           WHERE ix.indrelid = c.oid
             AND am.amname IN ('hnsw', 'ivfflat')
             AND a.attnum = ANY(ix.indkey)
       ) AS has_index
FROM pg_attribute a
JOIN pg_class c    ON c.oid = a.attrelid
JOIN pg_namespace n ON n.oid = c.relnamespace
JOIN pg_type t     ON t.oid = a.atttypid
WHERE t.typname = 'vector'
  AND c.relkind IN ('r','m','p')
  AND n.nspname NOT IN ('pg_catalog','information_schema')
  AND a.attnum > 0
  AND NOT a.attisdropped
ORDER BY n.nspname, c.relname, a.attnum;
"#;

/// Static SQL for `list_vector_indexes`. Bind: $1 = schema, $2 = table.
pub const SQL_LIST_VECTOR_INDEXES: &str = r#"
SELECT i.relname           AS name,
       n.nspname           AS schema,
       t.relname           AS table_name,
       a.attname           AS column,
       am.amname           AS method,
       COALESCE(i.reloptions, ARRAY[]::text[]) AS reloptions,
       pg_relation_size(i.oid) AS size_bytes,
       pg_get_indexdef(i.oid) AS definition
FROM pg_index ix
JOIN pg_class i    ON i.oid = ix.indexrelid
JOIN pg_class t    ON t.oid = ix.indrelid
JOIN pg_namespace n ON n.oid = t.relnamespace
JOIN pg_am am      ON am.oid = i.relam
JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ix.indkey[0]
WHERE n.nspname = $1
  AND t.relname = $2
  AND am.amname IN ('hnsw','ivfflat')
ORDER BY i.relname;
"#;

/// Build the SQL used by `sample_vectors`. Caller binds `$1` = limit (i64).
pub fn build_sample_vectors_sql(
    schema: &str,
    table: &str,
    vec_col: &str,
    pk_cols: &[String],
) -> String {
    let pk_sel = pk_cols
        .iter()
        .map(|c| quote_ident(c))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "SELECT {pk_sel}, {vec} \
         FROM {schema}.{table} \
         WHERE {vec} IS NOT NULL \
         ORDER BY random() \
         LIMIT $1",
        pk_sel = pk_sel,
        vec = quote_ident(vec_col),
        schema = quote_ident(schema),
        table = quote_ident(table),
    )
}

/// Parse a `pg_class.reloptions` array such as
/// `["m=16","ef_construction=64","lists=100"]` into structured params.
/// `index_definition` is used to extract the operator class
/// (e.g. `vector_cosine_ops`) since it's not in reloptions.
pub fn parse_reloptions(reloptions: &[String], index_definition: &str) -> VectorIndexParams {
    let mut out = VectorIndexParams::default();
    for opt in reloptions {
        if let Some((k, v)) = opt.split_once('=') {
            match k {
                "m" => out.m = v.parse().ok(),
                "ef_construction" => out.ef_construction = v.parse().ok(),
                "lists" => out.lists = v.parse().ok(),
                _ => {}
            }
        }
    }
    for op in [
        "vector_cosine_ops",
        "vector_l2_ops",
        "vector_ip_ops",
        "halfvec_cosine_ops",
        "halfvec_l2_ops",
        "halfvec_ip_ops",
    ] {
        if index_definition.contains(op) {
            out.ops = Some(op.to_string());
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_reloptions_hnsw() {
        let p = parse_reloptions(
            &[
                "m=16".to_string(),
                "ef_construction=64".to_string(),
            ],
            "CREATE INDEX foo ON public.t USING hnsw (v vector_cosine_ops) WITH (m='16', ef_construction='64')",
        );
        assert_eq!(p.m, Some(16));
        assert_eq!(p.ef_construction, Some(64));
        assert_eq!(p.lists, None);
        assert_eq!(p.ops.as_deref(), Some("vector_cosine_ops"));
    }

    #[test]
    fn parse_reloptions_ivfflat() {
        let p = parse_reloptions(
            &["lists=100".to_string()],
            "CREATE INDEX foo ON public.t USING ivfflat (v vector_l2_ops) WITH (lists='100')",
        );
        assert_eq!(p.lists, Some(100));
        assert_eq!(p.ops.as_deref(), Some("vector_l2_ops"));
    }

    #[test]
    fn parse_reloptions_unknown_keys_ignored() {
        let p = parse_reloptions(&["foo=bar".to_string()], "USING hnsw");
        assert!(p.m.is_none() && p.ef_construction.is_none() && p.lists.is_none());
        assert!(p.ops.is_none());
    }

    #[test]
    fn sample_vectors_sql_quotes_idents_and_handles_composite_pk() {
        let sql = build_sample_vectors_sql(
            "pub\"lic",
            "Items",
            "embedding",
            &["id".to_string(), "tenant".to_string()],
        );
        assert!(sql.contains("\"pub\"\"lic\".\"Items\""));
        assert!(sql.contains("\"id\", \"tenant\""));
        assert!(sql.contains("\"embedding\" IS NOT NULL"));
        assert!(sql.contains("LIMIT $1"));
    }
}
```

- [ ] **Step 2: Register the module**

Edit `src-tauri/src/db/mod.rs`. Add:

```rust
pub mod vector_introspect;
```

- [ ] **Step 3: Run unit tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib db::vector_introspect::tests
```

Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/vector_introspect.rs src-tauri/src/db/mod.rs
git commit -m "feat(week6): vector_introspect SQL helpers + reloption parser"
```

---

## Task 3: Rust — `list_vector_columns` command

**Goal:** Wire the SQL into a `#[tauri::command]`. Register in `lib.rs`.

**Files:**
- Modify: `src-tauri/src/commands/vector.rs`
- Modify: `src-tauri/src/lib.rs`

**Steps:**

- [ ] **Step 1: Implement the command**

Append to `src-tauri/src/commands/vector.rs` (above the `#[cfg(test)]` block):

```rust
use sqlx::Row;
use tauri::State;

use crate::db::pool::ConnectionRegistry;
use crate::db::vector_introspect::SQL_LIST_VECTOR_COLUMNS;
use crate::errors::{TuskError, TuskResult};

#[tauri::command]
pub async fn list_vector_columns(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
) -> TuskResult<Vec<VectorColumn>> {
    let pool = registry.pool(&connection_id)?;
    let rows = sqlx::query(SQL_LIST_VECTOR_COLUMNS)
        .fetch_all(&pool)
        .await
        .map_err(|e| TuskError::Query(e.to_string()))?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(VectorColumn {
            schema: r
                .try_get("schema")
                .map_err(|e| TuskError::Query(e.to_string()))?,
            table: r
                .try_get::<String, _>("table")
                .map_err(|e| TuskError::Query(e.to_string()))?,
            column: r
                .try_get("column")
                .map_err(|e| TuskError::Query(e.to_string()))?,
            dim: r
                .try_get::<Option<i32>, _>("dim")
                .map_err(|e| TuskError::Query(e.to_string()))?
                .unwrap_or(-1),
            has_index: r
                .try_get("has_index")
                .map_err(|e| TuskError::Query(e.to_string()))?,
        });
    }
    Ok(out)
}
```

- [ ] **Step 2: Register the command**

Edit `src-tauri/src/lib.rs`. Inside `tauri::generate_handler!`, after `commands::explain::run_explain,` add:

```rust
            commands::vector::list_vector_columns,
```

- [ ] **Step 3: Compile**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: green.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/vector.rs src-tauri/src/lib.rs
git commit -m "feat(week6): list_vector_columns command"
```

---

## Task 4: Rust — `list_vector_indexes` command

**Files:**
- Modify: `src-tauri/src/commands/vector.rs`
- Modify: `src-tauri/src/lib.rs`

**Steps:**

- [ ] **Step 1: Implement**

Append to `commands/vector.rs` (above `#[cfg(test)]`):

```rust
use crate::db::vector_introspect::{parse_reloptions, SQL_LIST_VECTOR_INDEXES};

#[tauri::command]
pub async fn list_vector_indexes(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
    schema: String,
    table: String,
) -> TuskResult<Vec<VectorIndex>> {
    let pool = registry.pool(&connection_id)?;
    let rows = sqlx::query(SQL_LIST_VECTOR_INDEXES)
        .bind(&schema)
        .bind(&table)
        .fetch_all(&pool)
        .await
        .map_err(|e| TuskError::Query(e.to_string()))?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let reloptions: Vec<String> = r
            .try_get::<Vec<String>, _>("reloptions")
            .map_err(|e| TuskError::Query(e.to_string()))?;
        let definition: String = r
            .try_get("definition")
            .map_err(|e| TuskError::Query(e.to_string()))?;
        let params = parse_reloptions(&reloptions, &definition);
        out.push(VectorIndex {
            name: r
                .try_get("name")
                .map_err(|e| TuskError::Query(e.to_string()))?,
            schema: r
                .try_get("schema")
                .map_err(|e| TuskError::Query(e.to_string()))?,
            table: r
                .try_get::<String, _>("table_name")
                .map_err(|e| TuskError::Query(e.to_string()))?,
            column: r
                .try_get("column")
                .map_err(|e| TuskError::Query(e.to_string()))?,
            method: r
                .try_get("method")
                .map_err(|e| TuskError::Query(e.to_string()))?,
            params,
            size_bytes: r
                .try_get("size_bytes")
                .map_err(|e| TuskError::Query(e.to_string()))?,
            definition,
        });
    }
    Ok(out)
}
```

- [ ] **Step 2: Register**

Edit `src-tauri/src/lib.rs`. After `commands::vector::list_vector_columns,` add:

```rust
            commands::vector::list_vector_indexes,
```

- [ ] **Step 3: Compile**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/vector.rs src-tauri/src/lib.rs
git commit -m "feat(week6): list_vector_indexes command"
```

---

## Task 5: Rust — `sample_vectors` command

**Files:**
- Modify: `src-tauri/src/commands/vector.rs`
- Modify: `src-tauri/src/lib.rs`

**Steps:**

- [ ] **Step 1: Implement**

Append to `commands/vector.rs` (above `#[cfg(test)]`):

```rust
use crate::db::vector_introspect::build_sample_vectors_sql;

#[tauri::command]
pub async fn sample_vectors(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
    schema: String,
    table: String,
    vec_col: String,
    pk_cols: Vec<String>,
    limit: u32,
) -> TuskResult<SampledVectors> {
    if pk_cols.is_empty() {
        return Err(TuskError::Query(
            "sample_vectors requires at least one PK column".into(),
        ));
    }
    let pool = registry.pool(&connection_id)?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| TuskError::Query(e.to_string()))?;
    sqlx::query("SET LOCAL statement_timeout = '30s'")
        .execute(&mut *tx)
        .await
        .map_err(|e| TuskError::Query(e.to_string()))?;

    let sql = build_sample_vectors_sql(&schema, &table, &vec_col, &pk_cols);
    let limit_i64 = limit as i64;
    let rows = sqlx::query(&sql)
        .bind(limit_i64)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| TuskError::Query(e.to_string()))?;

    let total_rows: i64 = sqlx::query_scalar(
        "SELECT COALESCE(reltuples::bigint, 0) FROM pg_class
         JOIN pg_namespace n ON n.oid = relnamespace
         WHERE n.nspname = $1 AND relname = $2",
    )
    .bind(&schema)
    .bind(&table)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| TuskError::Query(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| TuskError::Query(e.to_string()))?;

    let mut out_rows = Vec::with_capacity(rows.len());
    for r in rows {
        let mut pk_obj = serde_json::Map::new();
        for (i, key) in pk_cols.iter().enumerate() {
            let v: serde_json::Value = pg_value_to_json(&r, i)?;
            pk_obj.insert(key.clone(), v);
        }
        let vec: pgvector::Vector = r
            .try_get(pk_cols.len())
            .map_err(|e| TuskError::Query(e.to_string()))?;
        out_rows.push(SampledVectorRow {
            pk_json: serde_json::Value::Object(pk_obj),
            vec: vec.to_vec(),
        });
    }
    Ok(SampledVectors {
        rows: out_rows,
        total_rows,
    })
}

/// Best-effort conversion of an arbitrary Postgres column to JSON.
/// Used only for PK columns, which are typically int / bigint / uuid / text.
fn pg_value_to_json(row: &sqlx::postgres::PgRow, idx: usize) -> TuskResult<serde_json::Value> {
    use sqlx::postgres::PgValueRef;
    use sqlx::TypeInfo;
    use sqlx::ValueRef;

    let raw: PgValueRef = row
        .try_get_raw(idx)
        .map_err(|e| TuskError::Query(e.to_string()))?;
    if raw.is_null() {
        return Ok(serde_json::Value::Null);
    }
    let type_name = raw.type_info().name().to_string();
    match type_name.as_str() {
        "INT2" => row
            .try_get::<i16, _>(idx)
            .map(|v| serde_json::Value::from(v as i64))
            .map_err(|e| TuskError::Query(e.to_string())),
        "INT4" => row
            .try_get::<i32, _>(idx)
            .map(|v| serde_json::Value::from(v as i64))
            .map_err(|e| TuskError::Query(e.to_string())),
        "INT8" => row
            .try_get::<i64, _>(idx)
            .map(serde_json::Value::from)
            .map_err(|e| TuskError::Query(e.to_string())),
        "BOOL" => row
            .try_get::<bool, _>(idx)
            .map(serde_json::Value::from)
            .map_err(|e| TuskError::Query(e.to_string())),
        "FLOAT4" => row
            .try_get::<f32, _>(idx)
            .map(|v| serde_json::Value::from(v as f64))
            .map_err(|e| TuskError::Query(e.to_string())),
        "FLOAT8" => row
            .try_get::<f64, _>(idx)
            .map(serde_json::Value::from)
            .map_err(|e| TuskError::Query(e.to_string())),
        _ => row
            .try_get::<String, _>(idx)
            .map(serde_json::Value::from)
            .map_err(|e| TuskError::Query(e.to_string())),
    }
}
```

- [ ] **Step 2: Verify `pgvector` crate is already a dep**

```bash
grep -n "pgvector" src-tauri/Cargo.toml
```

If absent, add `pgvector = { version = "0.4", features = ["sqlx"] }` to `[dependencies]` and re-run `cargo check`. (Existing weeks already use the `vector` type via `Cell::Vector`, so the crate likely exists. If only the type wrapper is custom, prefer reading the raw `Vec<f32>` from the bytea representation — but the simpler path is the published `pgvector` crate.)

- [ ] **Step 3: Register**

Edit `src-tauri/src/lib.rs`. After `commands::vector::list_vector_indexes,` add:

```rust
            commands::vector::sample_vectors,
```

- [ ] **Step 4: Compile**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Expected: green.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/commands/vector.rs src-tauri/src/lib.rs
git commit -m "feat(week6): sample_vectors command with statement timeout"
```

---

## Task 6: Rust — integration tests `tests/vector.rs`

**Files:**
- Create: `src-tauri/tests/vector.rs`
- Modify: `src-tauri/tests/common.rs` if a vector-aware DB helper is needed (otherwise leave alone)

**Steps:**

- [ ] **Step 1: Inspect the existing test harness**

```bash
ls src-tauri/tests
sed -n '1,40p' src-tauri/tests/common.rs 2>/dev/null || true
```

Reuse the same connection-helper pattern other tests use (likely `connect_test_db()` returning a registry-bound connection_id). If it's named differently, adapt the imports below.

- [ ] **Step 2: Write the integration tests**

Create `src-tauri/tests/vector.rs`:

```rust
mod common;

use common::test_registry_with_connection;
use tusk_lib::commands::vector::{
    list_vector_columns, list_vector_indexes, sample_vectors, VectorColumn,
};

#[tokio::test]
async fn list_vector_columns_returns_dim_and_index_flag() {
    let (registry, conn_id) = test_registry_with_connection().await;
    let pool = registry.pool(&conn_id).unwrap();

    sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
        .execute(&pool).await.unwrap();
    sqlx::query("DROP TABLE IF EXISTS w6_vc_a, w6_vc_b CASCADE")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE w6_vc_a (id serial primary key, emb vector(8))")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE w6_vc_b (id serial primary key, emb vector(16))")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE INDEX ON w6_vc_a USING hnsw (emb vector_cosine_ops) WITH (m=16, ef_construction=64)")
        .execute(&pool).await.unwrap();

    let cols: Vec<VectorColumn> =
        list_vector_columns(tauri::State::from(&registry), conn_id.clone())
            .await
            .unwrap();
    let a = cols.iter().find(|c| c.table == "w6_vc_a").unwrap();
    let b = cols.iter().find(|c| c.table == "w6_vc_b").unwrap();
    assert_eq!(a.dim, 8);
    assert!(a.has_index);
    assert_eq!(b.dim, 16);
    assert!(!b.has_index);
}

#[tokio::test]
async fn list_vector_indexes_parses_hnsw_params() {
    let (registry, conn_id) = test_registry_with_connection().await;
    let pool = registry.pool(&conn_id).unwrap();
    sqlx::query("CREATE EXTENSION IF NOT EXISTS vector").execute(&pool).await.unwrap();
    sqlx::query("DROP TABLE IF EXISTS w6_vi_h CASCADE").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE w6_vi_h (id serial primary key, emb vector(8))")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE INDEX ON w6_vi_h USING hnsw (emb vector_cosine_ops) WITH (m=8, ef_construction=32)")
        .execute(&pool).await.unwrap();

    let idx = list_vector_indexes(
        tauri::State::from(&registry),
        conn_id.clone(),
        "public".into(),
        "w6_vi_h".into(),
    )
    .await
    .unwrap();
    assert_eq!(idx.len(), 1);
    let i = &idx[0];
    assert_eq!(i.method, "hnsw");
    assert_eq!(i.params.m, Some(8));
    assert_eq!(i.params.ef_construction, Some(32));
    assert_eq!(i.params.ops.as_deref(), Some("vector_cosine_ops"));
    assert!(i.size_bytes >= 0);
}

#[tokio::test]
async fn list_vector_indexes_parses_ivfflat_lists() {
    let (registry, conn_id) = test_registry_with_connection().await;
    let pool = registry.pool(&conn_id).unwrap();
    sqlx::query("CREATE EXTENSION IF NOT EXISTS vector").execute(&pool).await.unwrap();
    sqlx::query("DROP TABLE IF EXISTS w6_vi_iv CASCADE").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE w6_vi_iv (id serial primary key, emb vector(8))")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO w6_vi_iv (emb) SELECT array_fill(random()::float4, ARRAY[8])::vector FROM generate_series(1,200)")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE INDEX ON w6_vi_iv USING ivfflat (emb vector_l2_ops) WITH (lists=10)")
        .execute(&pool).await.unwrap();

    let idx = list_vector_indexes(
        tauri::State::from(&registry),
        conn_id.clone(),
        "public".into(),
        "w6_vi_iv".into(),
    ).await.unwrap();
    assert_eq!(idx.len(), 1);
    assert_eq!(idx[0].method, "ivfflat");
    assert_eq!(idx[0].params.lists, Some(10));
    assert_eq!(idx[0].params.ops.as_deref(), Some("vector_l2_ops"));
}

#[tokio::test]
async fn sample_vectors_returns_pk_and_vector() {
    let (registry, conn_id) = test_registry_with_connection().await;
    let pool = registry.pool(&conn_id).unwrap();
    sqlx::query("CREATE EXTENSION IF NOT EXISTS vector").execute(&pool).await.unwrap();
    sqlx::query("DROP TABLE IF EXISTS w6_sv_a CASCADE").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE w6_sv_a (id serial primary key, emb vector(4))")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO w6_sv_a (emb) SELECT array_fill(g::float4 / 10.0, ARRAY[4])::vector FROM generate_series(1,10) g")
        .execute(&pool).await.unwrap();

    let s = sample_vectors(
        tauri::State::from(&registry),
        conn_id.clone(),
        "public".into(),
        "w6_sv_a".into(),
        "emb".into(),
        vec!["id".to_string()],
        5,
    ).await.unwrap();
    assert_eq!(s.rows.len(), 5);
    assert_eq!(s.rows[0].vec.len(), 4);
    assert!(s.total_rows >= 0);
    assert!(s.rows[0].pk_json.get("id").is_some());
}

#[tokio::test]
async fn sample_vectors_handles_composite_pk() {
    let (registry, conn_id) = test_registry_with_connection().await;
    let pool = registry.pool(&conn_id).unwrap();
    sqlx::query("CREATE EXTENSION IF NOT EXISTS vector").execute(&pool).await.unwrap();
    sqlx::query("DROP TABLE IF EXISTS w6_sv_c CASCADE").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE w6_sv_c (tenant int, id int, emb vector(3), PRIMARY KEY (tenant, id))")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO w6_sv_c VALUES (1,1,'[0.1,0.2,0.3]'::vector), (1,2,'[0.4,0.5,0.6]'::vector)")
        .execute(&pool).await.unwrap();

    let s = sample_vectors(
        tauri::State::from(&registry),
        conn_id.clone(),
        "public".into(),
        "w6_sv_c".into(),
        "emb".into(),
        vec!["tenant".to_string(), "id".to_string()],
        10,
    ).await.unwrap();
    assert_eq!(s.rows.len(), 2);
    let pk = s.rows[0].pk_json.as_object().unwrap();
    assert!(pk.contains_key("tenant"));
    assert!(pk.contains_key("id"));
    assert_eq!(s.rows[0].vec.len(), 3);
}
```

If `test_registry_with_connection` is named differently in `tests/common.rs`, adjust the import. Do **not** alter the helper signature.

- [ ] **Step 3: Run integration tests**

```bash
docker compose -f infra/postgres/docker-compose.yml up -d
cargo test --manifest-path src-tauri/Cargo.toml --test vector
```

Expected: 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tests/vector.rs
git commit -m "test(week6): vector introspection + sample integration tests"
```

---

## Task 7: Frontend — `lib/vector/types.ts`

**Files:**
- Create: `src/lib/vector/types.ts`

**Steps:**

- [ ] **Step 1: Create the file**

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

export const ANN_OPERATOR_LABELS: Record<AnnOperator, string> = {
  "<=>": "cosine distance",
  "<->": "L2 distance",
  "<#>": "negative inner product",
};
```

- [ ] **Step 2: Typecheck**

```bash
pnpm typecheck
```

- [ ] **Step 3: Commit**

```bash
git add src/lib/vector/types.ts
git commit -m "feat(week6): vector types"
```

---

## Task 8: Frontend — typed Tauri wrappers

**Files:**
- Modify: `src/lib/tauri.ts`

**Steps:**

- [ ] **Step 1: Add wrappers**

Append to `src/lib/tauri.ts`:

```ts
import type {
  SampledVectors,
  VectorColumn,
  VectorIndex,
} from "@/lib/vector/types";

export function listVectorColumns(connectionId: string) {
  return invoke<RawVectorColumn[]>("list_vector_columns", {
    connectionId,
  }).then((rows) => rows.map(normalizeVectorColumn));
}

export function listVectorIndexes(
  connectionId: string,
  schema: string,
  table: string,
) {
  return invoke<RawVectorIndex[]>("list_vector_indexes", {
    connectionId,
    schema,
    table,
  }).then((rows) => rows.map(normalizeVectorIndex));
}

export function sampleVectors(args: {
  connectionId: string;
  schema: string;
  table: string;
  vecCol: string;
  pkCols: string[];
  limit: number;
}): Promise<SampledVectors> {
  return invoke<RawSampledVectors>("sample_vectors", {
    connectionId: args.connectionId,
    schema: args.schema,
    table: args.table,
    vecCol: args.vecCol,
    pkCols: args.pkCols,
    limit: args.limit,
  }).then(normalizeSampledVectors);
}

interface RawVectorColumn {
  schema: string;
  table: string;
  column: string;
  dim: number;
  has_index: boolean;
}
interface RawVectorIndexParams {
  m: number | null;
  ef_construction: number | null;
  lists: number | null;
  ops: string | null;
}
interface RawVectorIndex {
  name: string;
  schema: string;
  table: string;
  column: string;
  method: string;
  params: RawVectorIndexParams;
  size_bytes: number;
  definition: string;
}
interface RawSampledVectorRow {
  pk_json: Record<string, unknown>;
  vec: number[];
}
interface RawSampledVectors {
  rows: RawSampledVectorRow[];
  total_rows: number;
}

function normalizeVectorColumn(r: RawVectorColumn): VectorColumn {
  return {
    schema: r.schema,
    table: r.table,
    column: r.column,
    dim: r.dim,
    hasIndex: r.has_index,
  };
}
function normalizeVectorIndex(r: RawVectorIndex): VectorIndex {
  return {
    name: r.name,
    schema: r.schema,
    table: r.table,
    column: r.column,
    method: r.method === "ivfflat" ? "ivfflat" : "hnsw",
    params: {
      m: r.params.m ?? undefined,
      efConstruction: r.params.ef_construction ?? undefined,
      lists: r.params.lists ?? undefined,
      ops: r.params.ops ?? undefined,
    },
    sizeBytes: r.size_bytes,
    definition: r.definition,
  };
}
function normalizeSampledVectors(r: RawSampledVectors): SampledVectors {
  return {
    rows: r.rows.map((x) => ({ pkJson: x.pk_json, vec: x.vec })),
    totalRows: r.total_rows,
  };
}
```

(If `invoke` is not already imported in `tauri.ts`, add `import { invoke } from "@tauri-apps/api/core";` at the top.)

- [ ] **Step 2: Typecheck**

```bash
pnpm typecheck
```

- [ ] **Step 3: Commit**

```bash
git add src/lib/tauri.ts
git commit -m "feat(week6): typed wrappers for vector commands"
```

---

## Task 9: Frontend — `lib/vector/annSql.ts` (TDD)

**Files:**
- Create: `src/lib/vector/annSql.ts`
- Create: `src/lib/vector/annSql.test.ts`

**Steps:**

- [ ] **Step 1: Write failing tests**

Create `src/lib/vector/annSql.test.ts`:

```ts
import { describe, expect, it } from "vitest";

import { buildAnnSql } from "./annSql";

describe("buildAnnSql", () => {
  it("single PK + cosine", () => {
    const sql = buildAnnSql({
      schema: "public",
      table: "items",
      vecCol: "embedding",
      pkCols: ["id"],
      queryVector: [0.1, 0.2, 0.3],
      op: "<=>",
      limit: 20,
    });
    expect(sql).toContain('"public"."items"');
    expect(sql).toContain('"embedding" <=> \'[0.1,0.2,0.3]\'::vector AS distance');
    expect(sql).toMatch(/SELECT "id",/);
    expect(sql).toContain("ORDER BY distance");
    expect(sql).toContain("LIMIT 20");
  });

  it("composite PK selects both columns", () => {
    const sql = buildAnnSql({
      schema: "public",
      table: "items",
      vecCol: "embedding",
      pkCols: ["tenant", "id"],
      queryVector: [1, 2],
      op: "<->",
      limit: 5,
    });
    expect(sql).toMatch(/SELECT "tenant", "id",/);
    expect(sql).toContain("<->");
  });

  it("supports inner-product operator", () => {
    const sql = buildAnnSql({
      schema: "public",
      table: "items",
      vecCol: "embedding",
      pkCols: ["id"],
      queryVector: [0],
      op: "<#>",
      limit: 10,
    });
    expect(sql).toContain("<#>");
  });

  it("escapes identifiers with quotes/uppercase", () => {
    const sql = buildAnnSql({
      schema: 'pub"lic',
      table: "Items",
      vecCol: "Embedding",
      pkCols: ['I"d'],
      queryVector: [0],
      op: "<=>",
      limit: 1,
    });
    expect(sql).toContain('"pub""lic"."Items"');
    expect(sql).toContain('"Embedding"');
    expect(sql).toContain('"I""d"');
  });

  it("clamps limit to [1, 10000]", () => {
    expect(
      buildAnnSql({
        schema: "s",
        table: "t",
        vecCol: "v",
        pkCols: ["id"],
        queryVector: [0],
        op: "<=>",
        limit: -5,
      }),
    ).toContain("LIMIT 1");
    expect(
      buildAnnSql({
        schema: "s",
        table: "t",
        vecCol: "v",
        pkCols: ["id"],
        queryVector: [0],
        op: "<=>",
        limit: 99999,
      }),
    ).toContain("LIMIT 10000");
  });
});
```

- [ ] **Step 2: Run, expect failure**

```bash
pnpm test --run src/lib/vector/annSql.test.ts
```

Expected: cannot resolve `./annSql`.

- [ ] **Step 3: Implement**

Create `src/lib/vector/annSql.ts`:

```ts
import type { AnnOperator } from "./types";

export interface BuildAnnSqlArgs {
  schema: string;
  table: string;
  vecCol: string;
  pkCols: string[];
  queryVector: number[];
  op: AnnOperator;
  limit: number;
}

export function buildAnnSql(args: BuildAnnSqlArgs): string {
  const limit = Math.max(1, Math.min(10_000, Math.floor(args.limit)));
  const pkSelect = args.pkCols.map(escIdent).join(", ");
  const vecLit = `'[${args.queryVector.join(",")}]'::vector`;
  return [
    `SELECT ${pkSelect},`,
    `       ${escIdent(args.vecCol)} ${args.op} ${vecLit} AS distance,`,
    `       *`,
    `FROM ${escIdent(args.schema)}.${escIdent(args.table)}`,
    `ORDER BY distance`,
    `LIMIT ${limit};`,
  ].join("\n");
}

function escIdent(s: string): string {
  return `"${s.replace(/"/g, '""')}"`;
}
```

- [ ] **Step 4: Run, expect green**

```bash
pnpm test --run src/lib/vector/annSql.test.ts
```

Expected: 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/lib/vector/annSql.ts src/lib/vector/annSql.test.ts
git commit -m "feat(week6): buildAnnSql with operator + identifier escaping + limit clamp"
```

---

## Task 10: Frontend — `lib/vector/cellRender.ts` (TDD for helpers)

**Files:**
- Create: `src/lib/vector/cellRender.ts`
- Create: `src/lib/vector/cellRender.test.ts`

**Steps:**

- [ ] **Step 1: Write failing tests**

Create `src/lib/vector/cellRender.test.ts`:

```ts
import { describe, expect, it } from "vitest";

import { formatVectorSummary, l2Norm } from "./cellRender";

describe("l2Norm", () => {
  it("computes for [3,4]", () => {
    expect(l2Norm([3, 4])).toBeCloseTo(5);
  });
  it("zero vector → 0", () => {
    expect(l2Norm([0, 0, 0])).toBe(0);
  });
  it("empty → 0", () => {
    expect(l2Norm([])).toBe(0);
  });
});

describe("formatVectorSummary", () => {
  it("includes dim + norm", () => {
    expect(formatVectorSummary([3, 4])).toBe("[2d, ‖v‖=5.000]");
  });
});
```

- [ ] **Step 2: Run, expect failure**

```bash
pnpm test --run src/lib/vector/cellRender.test.ts
```

- [ ] **Step 3: Implement**

Create `src/lib/vector/cellRender.ts`:

```ts
export function l2Norm(v: number[]): number {
  let s = 0;
  for (const x of v) s += x * x;
  return Math.sqrt(s);
}

export function formatVectorSummary(v: number[]): string {
  return `[${v.length}d, ‖v‖=${l2Norm(v).toFixed(3)}]`;
}

/**
 * Draw a tiny sparkline of the first 32 dimensions (or fewer if shorter)
 * onto an existing canvas. Caller sizes the canvas; we only paint.
 */
export function renderSparkline(canvas: HTMLCanvasElement, v: number[]): void {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const w = canvas.width;
  const h = canvas.height;
  ctx.clearRect(0, 0, w, h);
  if (v.length === 0) return;
  const slice = v.slice(0, 32);
  let min = slice[0];
  let max = slice[0];
  for (const x of slice) {
    if (x < min) min = x;
    if (x > max) max = x;
  }
  const span = max - min || 1;
  ctx.beginPath();
  ctx.lineWidth = 1;
  ctx.strokeStyle = "currentColor";
  for (let i = 0; i < slice.length; i++) {
    const x = (i / Math.max(1, slice.length - 1)) * (w - 1);
    const norm = (slice[i] - min) / span;
    const y = (1 - norm) * (h - 1);
    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  }
  ctx.stroke();
}
```

- [ ] **Step 4: Run, expect green**

```bash
pnpm test --run src/lib/vector/cellRender.test.ts
```

- [ ] **Step 5: Commit**

```bash
git add src/lib/vector/cellRender.ts src/lib/vector/cellRender.test.ts
git commit -m "feat(week6): l2Norm + sparkline helpers"
```

---

## Task 11: Frontend — `useVectorMeta` store + connect-time refresh

**Files:**
- Create: `src/store/useVectorMeta.ts`
- Modify: `src/store/connections.ts` (only the `connect` action — add a one-line `refresh()` call)

**Steps:**

- [ ] **Step 1: Find the connect success site**

```bash
grep -n "async connect\|connect:\|connect(" src/store/connections.ts | head -20
```

Identify the function that runs after a successful connection (where `activeId` is set or `tauriConnect` resolves).

- [ ] **Step 2: Create the store**

Create `src/store/useVectorMeta.ts`:

```ts
import { create } from "zustand";

import { listVectorColumns } from "@/lib/tauri";
import type { VectorColumn } from "@/lib/vector/types";

interface State {
  byConn: Record<string, VectorColumn[]>;
  loading: Record<string, boolean>;
  refresh: (connId: string) => Promise<void>;
  hasVectorAt: (
    connId: string,
    schema: string,
    table: string,
    column: string,
  ) => VectorColumn | null;
  vectorColumnsForTable: (
    connId: string,
    schema: string,
    table: string,
  ) => VectorColumn[];
  tableHasVector: (connId: string, schema: string, table: string) => boolean;
}

export const useVectorMeta = create<State>((set, get) => ({
  byConn: {},
  loading: {},
  async refresh(connId) {
    set((s) => ({ loading: { ...s.loading, [connId]: true } }));
    try {
      const cols = await listVectorColumns(connId);
      set((s) => ({
        byConn: { ...s.byConn, [connId]: cols },
        loading: { ...s.loading, [connId]: false },
      }));
    } catch {
      set((s) => ({
        byConn: { ...s.byConn, [connId]: [] },
        loading: { ...s.loading, [connId]: false },
      }));
    }
  },
  hasVectorAt(connId, schema, table, column) {
    const list = get().byConn[connId] ?? [];
    return (
      list.find(
        (c) => c.schema === schema && c.table === table && c.column === column,
      ) ?? null
    );
  },
  vectorColumnsForTable(connId, schema, table) {
    return (get().byConn[connId] ?? []).filter(
      (c) => c.schema === schema && c.table === table,
    );
  },
  tableHasVector(connId, schema, table) {
    return get().vectorColumnsForTable(connId, schema, table).length > 0;
  },
}));
```

- [ ] **Step 3: Wire connect-time refresh**

Edit `src/store/connections.ts`. In the `connect` action, after the existing successful `invoke("connect", ...)` resolution (the line that sets active connection), add:

```ts
import("@/store/useVectorMeta").then((m) => {
  void m.useVectorMeta.getState().refresh(connectionId);
});
```

(Use dynamic import to avoid circular dependency between stores.)

- [ ] **Step 4: Typecheck**

```bash
pnpm typecheck
```

- [ ] **Step 5: Commit**

```bash
git add src/store/useVectorMeta.ts src/store/connections.ts
git commit -m "feat(week6): useVectorMeta store + connect-time refresh"
```

---

## Task 12: Frontend — SchemaTree vector badges + ⚠

**Files:**
- Modify: `src/features/sidebar/SchemaTree.tsx`

**Steps:**

- [ ] **Step 1: Locate the column row render**

```bash
grep -n "Column\|column\|attname" src/features/sidebar/SchemaTree.tsx | head -20
```

Identify the JSX where each column is rendered (the leaf row in the tree).

- [ ] **Step 2: Add badges**

In the imports:

```tsx
import { useVectorMeta } from "@/store/useVectorMeta";
```

Inside the component, near where `connId` is read:

```tsx
const hasVectorAt = useVectorMeta((s) => s.hasVectorAt);
```

In the column row JSX, after the column name span, add:

```tsx
{(() => {
  const v = hasVectorAt(connId, schemaName, tableName, columnName);
  if (!v) return null;
  return (
    <>
      <span
        className="text-muted-foreground ml-2 rounded bg-blue-500/10 px-1 text-[10px]"
        title={`vector(${v.dim})`}
      >
        vec({v.dim})
      </span>
      {!v.hasIndex && (
        <span
          className="ml-1 text-amber-600"
          title="No HNSW/IVFFlat index — sequential scan only"
        >
          ⚠
        </span>
      )}
    </>
  );
})()}
```

(Adapt `connId`, `schemaName`, `tableName`, `columnName` to whatever variable names already exist in the file; do not rename them.)

- [ ] **Step 3: Typecheck + lint**

```bash
pnpm typecheck && pnpm lint
```

- [ ] **Step 4: Commit**

```bash
git add src/features/sidebar/SchemaTree.tsx
git commit -m "feat(week6): SchemaTree vec(N) badge + missing-index warning"
```

---

## Task 13: Frontend — SchemaTree context menus

**Goal:** Add column context menu "Visualize (UMAP)" and table context menu "Vector indexes". Both fire callbacks the parent will wire (UMAP tab open / VectorIndexPanel open). For now, callbacks are passed via a small zustand "vectorActions" store so we don't need to thread props through the tree.

**Files:**
- Create: `src/store/useVectorActions.ts`
- Modify: `src/features/sidebar/SchemaTree.tsx`

**Steps:**

- [ ] **Step 1: Create actions store**

Create `src/store/useVectorActions.ts`:

```ts
import { create } from "zustand";

interface OpenUmapArgs {
  connId: string;
  schema: string;
  table: string;
  vecCol: string;
  pkCols: string[];
  dim: number;
}

interface OpenIndexPanelArgs {
  connId: string;
  schema: string;
  table: string;
}

interface State {
  openUmap: ((a: OpenUmapArgs) => void) | null;
  openIndexPanel: ((a: OpenIndexPanelArgs) => void) | null;
  setOpenUmap: (fn: ((a: OpenUmapArgs) => void) | null) => void;
  setOpenIndexPanel: (fn: ((a: OpenIndexPanelArgs) => void) | null) => void;
}

export const useVectorActions = create<State>((set) => ({
  openUmap: null,
  openIndexPanel: null,
  setOpenUmap: (fn) => set({ openUmap: fn }),
  setOpenIndexPanel: (fn) => set({ openIndexPanel: fn }),
}));
```

- [ ] **Step 2: Add menus in SchemaTree**

Locate the existing column row and table row context-menu setup. Use the same primitive (`@radix-ui/react-context-menu` or whatever the file already uses; check sibling rows). Add the items conditionally:

For column rows:

```tsx
{(() => {
  const v = hasVectorAt(connId, schemaName, tableName, columnName);
  if (!v) return null;
  return (
    <ContextMenuItem
      onSelect={() => {
        const open = useVectorActions.getState().openUmap;
        // pkCols come from the table's PK lookup — see Step 3
        if (open) open({
          connId,
          schema: schemaName,
          table: tableName,
          vecCol: columnName,
          pkCols: pkColsForTable,
          dim: v.dim,
        });
      }}
    >
      Visualize (UMAP)
    </ContextMenuItem>
  );
})()}
```

For table rows:

```tsx
{tableHasVector(connId, schemaName, tableName) && (
  <ContextMenuItem
    onSelect={() => {
      const open = useVectorActions.getState().openIndexPanel;
      if (open)
        open({ connId, schema: schemaName, table: tableName });
    }}
  >
    Vector indexes
  </ContextMenuItem>
)}
```

- [ ] **Step 3: Resolve `pkColsForTable`**

If SchemaTree already loads PK info per table (likely, from `list_columns`), reuse it. If not, fall back to: when "Visualize" is clicked, the UMAP tab itself queries PK on mount. To keep the menu simple, **pass an empty array for now**:

```tsx
pkCols: []
```

UmapTab (Task 21) will resolve PK columns via `list_columns` if `pkCols` is empty. Add a TODO comment in the menu so the executing engineer knows: `// pkCols resolved by UmapTab if empty`.

- [ ] **Step 4: Typecheck + lint**

```bash
pnpm typecheck && pnpm lint
```

- [ ] **Step 5: Commit**

```bash
git add src/store/useVectorActions.ts src/features/sidebar/SchemaTree.tsx
git commit -m "feat(week6): SchemaTree context menus for Visualize + Vector indexes"
```

---

## Task 14: Frontend — ResultsGrid vector cell renderer

**Files:**
- Modify: `src/features/results/ResultsGrid.tsx`

**Steps:**

- [ ] **Step 1: Find the cell render path**

```bash
grep -n "Cell\|cell\|render" src/features/results/ResultsGrid.tsx | head -30
```

Identify where cell values are formatted (likely a `formatCell` helper or a `<td>` mapping). Confirm vector values arrive as `{ kind: "Vector", value: number[] }` or similar (check `Cell::Vector` in `editing.rs`).

- [ ] **Step 2: Add vector rendering**

Imports:

```tsx
import { useEffect, useRef, useState } from "react";

import {
  formatVectorSummary,
  l2Norm,
  renderSparkline,
} from "@/lib/vector/cellRender";
```

Where the cell renderer decides what to show, branch on vector type. Replace the previous fallback for `vector` with:

```tsx
function VectorCell({ vec }: { vec: number[] }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    if (ref.current) renderSparkline(ref.current, vec);
  }, [vec]);
  return (
    <span
      className="inline-flex items-center gap-1 font-mono text-xs"
      title={`dim=${vec.length}, ‖v‖=${l2Norm(vec).toFixed(4)}`}
    >
      <canvas ref={ref} width={48} height={12} className="text-blue-500" />
      <span className="text-muted-foreground">{formatVectorSummary(vec)}</span>
    </span>
  );
}
```

In the cell mapping, when the cell's type is vector (the existing code likely uses a discriminator like `cell.kind === "Vector"` or `cell.type === "vector"` — match it):

```tsx
if (isVectorCell(cell)) return <VectorCell vec={cell.value} />;
```

- [ ] **Step 3: Add cell context menu item "Find similar rows"**

Find the existing cell context menu (Week 3 added Copy / Set NULL / etc.). Append:

```tsx
{isVectorCell(cell) && pkCols.length > 0 && (
  <ContextMenuItem
    onSelect={() =>
      useVectorActions.getState().openFindSimilar?.({
        connId,
        schema: tableSchema,
        table: tableName,
        vecCol: columnName,
        pkCols,
        queryVector: cell.value,
      })
    }
  >
    Find similar rows
  </ContextMenuItem>
)}
```

(`useVectorActions` will get a new field `openFindSimilar` — add it to the store now.)

- [ ] **Step 4: Extend `useVectorActions`**

Edit `src/store/useVectorActions.ts`. Add:

```ts
interface OpenFindSimilarArgs {
  connId: string;
  schema: string;
  table: string;
  vecCol: string;
  pkCols: string[];
  queryVector: number[];
}

// inside State:
openFindSimilar: ((a: OpenFindSimilarArgs) => void) | null;
setOpenFindSimilar: (fn: ((a: OpenFindSimilarArgs) => void) | null) => void;
```

And in the create body:

```ts
openFindSimilar: null,
setOpenFindSimilar: (fn) => set({ openFindSimilar: fn }),
```

- [ ] **Step 5: Double-click vector cell → raw modal**

In the same cell render path, when the existing double-click handler runs and `isVectorCell(cell)`, short-circuit the inline edit and open a tiny read-only modal showing the raw array. Use the existing modal primitive in the codebase (likely `<Dialog>` from shadcn). If a quick path exists (e.g. `confirm()`-style), use it; otherwise prepare a minimal Dialog:

```tsx
{vectorRawOpen && (
  <Dialog open onOpenChange={() => setVectorRawOpen(null)}>
    <DialogContent>
      <DialogHeader>
        <DialogTitle>Vector value</DialogTitle>
      </DialogHeader>
      <div className="text-muted-foreground text-xs">
        {formatVectorSummary(vectorRawOpen.value)}
      </div>
      <pre className="bg-muted max-h-[60vh] overflow-auto rounded p-2 text-[10px]">
        [{vectorRawOpen.value.join(", ")}]
      </pre>
      <DialogFooter>
        <Button
          onClick={() =>
            navigator.clipboard.writeText(
              `[${vectorRawOpen.value.join(",")}]`,
            )
          }
        >
          Copy
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
)}
```

with `vectorRawOpen` state in the component.

- [ ] **Step 6: Typecheck + lint**

```bash
pnpm typecheck && pnpm lint
```

- [ ] **Step 7: Commit**

```bash
git add src/features/results/ResultsGrid.tsx src/store/useVectorActions.ts
git commit -m "feat(week6): vector cell sparkline + Find similar menu + raw modal"
```

---

## Task 15: Frontend — `FindSimilarModal`

**Files:**
- Create: `src/features/vector/FindSimilarModal.tsx`
- Create: `src/features/vector/FindSimilarModal.test.tsx`
- Modify: `src/App.tsx` (or wherever top-level overlays live) to register the action handler

**Steps:**

- [ ] **Step 1: Implement**

Create `src/features/vector/FindSimilarModal.tsx`:

```tsx
import { useMemo, useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { buildAnnSql } from "@/lib/vector/annSql";
import {
  ANN_OPERATOR_LABELS,
  type AnnOperator,
} from "@/lib/vector/types";
import { useTabs } from "@/store/tabs";

export interface FindSimilarOpen {
  connId: string;
  schema: string;
  table: string;
  vecCol: string;
  pkCols: string[];
  queryVector: number[];
}

interface Props {
  open: FindSimilarOpen | null;
  onClose: () => void;
  onRun: (tabId: string) => void;
}

export function FindSimilarModal({ open, onClose, onRun }: Props) {
  const [op, setOp] = useState<AnnOperator>("<=>");
  const [limit, setLimit] = useState<number>(20);

  const sql = useMemo(() => {
    if (!open) return "";
    return buildAnnSql({
      schema: open.schema,
      table: open.table,
      vecCol: open.vecCol,
      pkCols: open.pkCols,
      queryVector: open.queryVector,
      op,
      limit,
    });
  }, [open, op, limit]);

  if (!open) return null;

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Find similar rows</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-3 text-sm">
          <div className="flex items-center gap-2">
            <label className="text-muted-foreground w-24 text-xs">
              Operator
            </label>
            <select
              value={op}
              onChange={(e) => setOp(e.target.value as AnnOperator)}
              className="border-input rounded border bg-transparent px-2 py-1 text-xs"
            >
              {(Object.keys(ANN_OPERATOR_LABELS) as AnnOperator[]).map((k) => (
                <option key={k} value={k}>
                  {k} — {ANN_OPERATOR_LABELS[k]}
                </option>
              ))}
            </select>
          </div>
          <div className="flex items-center gap-2">
            <label className="text-muted-foreground w-24 text-xs">LIMIT</label>
            <Input
              type="number"
              min={1}
              max={10000}
              value={limit}
              onChange={(e) => setLimit(Number(e.target.value))}
              className="w-32"
            />
          </div>
          <div>
            <div className="text-muted-foreground mb-1 text-xs">SQL</div>
            <pre className="bg-muted max-h-64 overflow-auto rounded p-2 text-[11px]">
              {sql}
            </pre>
          </div>
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button
            onClick={() => {
              const t = useTabs.getState();
              const id = t.newTab(open.connId);
              t.updateSql(id, sql);
              t.setActive(id);
              onRun(id);
              onClose();
            }}
          >
            Run
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
```

- [ ] **Step 2: Mount + register handler at app root**

Find the file that already mounts global overlays (search for `Toaster`, `<Dialog`, etc.). Likely `src/App.tsx`. Add:

```tsx
import { FindSimilarModal, type FindSimilarOpen } from "@/features/vector/FindSimilarModal";
import { useVectorActions } from "@/store/useVectorActions";
import { useEffect, useState } from "react";

// inside App component:
const [findSimilar, setFindSimilar] = useState<FindSimilarOpen | null>(null);
const setOpenFindSimilar = useVectorActions((s) => s.setOpenFindSimilar);
useEffect(() => {
  setOpenFindSimilar((args) => setFindSimilar(args));
  return () => setOpenFindSimilar(null);
}, [setOpenFindSimilar]);

// in JSX:
<FindSimilarModal
  open={findSimilar}
  onClose={() => setFindSimilar(null)}
  onRun={() => { /* tab is already active; results pane will show busy state once run is triggered */ }}
/>
```

The `Run` button creates a new tab and switches to it, but actually executing the SQL requires triggering the run action. Wire it via the `useTabs` extension below.

- [ ] **Step 3: Add `runActiveTab()` helper to `useTabs`**

Edit `src/store/tabs.ts`. Add a notify mechanism: a lightweight `requestRun` field that `EditorPane` listens to.

```ts
// in TabsState:
runRequestId: number;
requestRun: () => void;

// in create():
runRequestId: 0,
requestRun() {
  set((s) => ({ runRequestId: s.runRequestId + 1 }));
},
```

In `EditorPane.tsx`, subscribe:

```tsx
const runRequestId = useTabs((s) => s.runRequestId);
useEffect(() => {
  if (runRequestId > 0) run();
  // eslint-disable-next-line react-hooks/exhaustive-deps
}, [runRequestId]);
```

In `FindSimilarModal`, after `setActive(id)`, call `useTabs.getState().requestRun()` instead of `onRun`. Simplify `Props` accordingly:

```tsx
interface Props {
  open: FindSimilarOpen | null;
  onClose: () => void;
}
```

(Drop `onRun` prop and the matching App.tsx wiring.)

- [ ] **Step 4: Write the test**

Create `src/features/vector/FindSimilarModal.test.tsx`:

```tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { FindSimilarModal } from "./FindSimilarModal";

describe("FindSimilarModal", () => {
  it("renders SQL preview matching the operator", () => {
    render(
      <FindSimilarModal
        open={{
          connId: "c1",
          schema: "public",
          table: "items",
          vecCol: "embedding",
          pkCols: ["id"],
          queryVector: [0.1, 0.2],
        }}
        onClose={() => {}}
      />,
    );
    expect(
      screen.getByText(/embedding" <=> '\[0.1,0.2\]'::vector/),
    ).toBeInTheDocument();
  });

  it("changes operator updates preview", () => {
    render(
      <FindSimilarModal
        open={{
          connId: "c1",
          schema: "public",
          table: "items",
          vecCol: "embedding",
          pkCols: ["id"],
          queryVector: [0.1],
        }}
        onClose={() => {}}
      />,
    );
    const select = screen.getByRole("combobox");
    fireEvent.change(select, { target: { value: "<->" } });
    expect(screen.getByText(/<->/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 5: Run tests**

```bash
pnpm test --run src/features/vector/FindSimilarModal.test.tsx
pnpm typecheck && pnpm lint
```

- [ ] **Step 6: Commit**

```bash
git add src/features/vector/FindSimilarModal.tsx src/features/vector/FindSimilarModal.test.tsx src/App.tsx src/store/tabs.ts src/features/editor/EditorPane.tsx
git commit -m "feat(week6): FindSimilarModal + tab.requestRun"
```

---

## Task 16: Frontend — `Tab.umap` field + EditorPane routing

**Goal:** Add an optional `umap?: UmapTabState` field to the existing `Tab` and a `newUmapTab(...)` action. EditorPane renders `UmapTab` when present, else the existing editor + results.

**Files:**
- Modify: `src/store/tabs.ts`
- Modify: `src/features/editor/EditorPane.tsx`

**Steps:**

- [ ] **Step 1: Extend tabs store**

Edit `src/store/tabs.ts`. Above `interface Tab` add:

```ts
export interface UmapTabState {
  connId: string;
  schema: string;
  table: string;
  vecCol: string;
  pkCols: string[];
  dim: number;
  sample: number;
  nNeighbors: number;
  minDist: number;
  status: "idle" | "loading-pk" | "sampling" | "computing" | "ready" | "error";
  progress: number;
  error?: string;
  points?: { x: number; y: number; pkJson: Record<string, unknown> }[];
  selectedIdx?: number;
}
```

In `interface Tab` add:

```ts
umap?: UmapTabState;
```

In `TabsState` add:

```ts
newUmapTab: (init: {
  connId: string;
  schema: string;
  table: string;
  vecCol: string;
  pkCols: string[];
  dim: number;
}) => string;
patchUmap: (id: string, patch: Partial<UmapTabState>) => void;
```

In the store body:

```ts
newUmapTab(init) {
  const id = crypto.randomUUID();
  const umap: UmapTabState = {
    connId: init.connId,
    schema: init.schema,
    table: init.table,
    vecCol: init.vecCol,
    pkCols: init.pkCols,
    dim: init.dim,
    sample: 10000,
    nNeighbors: 15,
    minDist: 0.1,
    status: init.pkCols.length === 0 ? "loading-pk" : "sampling",
    progress: 0,
  };
  set((s) => ({
    tabs: [
      ...s.tabs,
      {
        id,
        title: `UMAP · ${init.schema}.${init.table}.${init.vecCol}`,
        connectionId: init.connId,
        sql: "",
        dirty: false,
        resultMode: "rows",
        umap,
      },
    ],
    activeId: id,
  }));
  return id;
},

patchUmap(id, patch) {
  set((s) => ({
    tabs: s.tabs.map((t) =>
      t.id === id && t.umap ? { ...t, umap: { ...t.umap, ...patch } } : t,
    ),
  }));
},
```

- [ ] **Step 2: Route in `EditorPane`**

Edit `src/features/editor/EditorPane.tsx`. Replace the `return (` content top-level layout to branch:

```tsx
if (activeTab.umap) {
  return <UmapTab tabId={activeTab.id} />;
}
```

(Add `import { UmapTab } from "@/features/vector/UmapTab";` — the file is created in Task 21. Until then, add a stub `UmapTab.tsx` that renders `<div>UMAP coming…</div>` so EditorPane compiles.)

- [ ] **Step 3: Stub `UmapTab` so the build passes**

Create `src/features/vector/UmapTab.tsx`:

```tsx
export function UmapTab(_props: { tabId: string }) {
  return <div className="p-4 text-sm">UMAP tab — implementation pending.</div>;
}
```

- [ ] **Step 4: Wire SchemaTree menu to open the UMAP tab**

Find the place in your app root (Task 15 used `App.tsx`) and register the handler:

```tsx
import { useVectorActions } from "@/store/useVectorActions";
import { useTabs } from "@/store/tabs";

useEffect(() => {
  const set = useVectorActions.getState().setOpenUmap;
  set((args) => useTabs.getState().newUmapTab(args));
  return () => set(null);
}, []);
```

- [ ] **Step 5: Typecheck + lint**

```bash
pnpm typecheck && pnpm lint
```

- [ ] **Step 6: Commit**

```bash
git add src/store/tabs.ts src/features/editor/EditorPane.tsx src/features/vector/UmapTab.tsx src/App.tsx
git commit -m "feat(week6): Tab.umap field + EditorPane routes UmapTab"
```

---

## Task 17: Frontend — `VectorIndexPanel` + Create index helper

**Files:**
- Create: `src/features/vector/VectorIndexPanel.tsx`
- Modify: `src/App.tsx` to mount + register the action

**Steps:**

- [ ] **Step 1: Implement panel**

Create `src/features/vector/VectorIndexPanel.tsx`:

```tsx
import { useEffect, useMemo, useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { Input } from "@/components/ui/input";
import { listVectorIndexes } from "@/lib/tauri";
import type { VectorIndex } from "@/lib/vector/types";
import { useTabs } from "@/store/tabs";
import { useVectorMeta } from "@/store/useVectorMeta";

export interface VectorIndexPanelOpen {
  connId: string;
  schema: string;
  table: string;
}

interface Props {
  open: VectorIndexPanelOpen | null;
  onClose: () => void;
}

export function VectorIndexPanel({ open, onClose }: Props) {
  const [indexes, setIndexes] = useState<VectorIndex[]>([]);
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const vectorCols = useVectorMeta((s) =>
    open ? s.vectorColumnsForTable(open.connId, open.schema, open.table) : [],
  );

  useEffect(() => {
    if (!open) return;
    setLoading(true);
    setErr(null);
    listVectorIndexes(open.connId, open.schema, open.table)
      .then(setIndexes)
      .catch((e) => setErr(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }, [open]);

  return (
    <Sheet open={!!open} onOpenChange={(o) => !o && onClose()}>
      <SheetContent side="right" className="w-[640px] max-w-[90vw]">
        <SheetHeader>
          <SheetTitle>
            Vector indexes — {open?.schema}.{open?.table}
          </SheetTitle>
        </SheetHeader>
        <div className="mt-4 flex flex-col gap-4 text-xs">
          {loading && (
            <div className="text-muted-foreground">Loading…</div>
          )}
          {err && (
            <div className="text-red-500">Error: {err}</div>
          )}
          {!loading && !err && indexes.length === 0 && (
            <div className="text-muted-foreground">
              No HNSW or IVFFlat indexes on this table.
            </div>
          )}
          {indexes.length > 0 && (
            <table className="w-full">
              <thead>
                <tr className="text-muted-foreground">
                  <th className="text-left">Name</th>
                  <th className="text-left">Column</th>
                  <th className="text-left">Method</th>
                  <th className="text-left">Params</th>
                  <th className="text-left">Size</th>
                </tr>
              </thead>
              <tbody>
                {indexes.map((i) => (
                  <tr key={i.name} className="border-border border-t">
                    <td className="font-mono">{i.name}</td>
                    <td>{i.column}</td>
                    <td>{i.method}</td>
                    <td>{paramsLabel(i)}</td>
                    <td>{(i.sizeBytes / 1024).toFixed(0)} KB</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          <CreateIndexForm
            schema={open?.schema ?? ""}
            table={open?.table ?? ""}
            vectorCols={vectorCols.map((c) => c.column)}
          />
        </div>
      </SheetContent>
    </Sheet>
  );
}

function paramsLabel(i: VectorIndex): string {
  const p = i.params;
  const parts: string[] = [];
  if (i.method === "hnsw") {
    parts.push(`m=${p.m ?? "?"}`);
    parts.push(`ef_construction=${p.efConstruction ?? "?"}`);
  } else {
    parts.push(`lists=${p.lists ?? "?"}`);
  }
  if (p.ops) parts.push(p.ops);
  return parts.join(" · ");
}

function CreateIndexForm({
  schema,
  table,
  vectorCols,
}: {
  schema: string;
  table: string;
  vectorCols: string[];
}) {
  const [col, setCol] = useState(vectorCols[0] ?? "");
  const [method, setMethod] = useState<"hnsw" | "ivfflat">("hnsw");
  const [m, setM] = useState(16);
  const [ef, setEf] = useState(64);
  const [lists, setLists] = useState(100);
  const [ops, setOps] = useState("vector_cosine_ops");

  const sql = useMemo(() => {
    if (!col) return "";
    if (method === "hnsw") {
      return `CREATE INDEX ON ${esc(schema)}.${esc(table)} USING hnsw (${esc(col)} ${ops}) WITH (m=${m}, ef_construction=${ef});`;
    }
    return `CREATE INDEX ON ${esc(schema)}.${esc(table)} USING ivfflat (${esc(col)} ${ops}) WITH (lists=${lists});`;
  }, [col, method, m, ef, lists, ops, schema, table]);

  return (
    <div className="border-border border-t pt-3">
      <h4 className="mb-2 font-semibold">Create index</h4>
      <div className="grid grid-cols-[120px_1fr] items-center gap-2">
        <label className="text-muted-foreground">Column</label>
        <select
          value={col}
          onChange={(e) => setCol(e.target.value)}
          className="border-input rounded border bg-transparent px-2 py-1 text-xs"
        >
          {vectorCols.map((c) => (
            <option key={c} value={c}>
              {c}
            </option>
          ))}
        </select>
        <label className="text-muted-foreground">Method</label>
        <select
          value={method}
          onChange={(e) => setMethod(e.target.value as "hnsw" | "ivfflat")}
          className="border-input rounded border bg-transparent px-2 py-1 text-xs"
        >
          <option value="hnsw">HNSW</option>
          <option value="ivfflat">IVFFlat</option>
        </select>
        <label className="text-muted-foreground">Operator class</label>
        <select
          value={ops}
          onChange={(e) => setOps(e.target.value)}
          className="border-input rounded border bg-transparent px-2 py-1 text-xs"
        >
          <option value="vector_cosine_ops">vector_cosine_ops</option>
          <option value="vector_l2_ops">vector_l2_ops</option>
          <option value="vector_ip_ops">vector_ip_ops</option>
        </select>
        {method === "hnsw" ? (
          <>
            <label className="text-muted-foreground">m</label>
            <Input
              type="number"
              value={m}
              onChange={(e) => setM(Number(e.target.value))}
            />
            <label className="text-muted-foreground">ef_construction</label>
            <Input
              type="number"
              value={ef}
              onChange={(e) => setEf(Number(e.target.value))}
            />
          </>
        ) : (
          <>
            <label className="text-muted-foreground">lists</label>
            <Input
              type="number"
              value={lists}
              onChange={(e) => setLists(Number(e.target.value))}
            />
          </>
        )}
      </div>
      <pre className="bg-muted mt-2 overflow-x-auto rounded p-2 text-[11px]">
        {sql}
      </pre>
      <Button
        size="sm"
        className="mt-2"
        onClick={() => {
          const t = useTabs.getState();
          const tab = t.tabs.find((x) => x.id === t.activeId);
          if (!tab) return;
          const next =
            (tab.sql ?? "") +
            (tab.sql.endsWith("\n") || tab.sql === "" ? "" : "\n") +
            sql +
            "\n";
          t.updateSql(tab.id, next);
        }}
      >
        Insert into editor
      </Button>
    </div>
  );
}

function esc(s: string): string {
  return `"${s.replace(/"/g, '""')}"`;
}
```

If `Sheet` is not yet a shadcn component in the project, add it: `npx shadcn@latest add sheet`. Otherwise reuse the existing primitive.

- [ ] **Step 2: Mount + register handler in `App.tsx`**

```tsx
import { VectorIndexPanel, type VectorIndexPanelOpen } from "@/features/vector/VectorIndexPanel";

const [indexPanel, setIndexPanel] = useState<VectorIndexPanelOpen | null>(null);
const setOpenIndexPanel = useVectorActions((s) => s.setOpenIndexPanel);
useEffect(() => {
  setOpenIndexPanel((args) => setIndexPanel(args));
  return () => setOpenIndexPanel(null);
}, [setOpenIndexPanel]);

// in JSX:
<VectorIndexPanel open={indexPanel} onClose={() => setIndexPanel(null)} />
```

- [ ] **Step 3: Typecheck + lint**

```bash
pnpm typecheck && pnpm lint
```

- [ ] **Step 4: Commit**

```bash
git add src/features/vector/VectorIndexPanel.tsx src/App.tsx components.json src/components/ui/sheet.tsx 2>/dev/null || true
git commit -m "feat(week6): VectorIndexPanel + Create index helper"
```

(If `sheet.tsx` was added by the shadcn CLI, include it.)

---

## Task 18: Frontend — UMAP worker + ts module

**Files:**
- Create: `src/lib/vector/umapWorker.entry.ts`
- Create: `src/lib/vector/umapWorker.ts`
- Modify: `package.json` (add `umap-js`)

**Steps:**

- [ ] **Step 1: Add dep**

```bash
pnpm add umap-js
pnpm add -D @types/umap-js 2>/dev/null || true
```

- [ ] **Step 2: Worker entry**

Create `src/lib/vector/umapWorker.entry.ts`:

```ts
import { UMAP } from "umap-js";

interface RunMsg {
  kind: "run";
  vecs: Float32Array;
  dim: number;
  count: number;
  nNeighbors: number;
  minDist: number;
}

self.addEventListener("message", (ev: MessageEvent<RunMsg>) => {
  const msg = ev.data;
  if (msg.kind !== "run") return;
  try {
    const data: number[][] = new Array(msg.count);
    for (let i = 0; i < msg.count; i++) {
      data[i] = Array.from(
        msg.vecs.subarray(i * msg.dim, (i + 1) * msg.dim),
      );
    }
    const umap = new UMAP({
      nComponents: 2,
      nNeighbors: msg.nNeighbors,
      minDist: msg.minDist,
    });
    const nEpochs = umap.initializeFit(data);
    for (let e = 0; e < nEpochs; e++) {
      umap.step();
      if (e % Math.max(1, Math.floor(nEpochs / 20)) === 0) {
        self.postMessage({ kind: "progress", value: e / nEpochs });
      }
    }
    const coords = umap.getEmbedding();
    const out = new Float32Array(coords.length * 2);
    for (let i = 0; i < coords.length; i++) {
      out[i * 2] = coords[i][0];
      out[i * 2 + 1] = coords[i][1];
    }
    self.postMessage({ kind: "done", coords: out }, { transfer: [out.buffer] });
  } catch (e) {
    self.postMessage({
      kind: "error",
      message: e instanceof Error ? e.message : String(e),
    });
  }
});

export {};
```

- [ ] **Step 3: TS wrapper**

Create `src/lib/vector/umapWorker.ts`:

```ts
export interface UmapRunArgs {
  vecs: Float32Array;
  dim: number;
  count: number;
  nNeighbors: number;
  minDist: number;
  onProgress?: (v: number) => void;
}

export function runUmap(args: UmapRunArgs): Promise<Float32Array> {
  return new Promise((resolve, reject) => {
    const worker = new Worker(
      new URL("./umapWorker.entry.ts", import.meta.url),
      { type: "module" },
    );
    worker.addEventListener("message", (ev) => {
      const m = ev.data as
        | { kind: "progress"; value: number }
        | { kind: "done"; coords: Float32Array }
        | { kind: "error"; message: string };
      if (m.kind === "progress") args.onProgress?.(m.value);
      else if (m.kind === "done") {
        resolve(m.coords);
        worker.terminate();
      } else if (m.kind === "error") {
        reject(new Error(m.message));
        worker.terminate();
      }
    });
    worker.postMessage(
      {
        kind: "run",
        vecs: args.vecs,
        dim: args.dim,
        count: args.count,
        nNeighbors: args.nNeighbors,
        minDist: args.minDist,
      },
      [args.vecs.buffer],
    );
  });
}
```

- [ ] **Step 4: Typecheck + build**

```bash
pnpm typecheck
pnpm build
```

The build must include the worker bundle (Vite handles `new URL(..., import.meta.url) + new Worker` natively).

- [ ] **Step 5: Commit**

```bash
git add package.json pnpm-lock.yaml src/lib/vector/umapWorker.ts src/lib/vector/umapWorker.entry.ts
git commit -m "feat(week6): umap-js Web Worker wrapper"
```

---

## Task 19: Frontend — `UmapControls`

**Files:**
- Create: `src/features/vector/UmapControls.tsx`
- Create: `src/features/vector/UmapControls.test.tsx`

**Steps:**

- [ ] **Step 1: Test**

Create `src/features/vector/UmapControls.test.tsx`:

```tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { UmapControls } from "./UmapControls";

describe("UmapControls", () => {
  it("calls onChange with new sample size", () => {
    const onChange = vi.fn();
    render(
      <UmapControls
        sample={10000}
        nNeighbors={15}
        minDist={0.1}
        onChange={onChange}
        onRun={() => {}}
        running={false}
      />,
    );
    const input = screen.getByLabelText(/sample/i);
    fireEvent.change(input, { target: { value: "5000" } });
    expect(onChange).toHaveBeenCalledWith({ sample: 5000 });
  });

  it("Run button disabled when running", () => {
    render(
      <UmapControls
        sample={10000}
        nNeighbors={15}
        minDist={0.1}
        onChange={() => {}}
        onRun={() => {}}
        running={true}
      />,
    );
    expect(screen.getByRole("button", { name: /run/i })).toBeDisabled();
  });
});
```

- [ ] **Step 2: Run, expect failure**

```bash
pnpm test --run src/features/vector/UmapControls.test.tsx
```

- [ ] **Step 3: Implement**

Create `src/features/vector/UmapControls.tsx`:

```tsx
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

interface Props {
  sample: number;
  nNeighbors: number;
  minDist: number;
  onChange: (
    patch: Partial<{ sample: number; nNeighbors: number; minDist: number }>,
  ) => void;
  onRun: () => void;
  running: boolean;
}

export function UmapControls({
  sample,
  nNeighbors,
  minDist,
  onChange,
  onRun,
  running,
}: Props) {
  return (
    <div className="border-border bg-muted/20 flex flex-col gap-3 border-r p-3 text-xs">
      <div className="flex flex-col gap-1">
        <label className="text-muted-foreground" htmlFor="umap-sample">
          Sample size
        </label>
        <Input
          id="umap-sample"
          type="number"
          min={100}
          max={50000}
          value={sample}
          onChange={(e) => onChange({ sample: Number(e.target.value) })}
        />
      </div>
      <div className="flex flex-col gap-1">
        <label className="text-muted-foreground">n_neighbors: {nNeighbors}</label>
        <input
          aria-label="n_neighbors"
          type="range"
          min={2}
          max={100}
          value={nNeighbors}
          onChange={(e) => onChange({ nNeighbors: Number(e.target.value) })}
        />
      </div>
      <div className="flex flex-col gap-1">
        <label className="text-muted-foreground">min_dist: {minDist.toFixed(2)}</label>
        <input
          aria-label="min_dist"
          type="range"
          min={0}
          max={0.99}
          step={0.01}
          value={minDist}
          onChange={(e) => onChange({ minDist: Number(e.target.value) })}
        />
      </div>
      <Button onClick={onRun} disabled={running} size="sm">
        {running ? "Running…" : "Re-run UMAP"}
      </Button>
    </div>
  );
}
```

- [ ] **Step 4: Run tests + typecheck**

```bash
pnpm test --run src/features/vector/UmapControls.test.tsx
pnpm typecheck && pnpm lint
```

- [ ] **Step 5: Commit**

```bash
git add src/features/vector/UmapControls.tsx src/features/vector/UmapControls.test.tsx
git commit -m "feat(week6): UmapControls"
```

---

## Task 20: Frontend — `UmapScatter`

**Files:**
- Create: `src/features/vector/UmapScatter.tsx`

**Steps:**

- [ ] **Step 1: Implement**

Create `src/features/vector/UmapScatter.tsx`:

```tsx
import { useEffect, useRef } from "react";

interface Point {
  x: number;
  y: number;
  pkJson: Record<string, unknown>;
}

interface Props {
  points: Point[];
  selectedIdx?: number;
  onSelect: (idx: number) => void;
}

export function UmapScatter({ points, selectedIdx, onSelect }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef({ tx: 0, ty: 0, scale: 1 });

  useEffect(() => {
    const c = canvasRef.current;
    const wrap = containerRef.current;
    if (!c || !wrap) return;

    const draw = () => {
      const ctx = c.getContext("2d");
      if (!ctx) return;
      const w = c.width;
      const h = c.height;
      ctx.clearRect(0, 0, w, h);
      if (points.length === 0) return;

      let minX = points[0].x, maxX = points[0].x;
      let minY = points[0].y, maxY = points[0].y;
      for (const p of points) {
        if (p.x < minX) minX = p.x;
        if (p.x > maxX) maxX = p.x;
        if (p.y < minY) minY = p.y;
        if (p.y > maxY) maxY = p.y;
      }
      const spanX = maxX - minX || 1;
      const spanY = maxY - minY || 1;
      const pad = 12;
      const v = viewRef.current;
      const project = (px: number, py: number): [number, number] => {
        const nx = ((px - minX) / spanX) * (w - 2 * pad) + pad;
        const ny = (1 - (py - minY) / spanY) * (h - 2 * pad) + pad;
        return [nx * v.scale + v.tx, ny * v.scale + v.ty];
      };

      ctx.fillStyle = "rgba(59,130,246,0.6)";
      for (let i = 0; i < points.length; i++) {
        const [x, y] = project(points[i].x, points[i].y);
        ctx.beginPath();
        ctx.arc(x, y, 2, 0, Math.PI * 2);
        ctx.fill();
      }
      if (selectedIdx !== undefined && points[selectedIdx]) {
        const p = points[selectedIdx];
        const [x, y] = project(p.x, p.y);
        ctx.strokeStyle = "rgb(239,68,68)";
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(x, y, 5, 0, Math.PI * 2);
        ctx.stroke();
      }

      // store projection for hit-testing
      (c as unknown as { _project: typeof project })._project = project;
    };

    const observer = new ResizeObserver(() => {
      const r = wrap.getBoundingClientRect();
      c.width = Math.floor(r.width);
      c.height = Math.floor(r.height);
      draw();
    });
    observer.observe(wrap);

    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const v = viewRef.current;
      const factor = Math.exp(-e.deltaY * 0.001);
      const rect = c.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      v.tx = mx - (mx - v.tx) * factor;
      v.ty = my - (my - v.ty) * factor;
      v.scale *= factor;
      draw();
    };
    let dragging = false;
    let lastX = 0,
      lastY = 0;
    const onDown = (e: MouseEvent) => {
      dragging = true;
      lastX = e.clientX;
      lastY = e.clientY;
    };
    const onMove = (e: MouseEvent) => {
      if (!dragging) return;
      const v = viewRef.current;
      v.tx += e.clientX - lastX;
      v.ty += e.clientY - lastY;
      lastX = e.clientX;
      lastY = e.clientY;
      draw();
    };
    const onUp = () => {
      dragging = false;
    };
    const onClick = (e: MouseEvent) => {
      const project = (c as unknown as { _project?: (x: number, y: number) => [number, number] })
        ._project;
      if (!project) return;
      const rect = c.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      let bestIdx = -1;
      let bestDist = Infinity;
      for (let i = 0; i < points.length; i++) {
        const [px, py] = project(points[i].x, points[i].y);
        const d = Math.hypot(px - mx, py - my);
        if (d < bestDist) {
          bestDist = d;
          bestIdx = i;
        }
      }
      if (bestIdx >= 0 && bestDist < 8) onSelect(bestIdx);
    };

    c.addEventListener("wheel", onWheel, { passive: false });
    c.addEventListener("mousedown", onDown);
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    c.addEventListener("click", onClick);

    return () => {
      observer.disconnect();
      c.removeEventListener("wheel", onWheel);
      c.removeEventListener("mousedown", onDown);
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      c.removeEventListener("click", onClick);
    };
  }, [points, selectedIdx, onSelect]);

  return (
    <div ref={containerRef} className="bg-background relative h-full w-full">
      <canvas ref={canvasRef} className="block h-full w-full" />
    </div>
  );
}
```

- [ ] **Step 2: Typecheck + lint**

```bash
pnpm typecheck && pnpm lint
```

- [ ] **Step 3: Commit**

```bash
git add src/features/vector/UmapScatter.tsx
git commit -m "feat(week6): UmapScatter canvas with zoom/pan/click"
```

---

## Task 21: Frontend — `UmapTab` orchestrator

**Files:**
- Modify: `src/features/vector/UmapTab.tsx` (replace stub)

**Steps:**

- [ ] **Step 1: Implement**

Replace `src/features/vector/UmapTab.tsx` with:

```tsx
import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

import { sampleVectors } from "@/lib/tauri";
import { runUmap } from "@/lib/vector/umapWorker";
import { useTabs } from "@/store/tabs";
import type { QueryResult } from "@/lib/types";
import { UmapControls } from "./UmapControls";
import { UmapScatter } from "./UmapScatter";

export function UmapTab({ tabId }: { tabId: string }) {
  const tab = useTabs((s) => s.tabs.find((t) => t.id === tabId));
  const patchUmap = useTabs((s) => s.patchUmap);
  const u = tab?.umap;
  const [rowDetail, setRowDetail] = useState<Record<string, unknown> | null>(
    null,
  );

  useEffect(() => {
    if (!u) return;
    if (u.status === "loading-pk") {
      void resolvePkAndStart(tabId, u, patchUmap);
    } else if (u.status === "sampling") {
      void runPipeline(tabId, u, patchUmap);
    }
  }, [u?.status]); // eslint-disable-line react-hooks/exhaustive-deps

  const points = useMemo(() => u?.points ?? [], [u?.points]);

  if (!tab || !u) return null;

  return (
    <div className="grid h-full grid-cols-[260px_1fr_320px]">
      <UmapControls
        sample={u.sample}
        nNeighbors={u.nNeighbors}
        minDist={u.minDist}
        running={u.status === "sampling" || u.status === "computing"}
        onChange={(p) => patchUmap(tabId, p)}
        onRun={() =>
          patchUmap(tabId, {
            status: "sampling",
            progress: 0,
            error: undefined,
          })
        }
      />
      <div className="relative">
        {(u.status === "sampling" || u.status === "computing") && (
          <div className="text-muted-foreground absolute right-3 top-3 z-10 text-xs">
            {u.status} {Math.round(u.progress * 100)}%
          </div>
        )}
        {u.status === "error" && (
          <div className="text-red-500 absolute inset-0 flex items-center justify-center text-sm">
            {u.error ?? "UMAP failed"}
          </div>
        )}
        <UmapScatter
          points={points}
          selectedIdx={u.selectedIdx}
          onSelect={(idx) => {
            patchUmap(tabId, { selectedIdx: idx });
            void fetchRow(u, points[idx]).then(setRowDetail);
          }}
        />
      </div>
      <div className="border-border overflow-auto border-l p-3 text-xs">
        {rowDetail ? (
          <pre className="whitespace-pre-wrap">
            {JSON.stringify(rowDetail, null, 2)}
          </pre>
        ) : (
          <div className="text-muted-foreground">Click a point to inspect.</div>
        )}
      </div>
    </div>
  );
}

async function resolvePkAndStart(
  tabId: string,
  u: NonNullable<ReturnType<typeof useTabs.getState>["tabs"][number]["umap"]>,
  patch: (id: string, p: Partial<typeof u>) => void,
): Promise<void> {
  try {
    const cols = await invoke<{ name: string; is_primary_key: boolean }[]>(
      "list_columns",
      {
        connectionId: u.connId,
        schema: u.schema,
        table: u.table,
      },
    );
    const pkCols = cols.filter((c) => c.is_primary_key).map((c) => c.name);
    if (pkCols.length === 0) {
      patch(tabId, {
        status: "error",
        error: "Table has no primary key — UMAP needs PK to map points back to rows.",
      });
      return;
    }
    patch(tabId, { pkCols, status: "sampling", progress: 0 });
  } catch (e) {
    patch(tabId, {
      status: "error",
      error: e instanceof Error ? e.message : String(e),
    });
  }
}

async function runPipeline(
  tabId: string,
  u: NonNullable<ReturnType<typeof useTabs.getState>["tabs"][number]["umap"]>,
  patch: (id: string, p: Partial<typeof u>) => void,
): Promise<void> {
  try {
    const sampled = await sampleVectors({
      connectionId: u.connId,
      schema: u.schema,
      table: u.table,
      vecCol: u.vecCol,
      pkCols: u.pkCols,
      limit: u.sample,
    });
    if (sampled.rows.length === 0) {
      patch(tabId, { status: "error", error: "No vectors sampled" });
      return;
    }
    const dim = sampled.rows[0].vec.length;
    const count = sampled.rows.length;
    const flat = new Float32Array(count * dim);
    for (let i = 0; i < count; i++) flat.set(sampled.rows[i].vec, i * dim);
    patch(tabId, { status: "computing", progress: 0 });
    const coords = await runUmap({
      vecs: flat,
      dim,
      count,
      nNeighbors: u.nNeighbors,
      minDist: u.minDist,
      onProgress: (v) => patch(tabId, { progress: v }),
    });
    const points = sampled.rows.map((r, i) => ({
      x: coords[i * 2],
      y: coords[i * 2 + 1],
      pkJson: r.pkJson,
    }));
    patch(tabId, { status: "ready", progress: 1, points });
  } catch (e) {
    patch(tabId, {
      status: "error",
      error: e instanceof Error ? e.message : String(e),
    });
    toast.error(e instanceof Error ? e.message : String(e));
  }
}

async function fetchRow(
  u: NonNullable<ReturnType<typeof useTabs.getState>["tabs"][number]["umap"]>,
  point: { pkJson: Record<string, unknown> },
): Promise<Record<string, unknown> | null> {
  const where = u.pkCols
    .map((c, i) => `"${c.replace(/"/g, '""')}" = $${i + 1}`)
    .join(" AND ");
  const params = u.pkCols.map((c) => point.pkJson[c]);
  const sql = `SELECT * FROM "${u.schema.replace(/"/g, '""')}"."${u.table.replace(/"/g, '""')}" WHERE ${where} LIMIT 1`;
  try {
    const r = await invoke<QueryResult>("execute_query_with_params", {
      connectionId: u.connId,
      sql,
      params,
    });
    if (!r.rows || r.rows.length === 0) return null;
    return r.rows[0] as unknown as Record<string, unknown>;
  } catch {
    // Fallback: literal interpolation (existing execute_query). Best-effort.
    const lit = u.pkCols
      .map((c, i) => `"${c.replace(/"/g, '""')}" = ${litVal(params[i])}`)
      .join(" AND ");
    const sql2 = `SELECT * FROM "${u.schema.replace(/"/g, '""')}"."${u.table.replace(/"/g, '""')}" WHERE ${lit} LIMIT 1`;
    const r = await invoke<QueryResult>("execute_query", {
      connectionId: u.connId,
      sql: sql2,
    });
    if (!r.rows || r.rows.length === 0) return null;
    return r.rows[0] as unknown as Record<string, unknown>;
  }
}

function litVal(v: unknown): string {
  if (typeof v === "number") return String(v);
  if (typeof v === "boolean") return v ? "TRUE" : "FALSE";
  if (v === null || v === undefined) return "NULL";
  return `'${String(v).replace(/'/g, "''")}'`;
}
```

If `execute_query_with_params` does not exist as a Tauri command, the fallback path is used. Document this in the manual verification doc (Task 22) — the fallback is safe for typical PK types (int, uuid, text) but inelegant.

- [ ] **Step 2: Typecheck + lint + build**

```bash
pnpm typecheck && pnpm lint
pnpm build
```

- [ ] **Step 3: Commit**

```bash
git add src/features/vector/UmapTab.tsx
git commit -m "feat(week6): UmapTab orchestrator (sample → worker → scatter → row detail)"
```

---

## Task 22: Manual verification document

**Files:**
- Create: `docs/superpowers/plans/manual-verification-week-6.md`

**Steps:**

- [ ] **Step 1: Write checklist**

```markdown
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
```

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/plans/manual-verification-week-6.md
git commit -m "docs(week6): manual verification checklist"
```

---

## Task 23: Final gate + closing checklist

**Files:** none

**Steps:**

- [ ] **Step 1: Run every gate**

```bash
pnpm install --frozen-lockfile
pnpm typecheck && pnpm lint && pnpm format:check
pnpm test
pnpm build
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
docker compose -f infra/postgres/docker-compose.yml up -d
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml -- --include-ignored
```

All green expected.

- [ ] **Step 2: Walk the manual checklist**

Walk every checkbox in `docs/superpowers/plans/manual-verification-week-6.md`.

- [ ] **Step 3: Closing checklist**

- [ ] §1 spec success criteria 10/10 verified manually.
- [ ] Only one new npm dep (`umap-js`); no new Rust crates beyond what was already in `Cargo.toml`. Confirm with `git diff main -- package.json src-tauri/Cargo.toml`.
- [ ] All commits use the convention from this header (no Co-Authored-By trailers).
- [ ] No `TODO` / `FIXME` left in shipped code (the comment in Task 13 step 3 about pkCols is acceptable as a runtime fallback note; no unfinished work).
- [ ] PLAN.md Week 6 sub-bullets all checked.

**Done.**
