// src-tauri/src/commands/query.rs
use std::time::Instant;

use serde::Serialize;
use tauri::State;

use crate::commands::sqlast::{parse_select_target, NotEditableReason, ParsedSelect};
use crate::db::decoder::{columns_of, decode_row, Cell, ColumnMeta};
use crate::db::pg_meta::{fetch_table_meta, FkRef, MetaCache};
use crate::db::pool::ConnectionRegistry;
use crate::db::state::{HistoryEntry, StateStore};
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Vec<Cell>>,
    pub duration_ms: u128,
    pub row_count: usize,
    pub meta: ResultMeta,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultMeta {
    pub editable: bool,
    pub reason: Option<String>,
    pub table: Option<TableRef>,
    pub pk_columns: Vec<String>,
    pub pk_column_indices: Vec<usize>,
    pub column_types: Vec<ColumnTypeMeta>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableRef {
    pub schema: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnTypeMeta {
    pub name: String,
    pub oid: u32,
    pub type_name: String,
    pub nullable: bool,
    pub enum_values: Option<Vec<String>>,
    pub fk: Option<FkRef>,
}

#[tauri::command]
pub async fn execute_query(
    registry: State<'_, ConnectionRegistry>,
    meta_cache: State<'_, MetaCache>,
    store: State<'_, StateStore>,
    connection_id: String,
    sql: String,
) -> TuskResult<QueryResult> {
    let pool = registry.pool(&connection_id)?;
    let started = Instant::now();
    let result = sqlx::query(&sql).fetch_all(&pool).await;
    let duration_ms = started.elapsed().as_millis();
    let preview: String = sql.chars().take(200).collect();

    let (status, err_msg, rc): (&str, Option<String>, Option<i64>) = match &result {
        Ok(rows) => ("ok", None, Some(rows.len() as i64)),
        Err(e) => ("error", Some(e.to_string()), None),
    };
    let entry_id = uuid::Uuid::new_v4().to_string();
    if let Err(history_err) = store.insert_history_entry(&HistoryEntry {
        id: entry_id,
        conn_id: connection_id.clone(),
        source: "editor".into(),
        tx_id: None,
        sql_preview: preview,
        sql_full: Some(sql.clone()),
        started_at: chrono::Utc::now().timestamp_millis(),
        duration_ms: duration_ms as i64,
        row_count: rc,
        status: status.into(),
        error_message: err_msg,
        statement_count: 1,
    }) {
        eprintln!("failed to record history entry: {history_err}");
    }

    let rows = result.map_err(|e| TuskError::Query(e.to_string()))?;
    let columns = rows.first().map(columns_of).unwrap_or_default();
    let row_count = rows.len();
    let mut data = Vec::with_capacity(row_count);
    for row in &rows {
        data.push(decode_row(row, &columns));
    }

    let meta = build_meta(
        &pool,
        meta_cache.inner(),
        &connection_id,
        &sql,
        &columns,
        row_count,
    )
    .await;

    Ok(QueryResult {
        columns,
        rows: data,
        duration_ms,
        row_count,
        meta,
    })
}

async fn build_meta(
    pool: &sqlx::PgPool,
    cache: &MetaCache,
    conn_id: &str,
    sql: &str,
    columns: &[ColumnMeta],
    row_count: usize,
) -> ResultMeta {
    let parsed = parse_select_target(sql);
    let (schema, table) = match parsed {
        ParsedSelect::SingleTable { schema, table } => (schema, table),
        ParsedSelect::NotEditable { reason } => {
            return not_editable(reason_to_string(&reason), columns, vec![], vec![]);
        }
    };
    if row_count > 10_000 {
        return not_editable("too-large".into(), columns, vec![], vec![]);
    }
    let table_meta = match fetch_table_meta(pool, cache, conn_id, &schema, &table).await {
        Ok(m) => m,
        Err(_) => return not_editable("unknown-type".into(), columns, vec![], vec![]),
    };
    let pk_indices: Vec<usize> = table_meta
        .pk_columns
        .iter()
        .filter_map(|pk| columns.iter().position(|c| c.name == *pk))
        .collect();
    if pk_indices.len() != table_meta.pk_columns.len() {
        return not_editable(
            "pk-not-in-select".into(),
            columns,
            table_meta.pk_columns.clone(),
            vec![],
        );
    }
    let column_types = columns
        .iter()
        .map(|c| {
            let row = table_meta.columns.iter().find(|cm| cm.name == c.name);
            ColumnTypeMeta {
                name: c.name.clone(),
                oid: c.oid,
                type_name: c.type_name.clone(),
                nullable: row.map(|r| r.nullable).unwrap_or(true),
                enum_values: row.and_then(|r| r.enum_values.clone()),
                fk: row.and_then(|r| r.fk.clone()),
            }
        })
        .collect();
    ResultMeta {
        editable: true,
        reason: None,
        table: Some(TableRef {
            schema,
            name: table,
        }),
        pk_columns: table_meta.pk_columns,
        pk_column_indices: pk_indices,
        column_types,
    }
}

fn not_editable(
    reason: String,
    columns: &[ColumnMeta],
    pk_columns: Vec<String>,
    pk_column_indices: Vec<usize>,
) -> ResultMeta {
    ResultMeta {
        editable: false,
        reason: Some(reason),
        table: None,
        pk_columns,
        pk_column_indices,
        column_types: columns
            .iter()
            .map(|c| ColumnTypeMeta {
                name: c.name.clone(),
                oid: c.oid,
                type_name: c.type_name.clone(),
                nullable: true,
                enum_values: None,
                fk: None,
            })
            .collect(),
    }
}

fn reason_to_string(r: &NotEditableReason) -> String {
    match r {
        NotEditableReason::NotSelect => "no-pk".into(),
        NotEditableReason::MultiTable => "multi-table".into(),
        NotEditableReason::Cte => "computed".into(),
        NotEditableReason::Subquery => "computed".into(),
        NotEditableReason::Computed => "computed".into(),
        NotEditableReason::ParserFailed => "parser-failed".into(),
        NotEditableReason::MultipleStatements => "computed".into(),
    }
}
