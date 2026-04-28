// src-tauri/src/commands/query.rs
use std::time::Instant;

use serde::Serialize;
use tauri::State;

use crate::db::decoder::{columns_of, decode_row, Cell, ColumnMeta};
use crate::db::pool::ConnectionRegistry;
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Vec<Cell>>,
    pub duration_ms: u128,
    pub row_count: usize,
}

#[tauri::command]
pub async fn execute_query(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
    sql: String,
) -> TuskResult<QueryResult> {
    let pool = registry.pool(&connection_id)?;
    let started = Instant::now();
    let rows = sqlx::query(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| TuskError::Query(e.to_string()))?;
    let duration_ms = started.elapsed().as_millis();

    let columns = rows.first().map(columns_of).unwrap_or_default();
    let row_count = rows.len();
    let mut data = Vec::with_capacity(row_count);
    for row in &rows {
        data.push(decode_row(row, &columns));
    }

    Ok(QueryResult {
        columns,
        rows: data,
        duration_ms,
        row_count,
    })
}
