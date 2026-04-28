//! Synthesize CREATE TABLE DDL strings from `pg_catalog` rows.
//! Deterministic — same input always yields same string + checksum.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use serde::Serialize;
use sqlx::{PgPool, Row};

use crate::errors::{TuskError, TuskResult};

/// Double-quote a PostgreSQL identifier, escaping any embedded double-quotes.
fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

#[derive(Debug, Clone, Serialize)]
pub struct TableDdl {
    pub schema: String,
    pub table: String,
    pub pg_relid: u32,
    pub ddl: String,
    pub checksum: String,
}

pub async fn list_user_tables(pool: &PgPool) -> TuskResult<Vec<(String, String, u32)>> {
    let rows = sqlx::query(
        "SELECT n.nspname, c.relname, c.oid::int4
         FROM pg_class c
         JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE c.relkind IN ('r','p','m')
           AND n.nspname NOT IN ('pg_catalog','information_schema','pg_toast')
         ORDER BY n.nspname, c.relname",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let n: String = r
            .try_get(0)
            .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
        let t: String = r
            .try_get(1)
            .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
        let oid: i32 = r
            .try_get(2)
            .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
        out.push((n, t, oid as u32));
    }
    Ok(out)
}

pub async fn build_table_ddl(pool: &PgPool, schema: &str, table: &str) -> TuskResult<TableDdl> {
    let cols = sqlx::query(
        "SELECT a.attname, format_type(a.atttypid, a.atttypmod), a.attnotnull,
                pg_get_expr(d.adbin, d.adrelid),
                col_description(a.attrelid, a.attnum)
         FROM pg_attribute a
         LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
         WHERE a.attrelid = ($1 || '.' || $2)::regclass
           AND a.attnum > 0
           AND NOT a.attisdropped
         ORDER BY a.attnum",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await
    .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;

    let mut ddl = format!(
        "CREATE TABLE {}.{} (\n",
        quote_ident(schema),
        quote_ident(table)
    );
    for (i, r) in cols.iter().enumerate() {
        let name: String = r
            .try_get(0)
            .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
        let ty: String = r
            .try_get(1)
            .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
        let notnull: bool = r
            .try_get(2)
            .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
        let default: Option<String> = r.try_get(3).ok();
        let comment: Option<String> = r.try_get::<Option<String>, _>(4).ok().flatten();
        let mut line = format!("  \"{name}\" {ty}");
        if notnull {
            line.push_str(" NOT NULL");
        }
        if let Some(d) = default {
            line.push_str(&format!(" DEFAULT {d}"));
        }
        if i + 1 < cols.len() {
            line.push(',');
        }
        if let Some(c) = comment {
            line.push_str(&format!("  -- {c}"));
        }
        line.push('\n');
        ddl.push_str(&line);
    }
    ddl.push_str(");\n");

    if let Ok(pk_rows) = sqlx::query(
        "SELECT a.attname
         FROM pg_index i
         JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
         WHERE i.indrelid = ($1 || '.' || $2)::regclass AND i.indisprimary
         ORDER BY array_position(i.indkey, a.attnum)",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await
    {
        let pk: Vec<String> = pk_rows
            .iter()
            .filter_map(|r| r.try_get::<String, _>(0).ok())
            .collect();
        if !pk.is_empty() {
            ddl.push_str(&format!(
                "ALTER TABLE {}.{} ADD PRIMARY KEY ({});\n",
                quote_ident(schema),
                quote_ident(table),
                pk.iter()
                    .map(|c| format!("\"{c}\""))
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }
    }

    if let Ok(fk_rows) = sqlx::query(
        "SELECT conname, pg_get_constraintdef(oid)
         FROM pg_constraint
         WHERE conrelid = ($1 || '.' || $2)::regclass AND contype = 'f'
         ORDER BY conname",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await
    {
        for r in fk_rows {
            let name: String = r.try_get(0).unwrap_or_default();
            let def: String = r.try_get(1).unwrap_or_default();
            ddl.push_str(&format!(
                "ALTER TABLE {}.{} ADD CONSTRAINT {name} {def};\n",
                quote_ident(schema),
                quote_ident(table),
            ));
        }
    }

    if let Ok(c) = sqlx::query("SELECT obj_description(($1 || '.' || $2)::regclass, 'pg_class')")
        .bind(schema)
        .bind(table)
        .fetch_one(pool)
        .await
    {
        if let Ok(Some(comment)) = c.try_get::<Option<String>, _>(0) {
            ddl.push_str(&format!(
                "COMMENT ON TABLE {}.{} IS '{}';\n",
                quote_ident(schema),
                quote_ident(table),
                comment.replace('\'', "''")
            ));
        }
    }

    let oid_row = sqlx::query("SELECT ($1 || '.' || $2)::regclass::oid::int4")
        .bind(schema)
        .bind(table)
        .fetch_one(pool)
        .await
        .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
    let pg_relid: i32 = oid_row
        .try_get(0)
        .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;

    let mut h = DefaultHasher::new();
    ddl.hash(&mut h);
    let checksum = format!("{:016x}", h.finish());

    Ok(TableDdl {
        schema: schema.to_string(),
        table: table.to_string(),
        pg_relid: pg_relid as u32,
        ddl,
        checksum,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_is_stable_for_same_string() {
        let a = "CREATE TABLE x (id int);";
        let mut h1 = DefaultHasher::new();
        a.hash(&mut h1);
        let mut h2 = DefaultHasher::new();
        a.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }
}
