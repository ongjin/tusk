use serde::Serialize;
use sqlx::Row;
use tauri::State;

use crate::db::pool::ConnectionRegistry;
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Serialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
}

async fn fetch_strings(
    registry: State<'_, ConnectionRegistry>,
    connection_id: &str,
    sql: &str,
) -> TuskResult<Vec<String>> {
    let pool = registry.pool(connection_id)?;
    let rows = sqlx::query(sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| TuskError::Query(e.to_string()))?;
    rows.iter()
        .map(|r| {
            r.try_get::<String, _>(0)
                .map_err(|e| TuskError::Query(e.to_string()))
        })
        .collect()
}

#[tauri::command]
pub async fn list_databases(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
) -> TuskResult<Vec<String>> {
    fetch_strings(
        registry,
        &connection_id,
        "SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname",
    )
    .await
}

#[tauri::command]
pub async fn list_schemas(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
) -> TuskResult<Vec<String>> {
    fetch_strings(
        registry,
        &connection_id,
        "SELECT schema_name FROM information_schema.schemata
         WHERE schema_name NOT IN ('pg_catalog','information_schema','pg_toast')
         ORDER BY schema_name",
    )
    .await
}

#[tauri::command]
pub async fn list_tables(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
    schema: String,
) -> TuskResult<Vec<String>> {
    let pool = registry.pool(&connection_id)?;
    let rows = sqlx::query(
        "SELECT table_name FROM information_schema.tables
         WHERE table_schema = $1 AND table_type = 'BASE TABLE'
         ORDER BY table_name",
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .map_err(|e| TuskError::Query(e.to_string()))?;
    rows.iter()
        .map(|r| {
            r.try_get::<String, _>(0)
                .map_err(|e| TuskError::Query(e.to_string()))
        })
        .collect()
}

#[tauri::command]
pub async fn list_columns(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
    schema: String,
    table: String,
) -> TuskResult<Vec<ColumnInfo>> {
    let pool = registry.pool(&connection_id)?;
    let rows = sqlx::query(
        "SELECT column_name, data_type, is_nullable
         FROM information_schema.columns
         WHERE table_schema = $1 AND table_name = $2
         ORDER BY ordinal_position",
    )
    .bind(&schema)
    .bind(&table)
    .fetch_all(&pool)
    .await
    .map_err(|e| TuskError::Query(e.to_string()))?;

    rows.iter()
        .map(|r| {
            let name: String = r.try_get(0).map_err(|e| TuskError::Query(e.to_string()))?;
            let data_type: String = r.try_get(1).map_err(|e| TuskError::Query(e.to_string()))?;
            let nullable: String = r.try_get(2).map_err(|e| TuskError::Query(e.to_string()))?;
            Ok(ColumnInfo {
                name,
                data_type,
                is_nullable: nullable.eq_ignore_ascii_case("yes"),
            })
        })
        .collect()
}
