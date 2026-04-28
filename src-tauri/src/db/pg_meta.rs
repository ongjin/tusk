// src-tauri/src/db/pg_meta.rs
//
// Per-table metadata lookups (PK columns, enum values, FK targets) with
// LRU cache keyed by (conn_id, schema, table).

use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use lru::LruCache;
use serde::Serialize;
use sqlx::PgPool;

use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableMeta {
    pub schema: String,
    pub table: String,
    pub pk_columns: Vec<String>,
    pub columns: Vec<ColumnMetaRow>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMetaRow {
    pub name: String,
    pub oid: u32,
    pub type_name: String,
    pub nullable: bool,
    pub enum_values: Option<Vec<String>>,
    pub fk: Option<FkRef>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FkRef {
    pub schema: String,
    pub table: String,
    pub column: String,
}

const TTL: Duration = Duration::from_secs(60);

type Key = (String, String, String); // (conn_id, schema, table)
type Entry = (Instant, TableMeta);

pub struct MetaCache {
    inner: Mutex<LruCache<Key, Entry>>,
}

impl MetaCache {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(LruCache::new(NonZeroUsize::new(256).unwrap())),
        }
    }

    fn cached(&self, key: &Key) -> Option<TableMeta> {
        let mut c = self.inner.lock().unwrap();
        if let Some((stored_at, meta)) = c.get(key) {
            if stored_at.elapsed() < TTL {
                return Some(meta.clone());
            }
        }
        c.pop(key);
        None
    }

    fn store(&self, key: Key, meta: TableMeta) {
        self.inner.lock().unwrap().put(key, (Instant::now(), meta));
    }

    pub fn invalidate_conn(&self, conn_id: &str) {
        let mut c = self.inner.lock().unwrap();
        let to_remove: Vec<Key> = c
            .iter()
            .filter_map(|(k, _)| {
                if k.0 == conn_id {
                    Some(k.clone())
                } else {
                    None
                }
            })
            .collect();
        for k in to_remove {
            c.pop(&k);
        }
    }
}

impl Default for MetaCache {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn fetch_table_meta(
    pool: &PgPool,
    cache: &MetaCache,
    conn_id: &str,
    schema: &str,
    table: &str,
) -> TuskResult<TableMeta> {
    let key = (conn_id.to_string(), schema.to_string(), table.to_string());
    if let Some(m) = cache.cached(&key) {
        return Ok(m);
    }

    let cols_q = r#"
        SELECT a.attname AS name,
               a.atttypid::oid::int4 AS oid,
               t.typname AS type_name,
               NOT a.attnotnull AS nullable,
               t.typtype = 'e' AS is_enum,
               t.oid AS type_oid
        FROM pg_attribute a
        JOIN pg_class c ON c.oid = a.attrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        JOIN pg_type t ON t.oid = a.atttypid
        WHERE n.nspname = $1 AND c.relname = $2
          AND a.attnum > 0 AND NOT a.attisdropped
        ORDER BY a.attnum
    "#;
    let col_rows =
        sqlx::query_as::<_, (String, i32, String, bool, bool, sqlx::postgres::types::Oid)>(cols_q)
            .bind(schema)
            .bind(table)
            .fetch_all(pool)
            .await
            .map_err(|e| TuskError::State(format!("pg_meta cols: {e}")))?;

    if col_rows.is_empty() {
        return Err(TuskError::State(format!(
            "table {schema}.{table} not found"
        )));
    }

    let pk_q = r#"
        SELECT a.attname
        FROM pg_index i
        JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
        JOIN pg_class c ON c.oid = i.indrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = $1 AND c.relname = $2 AND i.indisprimary
        ORDER BY array_position(i.indkey, a.attnum)
    "#;
    let pk_rows: Vec<(String,)> = sqlx::query_as(pk_q)
        .bind(schema)
        .bind(table)
        .fetch_all(pool)
        .await
        .map_err(|e| TuskError::State(format!("pg_meta pk: {e}")))?;
    let pk_columns: Vec<String> = pk_rows.into_iter().map(|(n,)| n).collect();

    let fk_q = r#"
        SELECT
            att.attname            AS col,
            ns.nspname             AS ref_schema,
            cl.relname             AS ref_table,
            ratt.attname           AS ref_col
        FROM pg_constraint c
        JOIN pg_class cl_src ON cl_src.oid = c.conrelid
        JOIN pg_namespace ns_src ON ns_src.oid = cl_src.relnamespace
        JOIN pg_attribute att ON att.attrelid = c.conrelid AND att.attnum = ANY(c.conkey)
        JOIN pg_class cl ON cl.oid = c.confrelid
        JOIN pg_namespace ns ON ns.oid = cl.relnamespace
        JOIN pg_attribute ratt ON ratt.attrelid = c.confrelid AND ratt.attnum = ANY(c.confkey)
        WHERE ns_src.nspname = $1 AND cl_src.relname = $2 AND c.contype = 'f'
    "#;
    let fk_rows: Vec<(String, String, String, String)> = sqlx::query_as(fk_q)
        .bind(schema)
        .bind(table)
        .fetch_all(pool)
        .await
        .map_err(|e| TuskError::State(format!("pg_meta fk: {e}")))?;

    let mut columns = Vec::with_capacity(col_rows.len());
    for (name, oid_i32, type_name, nullable, is_enum, type_oid) in col_rows {
        let enum_values = if is_enum {
            let evs: Vec<(String,)> = sqlx::query_as(
                "SELECT enumlabel FROM pg_enum WHERE enumtypid = $1 ORDER BY enumsortorder",
            )
            .bind(type_oid)
            .fetch_all(pool)
            .await
            .map_err(|e| TuskError::State(format!("pg_meta enum: {e}")))?;
            Some(evs.into_iter().map(|(l,)| l).collect())
        } else {
            None
        };
        let fk = fk_rows
            .iter()
            .find(|(c, _, _, _)| c == &name)
            .map(|(_, s, t, c)| FkRef {
                schema: s.clone(),
                table: t.clone(),
                column: c.clone(),
            });
        // Prefer decoder's canonical PgTypeName mapping when known (matches frontend
        // PgTypeName union); fall back to pg_type.typname for enum / domain / array /
        // extension types that the decoder doesn't enumerate.
        let canonical = crate::db::decoder::pg_type_name(oid_i32 as u32);
        let resolved_type_name = if canonical == "unknown" {
            type_name
        } else {
            canonical.to_string()
        };
        columns.push(ColumnMetaRow {
            name,
            oid: oid_i32 as u32,
            type_name: resolved_type_name,
            nullable,
            enum_values,
            fk,
        });
    }

    let meta = TableMeta {
        schema: schema.to_string(),
        table: table.to_string(),
        pk_columns,
        columns,
    };
    cache.store(key, meta.clone());
    Ok(meta)
}
