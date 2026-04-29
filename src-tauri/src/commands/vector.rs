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
use crate::db::vector_introspect::{build_sample_vectors_sql, parse_reloptions, SQL_LIST_VECTOR_INDEXES};
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_ident_doubles_embedded_quotes() {
        assert_eq!(quote_ident("plain"), "\"plain\"");
        assert_eq!(quote_ident("we\"ird"), "\"we\"\"ird\"");
    }
}
