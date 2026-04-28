use serde::Serialize;
use tauri::State;

use crate::db::pool::ConnectionRegistry;
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FkOption {
    pub pk_value: String,
    pub display: String,
}

#[tauri::command]
pub async fn fk_lookup(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
    schema: String,
    table: String,
    pk_column: String,
    query: Option<String>,
    limit: Option<i64>,
) -> TuskResult<Vec<FkOption>> {
    let pool = registry.pool(&connection_id)?;
    let lim = limit.unwrap_or(50);

    // Find first text-ish column for display (fallback to pk).
    let display_col: Option<String> = sqlx::query_scalar(
        r#"SELECT a.attname FROM pg_attribute a
           JOIN pg_type t ON t.oid = a.atttypid
           JOIN pg_class c ON c.oid = a.attrelid
           JOIN pg_namespace n ON n.oid = c.relnamespace
           WHERE n.nspname = $1 AND c.relname = $2
             AND a.attnum > 0 AND NOT a.attisdropped
             AND t.typname IN ('text','varchar','bpchar','name')
           ORDER BY a.attnum LIMIT 1"#,
    )
    .bind(&schema)
    .bind(&table)
    .fetch_optional(&pool)
    .await
    .map_err(|e| TuskError::Query(e.to_string()))?;

    let display = display_col.clone().unwrap_or_else(|| pk_column.clone());

    let where_clause = match &query {
        Some(q) if !q.is_empty() => format!(
            "WHERE \"{display}\"::text ILIKE '%{}%'",
            q.replace('\'', "''")
        ),
        _ => String::new(),
    };

    let sql = format!(
        "SELECT \"{pk_column}\"::text, \"{display}\"::text
         FROM \"{schema}\".\"{table}\" {where_clause}
         ORDER BY \"{pk_column}\" LIMIT {lim}"
    );
    let rows: Vec<(String, String)> = sqlx::query_as(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| TuskError::Query(e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|(pk, disp)| FkOption {
            pk_value: pk,
            display: disp,
        })
        .collect())
}
