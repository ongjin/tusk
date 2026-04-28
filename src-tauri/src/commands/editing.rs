// src-tauri/src/commands/editing.rs
//
// Pure builders that turn a `PendingBatch` into a parameterized
// `sqlx::query` (executed against the connection) plus a literal-inlined
// SQL string (Preview / response). Atomic Submit handles both PkOnly and
// Strict conflict detection modes.

use serde::{Deserialize, Serialize};
use sqlx::Postgres;
use tauri::State;

use crate::db::decoder::{columns_of, decode_row, Cell};
use crate::db::pg_literals::to_literal;
use crate::db::pool::ConnectionRegistry;
use crate::db::state::{HistoryEntry, HistoryStatement, StateStore};
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingBatch {
    pub batch_id: String,
    pub op: PendingOp,
    pub table: TableRef,
    pub pk_columns: Vec<String>,
    pub pk_values: Vec<Cell>,
    pub edits: Vec<ColumnEdit>,
    pub captured_row: Vec<Cell>,
    pub captured_columns: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnEdit {
    pub column: String,
    pub next: Cell,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableRef {
    pub schema: String,
    pub name: String,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PendingOp {
    Update,
    Insert,
    Delete,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum ConflictMode {
    PkOnly,
    Strict,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum BatchResult {
    Ok {
        batch_id: String,
        affected: u64,
        executed_sql: String,
    },
    Conflict {
        batch_id: String,
        executed_sql: String,
        current: Vec<Cell>,
    },
    Error {
        batch_id: String,
        executed_sql: String,
        message: String,
    },
}

pub struct BuiltUpdate {
    pub parameterized_sql: String,
    pub binds: Vec<Cell>,
    pub preview_sql: String,
}

pub fn build_update(b: &PendingBatch, mode: ConflictMode) -> TuskResult<BuiltUpdate> {
    if b.op != PendingOp::Update {
        return Err(TuskError::Editing(format!(
            "expected Update, got {:?}",
            b.op
        )));
    }
    if b.edits.is_empty() {
        return Err(TuskError::Editing("update with no edits".into()));
    }
    let table_ident = format!("\"{}\".\"{}\"", b.table.schema, b.table.name);

    // SET clause
    let mut set_parts = Vec::with_capacity(b.edits.len());
    let mut set_preview = Vec::with_capacity(b.edits.len());
    let mut binds: Vec<Cell> = Vec::new();
    for (i, e) in b.edits.iter().enumerate() {
        set_parts.push(format!("\"{}\" = ${}", e.column, i + 1));
        set_preview.push(format!("\"{}\" = {}", e.column, to_literal(&e.next)));
        binds.push(e.next.clone());
    }

    // WHERE clause: PK always.
    let mut where_parts = Vec::new();
    let mut where_preview = Vec::new();
    for (j, (pkc, pkv)) in b.pk_columns.iter().zip(b.pk_values.iter()).enumerate() {
        let bind_idx = binds.len() + 1;
        where_parts.push(format!("\"{}\" IS NOT DISTINCT FROM ${}", pkc, bind_idx));
        where_preview.push(format!(
            "\"{}\" IS NOT DISTINCT FROM {}",
            pkc,
            to_literal(pkv)
        ));
        binds.push(pkv.clone());
        let _ = j;
    }

    if let ConflictMode::Strict = mode {
        // Add per-column NULL-safe equality on every non-PK captured column,
        // INCLUDING the column being edited. The captured `val` is the
        // column's ORIGINAL value (snapshot at edit-start), so comparing
        // against it detects same-column lost updates: a concurrent client
        // that changed this column will produce affected=0.
        // Skip floats (PG IS NOT DISTINCT FROM still works for floats but
        // exact-bit equality is misleading; spec calls this out).
        for (col, val) in b.captured_columns.iter().zip(b.captured_row.iter()) {
            let is_pk = b.pk_columns.contains(col);
            let is_float = matches!(val, Cell::Float(_));
            if is_pk || is_float {
                continue;
            }
            let bind_idx = binds.len() + 1;
            where_parts.push(format!("\"{}\" IS NOT DISTINCT FROM ${}", col, bind_idx));
            where_preview.push(format!(
                "\"{}\" IS NOT DISTINCT FROM {}",
                col,
                to_literal(val)
            ));
            binds.push(val.clone());
        }
    }

    let parameterized_sql = format!(
        "UPDATE {table_ident} SET {} WHERE {}",
        set_parts.join(", "),
        where_parts.join(" AND ")
    );
    let preview_sql = format!(
        "UPDATE {table_ident} SET {} WHERE {}",
        set_preview.join(", "),
        where_preview.join(" AND ")
    );
    Ok(BuiltUpdate {
        parameterized_sql,
        binds,
        preview_sql,
    })
}

pub fn build_insert(b: &PendingBatch) -> TuskResult<BuiltUpdate> {
    if b.op != PendingOp::Insert {
        return Err(TuskError::Editing(format!(
            "expected Insert, got {:?}",
            b.op
        )));
    }
    if b.edits.is_empty() {
        return Err(TuskError::Editing("insert with no values".into()));
    }
    let table_ident = format!("\"{}\".\"{}\"", b.table.schema, b.table.name);
    let cols: Vec<String> = b
        .edits
        .iter()
        .map(|e| format!("\"{}\"", e.column))
        .collect();
    let placeholders: Vec<String> = (1..=b.edits.len()).map(|i| format!("${i}")).collect();
    let preview_vals: Vec<String> = b.edits.iter().map(|e| to_literal(&e.next)).collect();
    let binds: Vec<Cell> = b.edits.iter().map(|e| e.next.clone()).collect();
    let parameterized_sql = format!(
        "INSERT INTO {table_ident} ({}) VALUES ({})",
        cols.join(", "),
        placeholders.join(", ")
    );
    let preview_sql = format!(
        "INSERT INTO {table_ident} ({}) VALUES ({})",
        cols.join(", "),
        preview_vals.join(", ")
    );
    Ok(BuiltUpdate {
        parameterized_sql,
        binds,
        preview_sql,
    })
}

pub fn build_delete(b: &PendingBatch, mode: ConflictMode) -> TuskResult<BuiltUpdate> {
    if b.op != PendingOp::Delete {
        return Err(TuskError::Editing(format!(
            "expected Delete, got {:?}",
            b.op
        )));
    }
    let table_ident = format!("\"{}\".\"{}\"", b.table.schema, b.table.name);
    let mut where_parts = Vec::new();
    let mut where_preview = Vec::new();
    let mut binds = Vec::new();
    for (pkc, pkv) in b.pk_columns.iter().zip(b.pk_values.iter()) {
        let bind_idx = binds.len() + 1;
        where_parts.push(format!("\"{}\" IS NOT DISTINCT FROM ${}", pkc, bind_idx));
        where_preview.push(format!(
            "\"{}\" IS NOT DISTINCT FROM {}",
            pkc,
            to_literal(pkv)
        ));
        binds.push(pkv.clone());
    }
    if let ConflictMode::Strict = mode {
        for (col, val) in b.captured_columns.iter().zip(b.captured_row.iter()) {
            if b.pk_columns.contains(col) {
                continue;
            }
            if matches!(val, Cell::Float(_)) {
                continue;
            }
            let bind_idx = binds.len() + 1;
            where_parts.push(format!("\"{}\" IS NOT DISTINCT FROM ${}", col, bind_idx));
            where_preview.push(format!(
                "\"{}\" IS NOT DISTINCT FROM {}",
                col,
                to_literal(val)
            ));
            binds.push(val.clone());
        }
    }
    let parameterized_sql = format!(
        "DELETE FROM {table_ident} WHERE {}",
        where_parts.join(" AND ")
    );
    let preview_sql = format!(
        "DELETE FROM {table_ident} WHERE {}",
        where_preview.join(" AND ")
    );
    Ok(BuiltUpdate {
        parameterized_sql,
        binds,
        preview_sql,
    })
}

pub fn bind_cells<'q>(
    q: sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments>,
    binds: &'q [Cell],
) -> sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments> {
    let mut q = q;
    for c in binds {
        q = match c {
            Cell::Null => q.bind(None::<i32>),
            Cell::Bool(v) => q.bind(*v),
            Cell::Int(v) => q.bind(*v),
            Cell::Bigint(s) => q.bind(s.parse::<i64>().unwrap_or(0)),
            Cell::Float(v) => q.bind(*v),
            Cell::Numeric(s) => q.bind(s.parse::<bigdecimal::BigDecimal>().unwrap_or_default()),
            Cell::Text(v) => q.bind(v.clone()),
            Cell::Bytea { b64 } => {
                use base64::{engine::general_purpose::STANDARD, Engine};
                q.bind(STANDARD.decode(b64).unwrap_or_default())
            }
            Cell::Uuid(v) => q.bind(uuid::Uuid::parse_str(v).unwrap_or_default()),
            Cell::Inet(v) => q.bind(
                v.parse::<ipnetwork::IpNetwork>()
                    .unwrap_or_else(|_| "0.0.0.0/0".parse().unwrap()),
            ),
            Cell::Date(v) => q.bind(v.parse::<chrono::NaiveDate>().unwrap_or_default()),
            Cell::Time(v) => q.bind(v.parse::<chrono::NaiveTime>().unwrap_or_default()),
            Cell::Timestamp(v) => q.bind(v.parse::<chrono::NaiveDateTime>().unwrap_or_default()),
            Cell::Timestamptz(v) => q.bind(
                v.parse::<chrono::DateTime<chrono::Utc>>()
                    .unwrap_or_default(),
            ),
            Cell::Json(v) => q.bind(v.clone()),
            // Other variants (Interval, Array, Enum, Vector, Timetz, Unknown) are not
            // typically in a PendingBatch's bind list (Week 3 widget set).
            // Bind as Null for safety; a later task can extend.
            // TODO(week-3+): add proper bindings for Interval/Array/Enum/Vector/Timetz/Unknown.
            _ => q.bind(None::<i32>),
        };
    }
    q
}

/// Build the parameterized + preview SQL for a batch using the chosen mode.
/// Pure — no DB I/O.
fn build_for_batch(b: &PendingBatch, mode: ConflictMode) -> TuskResult<BuiltUpdate> {
    match b.op {
        PendingOp::Update => build_update(b, mode),
        PendingOp::Insert => build_insert(b),
        PendingOp::Delete => build_delete(b, mode),
    }
}

/// Preview the rendered (literal-inlined) SQL for each batch — no execution.
#[tauri::command]
pub async fn preview_pending_changes(
    batches: Vec<PendingBatch>,
    mode: ConflictMode,
) -> TuskResult<Vec<BatchResult>> {
    let mut out = Vec::with_capacity(batches.len());
    for b in &batches {
        match build_for_batch(b, mode) {
            Ok(built) => out.push(BatchResult::Ok {
                batch_id: b.batch_id.clone(),
                affected: 0,
                executed_sql: built.preview_sql,
            }),
            Err(e) => out.push(BatchResult::Error {
                batch_id: b.batch_id.clone(),
                executed_sql: String::new(),
                message: e.to_string(),
            }),
        }
    }
    Ok(out)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitResponse {
    pub batches: Vec<BatchResult>,
}

/// Fetch the current server-side row state for a conflict report.
/// Returns the row decoded as `Vec<Cell>` aligned with `b.captured_columns`.
async fn fetch_current<'e, E>(executor: E, b: &PendingBatch) -> Option<Vec<Cell>>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    if b.captured_columns.is_empty() || b.pk_columns.is_empty() {
        return None;
    }
    let table_ident = format!("\"{}\".\"{}\"", b.table.schema, b.table.name);
    let cols: Vec<String> = b
        .captured_columns
        .iter()
        .map(|c| format!("\"{}\"", c))
        .collect();
    let mut where_parts = Vec::with_capacity(b.pk_columns.len());
    for (i, pk) in b.pk_columns.iter().enumerate() {
        where_parts.push(format!("\"{}\" IS NOT DISTINCT FROM ${}", pk, i + 1));
    }
    let sql = format!(
        "SELECT {} FROM {} WHERE {} LIMIT 1",
        cols.join(", "),
        table_ident,
        where_parts.join(" AND ")
    );
    let q = sqlx::query(&sql);
    let q = bind_cells(q, &b.pk_values);
    match q.fetch_optional(executor).await {
        Ok(Some(row)) => {
            let cols_meta = columns_of(&row);
            Some(decode_row(&row, &cols_meta))
        }
        _ => None,
    }
}

/// Submit pending changes atomically. In-tx submits run on the sticky
/// connection and append to the existing history entry; out-of-tx submits
/// open an implicit transaction and create a new history entry. A single
/// conflict stops the batch (rollback for out-of-tx; just stop for in-tx).
#[tauri::command]
pub async fn submit_pending_changes(
    registry: State<'_, ConnectionRegistry>,
    store: State<'_, StateStore>,
    connection_id: String,
    batches: Vec<PendingBatch>,
    mode: ConflictMode,
) -> TuskResult<SubmitResponse> {
    let active = registry.handle(&connection_id)?;

    let mut slot = active.tx_slot.lock().await;
    if slot.is_some() {
        // ----- In-tx path: execute on the sticky conn, append history -----
        let sticky = slot.as_mut().expect("checked");
        let entry_id = sticky.history_entry_id.clone();
        let mut results: Vec<BatchResult> = Vec::with_capacity(batches.len());
        let mut stop = false;
        for b in &batches {
            if stop {
                break;
            }
            let built = match build_for_batch(b, mode) {
                Ok(v) => v,
                Err(e) => {
                    results.push(BatchResult::Error {
                        batch_id: b.batch_id.clone(),
                        executed_sql: String::new(),
                        message: e.to_string(),
                    });
                    stop = true;
                    continue;
                }
            };
            let started = std::time::Instant::now();
            let q = sqlx::query(&built.parameterized_sql);
            let q = bind_cells(q, &built.binds);
            let r = q.execute(&mut *sticky.conn).await;
            let duration_ms = started.elapsed().as_millis() as i64;
            sticky.statement_count += 1;
            let ordinal = (sticky.statement_count.saturating_sub(1)) as i64;
            match r {
                Ok(done) => {
                    let affected = done.rows_affected();
                    if affected == 0 && b.op != PendingOp::Insert {
                        let current = fetch_current(&mut *sticky.conn, b)
                            .await
                            .unwrap_or_default();
                        let _ = store.append_history_statement(&HistoryStatement {
                            id: uuid::Uuid::new_v4().to_string(),
                            entry_id: entry_id.clone(),
                            ordinal,
                            sql: built.preview_sql.chars().take(2000).collect(),
                            duration_ms,
                            row_count: Some(0),
                            status: "error".into(),
                            error_message: Some("conflict".into()),
                        });
                        results.push(BatchResult::Conflict {
                            batch_id: b.batch_id.clone(),
                            executed_sql: built.preview_sql,
                            current,
                        });
                        stop = true;
                    } else {
                        let _ = store.append_history_statement(&HistoryStatement {
                            id: uuid::Uuid::new_v4().to_string(),
                            entry_id: entry_id.clone(),
                            ordinal,
                            sql: built.preview_sql.chars().take(2000).collect(),
                            duration_ms,
                            row_count: Some(affected as i64),
                            status: "ok".into(),
                            error_message: None,
                        });
                        results.push(BatchResult::Ok {
                            batch_id: b.batch_id.clone(),
                            affected,
                            executed_sql: built.preview_sql,
                        });
                    }
                }
                Err(e) => {
                    let msg = e.to_string();
                    let _ = store.append_history_statement(&HistoryStatement {
                        id: uuid::Uuid::new_v4().to_string(),
                        entry_id: entry_id.clone(),
                        ordinal,
                        sql: built.preview_sql.chars().take(2000).collect(),
                        duration_ms,
                        row_count: None,
                        status: "error".into(),
                        error_message: Some(msg.clone()),
                    });
                    results.push(BatchResult::Error {
                        batch_id: b.batch_id.clone(),
                        executed_sql: built.preview_sql,
                        message: msg,
                    });
                    stop = true;
                }
            }
        }
        return Ok(SubmitResponse { batches: results });
    }

    // ----- Out-of-tx path: implicit transaction with rollback on conflict ----
    drop(slot);

    let entry_id = uuid::Uuid::new_v4().to_string();
    let started_at_ms = chrono::Utc::now().timestamp_millis();
    let preview_for_entry = format!("[submit {} batch(es)]", batches.len());
    store.insert_history_entry(&HistoryEntry {
        id: entry_id.clone(),
        conn_id: connection_id.clone(),
        source: "inline".into(),
        tx_id: None,
        sql_preview: preview_for_entry,
        sql_full: None,
        started_at: started_at_ms,
        duration_ms: 0,
        row_count: None,
        status: "open".into(),
        error_message: None,
        statement_count: 0,
    })?;

    let started = std::time::Instant::now();
    let mut tx = active
        .pool
        .begin()
        .await
        .map_err(|e| TuskError::Editing(e.to_string()))?;

    let mut results: Vec<BatchResult> = Vec::with_capacity(batches.len());
    let mut conflict_or_error = false;
    let mut stmt_count: i64 = 0;
    let mut total_affected: i64 = 0;

    for b in &batches {
        if conflict_or_error {
            break;
        }
        let built = match build_for_batch(b, mode) {
            Ok(v) => v,
            Err(e) => {
                results.push(BatchResult::Error {
                    batch_id: b.batch_id.clone(),
                    executed_sql: String::new(),
                    message: e.to_string(),
                });
                conflict_or_error = true;
                continue;
            }
        };
        let stmt_started = std::time::Instant::now();
        let q = sqlx::query(&built.parameterized_sql);
        let q = bind_cells(q, &built.binds);
        let r = q.execute(&mut *tx).await;
        let duration_ms = stmt_started.elapsed().as_millis() as i64;
        let ordinal = stmt_count;
        stmt_count += 1;
        match r {
            Ok(done) => {
                let affected = done.rows_affected();
                if affected == 0 && b.op != PendingOp::Insert {
                    let current = fetch_current(&mut *tx, b).await.unwrap_or_default();
                    let _ = store.append_history_statement(&HistoryStatement {
                        id: uuid::Uuid::new_v4().to_string(),
                        entry_id: entry_id.clone(),
                        ordinal,
                        sql: built.preview_sql.chars().take(2000).collect(),
                        duration_ms,
                        row_count: Some(0),
                        status: "error".into(),
                        error_message: Some("conflict".into()),
                    });
                    results.push(BatchResult::Conflict {
                        batch_id: b.batch_id.clone(),
                        executed_sql: built.preview_sql,
                        current,
                    });
                    conflict_or_error = true;
                } else {
                    total_affected += affected as i64;
                    let _ = store.append_history_statement(&HistoryStatement {
                        id: uuid::Uuid::new_v4().to_string(),
                        entry_id: entry_id.clone(),
                        ordinal,
                        sql: built.preview_sql.chars().take(2000).collect(),
                        duration_ms,
                        row_count: Some(affected as i64),
                        status: "ok".into(),
                        error_message: None,
                    });
                    results.push(BatchResult::Ok {
                        batch_id: b.batch_id.clone(),
                        affected,
                        executed_sql: built.preview_sql,
                    });
                }
            }
            Err(e) => {
                let msg = e.to_string();
                let _ = store.append_history_statement(&HistoryStatement {
                    id: uuid::Uuid::new_v4().to_string(),
                    entry_id: entry_id.clone(),
                    ordinal,
                    sql: built.preview_sql.chars().take(2000).collect(),
                    duration_ms,
                    row_count: None,
                    status: "error".into(),
                    error_message: Some(msg.clone()),
                });
                results.push(BatchResult::Error {
                    batch_id: b.batch_id.clone(),
                    executed_sql: built.preview_sql,
                    message: msg,
                });
                conflict_or_error = true;
            }
        }
    }

    let total_duration = started.elapsed().as_millis() as i64;
    let final_status = if conflict_or_error {
        if let Err(e) = tx.rollback().await {
            eprintln!("submit_pending_changes: rollback failed: {e}");
        }
        "rolled_back"
    } else {
        if let Err(e) = tx.commit().await {
            let msg = e.to_string();
            // Surface commit failure as a final Error result if not already.
            if results
                .last()
                .map(|r| !matches!(r, BatchResult::Error { .. }))
                .unwrap_or(true)
            {
                results.push(BatchResult::Error {
                    batch_id: String::new(),
                    executed_sql: "COMMIT".into(),
                    message: msg.clone(),
                });
            }
            store.update_history_entry_finalize(
                &entry_id,
                total_duration,
                Some(total_affected),
                "error",
                Some(&msg),
                stmt_count,
            )?;
            return Ok(SubmitResponse { batches: results });
        }
        "ok"
    };
    store.update_history_entry_finalize(
        &entry_id,
        total_duration,
        Some(total_affected),
        final_status,
        None,
        stmt_count,
    )?;

    Ok(SubmitResponse { batches: results })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch_update_simple() -> PendingBatch {
        PendingBatch {
            batch_id: "b1".into(),
            op: PendingOp::Update,
            table: TableRef {
                schema: "public".into(),
                name: "users".into(),
            },
            pk_columns: vec!["id".into()],
            pk_values: vec![Cell::Int(42)],
            edits: vec![ColumnEdit {
                column: "email".into(),
                next: Cell::Text("new@x".into()),
            }],
            captured_row: vec![Cell::Int(42), Cell::Text("old@x".into()), Cell::Bool(true)],
            captured_columns: vec!["id".into(), "email".into(), "active".into()],
        }
    }

    #[test]
    fn build_update_pk_only_no_strict_clauses() {
        let built = build_update(&batch_update_simple(), ConflictMode::PkOnly).unwrap();
        assert_eq!(
            built.parameterized_sql,
            "UPDATE \"public\".\"users\" SET \"email\" = $1 WHERE \"id\" IS NOT DISTINCT FROM $2"
        );
        assert_eq!(built.binds.len(), 2);
        assert!(built.preview_sql.contains("'new@x'"));
        assert!(built.preview_sql.contains("42"));
        assert!(!built.preview_sql.contains("\"active\""));
    }

    #[test]
    fn build_update_strict_adds_captured_clauses() {
        let built = build_update(&batch_update_simple(), ConflictMode::Strict).unwrap();
        // Strict mode adds IS NOT DISTINCT FROM for every non-PK non-float column,
        // INCLUDING the edited column (with its ORIGINAL captured value, not the new one).
        // This is the lost-update detection invariant: a concurrent change to the
        // edited column will produce affected=0.
        assert!(
            built
                .parameterized_sql
                .contains("\"email\" IS NOT DISTINCT FROM"),
            "Strict mode should include edited column with original value:\n{}",
            built.parameterized_sql
        );
        assert!(
            built
                .parameterized_sql
                .contains("\"active\" IS NOT DISTINCT FROM"),
            "Strict mode should include non-edited captured column:\n{}",
            built.parameterized_sql
        );
        // 4 binds: SET email='new@x' + PK id=42 + WHERE email='old@x' (orig) + WHERE active=true
        assert_eq!(built.binds.len(), 4);
        // Preview should reference both the new email value (in SET) and the original (in WHERE).
        assert!(built.preview_sql.contains("'new@x'"));
        assert!(built.preview_sql.contains("'old@x'"));
    }

    #[test]
    fn build_insert_uses_value_list() {
        let mut b = batch_update_simple();
        b.op = PendingOp::Insert;
        let built = build_insert(&b).unwrap();
        assert_eq!(
            built.parameterized_sql,
            "INSERT INTO \"public\".\"users\" (\"email\") VALUES ($1)"
        );
    }

    #[test]
    fn build_delete_pk_only() {
        let mut b = batch_update_simple();
        b.op = PendingOp::Delete;
        let built = build_delete(&b, ConflictMode::PkOnly).unwrap();
        assert_eq!(
            built.parameterized_sql,
            "DELETE FROM \"public\".\"users\" WHERE \"id\" IS NOT DISTINCT FROM $1"
        );
    }
}
