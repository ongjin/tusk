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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_ident_doubles_embedded_quotes() {
        assert_eq!(quote_ident("plain"), "\"plain\"");
        assert_eq!(quote_ident("we\"ird"), "\"we\"\"ird\"");
    }
}
