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
