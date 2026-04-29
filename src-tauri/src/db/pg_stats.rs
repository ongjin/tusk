//! Per-column cardinality lookup via `pg_stats`.
use std::collections::HashMap;

use serde::Serialize;

use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnStats {
    pub n_distinct: Option<f64>,
    pub null_frac: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct ColumnRef {
    pub schema: String,
    pub table: String,
    pub column: String,
}

pub async fn fetch_column_stats(
    pool: &sqlx::PgPool,
    refs: &[ColumnRef],
) -> TuskResult<HashMap<(String, String, String), ColumnStats>> {
    if refs.is_empty() {
        return Ok(HashMap::new());
    }

    let mut placeholders = String::new();
    for i in 0..refs.len() {
        if i > 0 {
            placeholders.push(',');
        }
        let base = i * 3 + 1;
        placeholders.push_str(&format!("(${}, ${}, ${})", base, base + 1, base + 2));
    }
    let sql = format!(
        r#"
        WITH input(schema_name, table_name, column_name) AS (
            VALUES {placeholders}
        )
        SELECT input.schema_name, input.table_name, input.column_name,
               s.n_distinct, s.null_frac
        FROM input
        LEFT JOIN pg_stats s
          ON s.schemaname = input.schema_name
         AND s.tablename = input.table_name
         AND s.attname = input.column_name
        "#
    );

    let mut q = sqlx::query_as::<_, (String, String, String, Option<f64>, Option<f32>)>(&sql);
    for r in refs {
        q = q.bind(&r.schema).bind(&r.table).bind(&r.column);
    }
    let rows = q
        .fetch_all(pool)
        .await
        .map_err(|e| TuskError::Explain(format!("pg_stats query failed: {e}")))?;

    let mut out = HashMap::with_capacity(rows.len());
    for (schema, table, column, n_distinct, null_frac_f32) in rows {
        out.insert(
            (schema, table, column),
            ColumnStats {
                n_distinct,
                null_frac: null_frac_f32.map(f64::from),
            },
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn missing_table_returns_none_pair() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect("postgres://tusk:tusk@127.0.0.1:55432/tusk_test")
            .await
            .unwrap();
        let refs = vec![ColumnRef {
            schema: "public".into(),
            table: "no_such_table_in_week5".into(),
            column: "no_such_column".into(),
        }];
        let m = fetch_column_stats(&pool, &refs).await.unwrap();
        let stats = m
            .get(&(
                "public".into(),
                "no_such_table_in_week5".into(),
                "no_such_column".into(),
            ))
            .unwrap();
        assert!(stats.n_distinct.is_none());
        assert!(stats.null_frac.is_none());
    }
}
