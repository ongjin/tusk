//! Tool implementations exposed to the LLM (T18 frontend wraps them).

use serde::Serialize;
use sqlx::Row;
use tauri::State;

use crate::db::decoder::{columns_of, decode_row};
use crate::db::pool::ConnectionRegistry;
use crate::db::schema_embed::build_table_ddl;
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexRow {
    pub name: String,
    pub definition: String,
    pub is_unique: bool,
    pub is_primary: bool,
}

#[tauri::command]
pub async fn get_table_schema(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
    schema: String,
    table: String,
) -> TuskResult<String> {
    let pool = registry.pool(&connection_id)?;
    let ddl = build_table_ddl(&pool, &schema, &table).await?;
    Ok(ddl.ddl)
}

#[tauri::command]
pub async fn list_indexes(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
    schema: String,
    table: String,
) -> TuskResult<Vec<IndexRow>> {
    let pool = registry.pool(&connection_id)?;
    let rows = sqlx::query(
        "SELECT i.relname, pg_get_indexdef(ix.indexrelid),
                ix.indisunique, ix.indisprimary
         FROM pg_index ix
         JOIN pg_class i ON i.oid = ix.indexrelid
         WHERE ix.indrelid = ($1 || '.' || $2)::regclass
         ORDER BY i.relname",
    )
    .bind(&schema)
    .bind(&table)
    .fetch_all(&pool)
    .await
    .map_err(|e| TuskError::Query(e.to_string()))?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(IndexRow {
            name: r.try_get(0).map_err(|e| TuskError::Query(e.to_string()))?,
            definition: r.try_get(1).map_err(|e| TuskError::Query(e.to_string()))?,
            is_unique: r.try_get(2).map_err(|e| TuskError::Query(e.to_string()))?,
            is_primary: r.try_get(3).map_err(|e| TuskError::Query(e.to_string()))?,
        });
    }
    Ok(out)
}

#[tauri::command]
pub async fn sample_rows(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
    schema: String,
    table: String,
    limit: u32,
) -> TuskResult<serde_json::Value> {
    let pool = registry.pool(&connection_id)?;
    let limit = limit.min(20);
    // Schema/table are trusted (sourced from pg_class enum) — but wrap in
    // identifier quotes for defense in depth.
    let sql = format!("SELECT * FROM \"{schema}\".\"{table}\" LIMIT {limit}");
    let rows = sqlx::query(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| TuskError::Query(e.to_string()))?;

    // columns_of takes a single &PgRow (see decoder.rs line 131).
    let cols = rows.first().map(columns_of).unwrap_or_default();
    let decoded: Vec<_> = rows.iter().map(|r| decode_row(r, &cols)).collect();
    Ok(serde_json::json!({ "columns": cols, "rows": decoded }))
}
