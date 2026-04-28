// src-tauri/src/commands/query.rs
use std::time::Instant;

use serde::Serialize;
use sqlx::{Column, Row, TypeInfo};
use tauri::State;

use crate::db::pool::ConnectionRegistry;
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Serialize)]
pub struct ColumnMeta {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Vec<serde_json::Value>>,
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

    let columns = rows
        .first()
        .map(|r| {
            r.columns()
                .iter()
                .map(|c| ColumnMeta {
                    name: c.name().to_string(),
                    type_name: c.type_info().name().to_string(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let row_count = rows.len();
    let mut data = Vec::with_capacity(row_count);
    for row in &rows {
        let mut cells = Vec::with_capacity(row.len());
        for i in 0..row.len() {
            cells.push(decode_cell(row, i));
        }
        data.push(cells);
    }

    Ok(QueryResult {
        columns,
        rows: data,
        duration_ms,
        row_count,
    })
}

fn decode_cell(row: &sqlx::postgres::PgRow, idx: usize) -> serde_json::Value {
    use sqlx::ValueRef;

    if let Ok(v) = row.try_get::<Option<bool>, _>(idx) {
        return serde_json::to_value(v).unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<i64>, _>(idx) {
        return serde_json::to_value(v).unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<f64>, _>(idx) {
        return serde_json::to_value(v).unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<String>, _>(idx) {
        return serde_json::to_value(v).unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<serde_json::Value>, _>(idx) {
        return v.unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(idx) {
        return serde_json::to_value(v.map(|d| d.to_rfc3339())).unwrap_or(serde_json::Value::Null);
    }
    let raw = row.try_get_raw(idx);
    match raw {
        Ok(value) if value.is_null() => serde_json::Value::Null,
        _ => serde_json::Value::String("<unsupported type>".into()),
    }
}
