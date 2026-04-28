use serde::Serialize;
use tauri::State;

use crate::db::pool::{ConnectionRegistry, StickyTx};
use crate::db::state::{HistoryEntry, StateStore};
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TxStateSnapshot {
    pub conn_id: String,
    pub active: bool,
    pub tx_id: Option<String>,
    pub started_at: Option<i64>,
    pub statement_count: u32,
    pub pid: Option<i32>,
}

#[tauri::command]
pub async fn tx_begin(
    registry: State<'_, ConnectionRegistry>,
    store: State<'_, StateStore>,
    connection_id: String,
) -> TuskResult<TxStateSnapshot> {
    let active = registry.handle(&connection_id)?;
    // Hold the slot lock across the whole begin flow so concurrent calls
    // can't both run BEGIN and clobber each other.
    let mut slot = active.tx_slot.lock().await;
    if slot.is_some() {
        return Err(TuskError::Tx("transaction already active".into()));
    }
    let mut conn = active
        .pool
        .acquire()
        .await
        .map_err(|e| TuskError::Tx(e.to_string()))?;
    sqlx::query("BEGIN")
        .execute(&mut *conn)
        .await
        .map_err(|e| TuskError::Tx(e.to_string()))?;
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| TuskError::Tx(e.to_string()))?;
    let tx_id = uuid::Uuid::new_v4().to_string();
    let entry_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    store.insert_history_entry(&HistoryEntry {
        id: entry_id.clone(),
        conn_id: connection_id.clone(),
        source: "editor".into(),
        tx_id: Some(tx_id.clone()),
        sql_preview: format!("[transaction {}]", &tx_id[..8]),
        sql_full: None,
        started_at: now,
        duration_ms: 0,
        row_count: None,
        status: "open".into(),
        error_message: None,
        statement_count: 0,
    })?;
    *slot = Some(StickyTx {
        tx_id: tx_id.clone(),
        conn,
        started_at: std::time::Instant::now(),
        backend_pid: pid,
        statement_count: 0,
        history_entry_id: entry_id,
    });
    Ok(TxStateSnapshot {
        conn_id: connection_id,
        active: true,
        tx_id: Some(tx_id),
        started_at: Some(now),
        statement_count: 0,
        pid: Some(pid),
    })
}

#[tauri::command]
pub async fn tx_commit(
    registry: State<'_, ConnectionRegistry>,
    store: State<'_, StateStore>,
    connection_id: String,
) -> TuskResult<TxStateSnapshot> {
    finalize_tx(&registry, &store, &connection_id, "COMMIT", "ok").await
}

#[tauri::command]
pub async fn tx_rollback(
    registry: State<'_, ConnectionRegistry>,
    store: State<'_, StateStore>,
    connection_id: String,
) -> TuskResult<TxStateSnapshot> {
    finalize_tx(&registry, &store, &connection_id, "ROLLBACK", "rolled_back").await
}

async fn finalize_tx(
    registry: &State<'_, ConnectionRegistry>,
    store: &State<'_, StateStore>,
    connection_id: &str,
    sql: &str,
    final_status: &str,
) -> TuskResult<TxStateSnapshot> {
    let active = registry.handle(connection_id)?;
    let mut sticky = {
        let mut slot = active.tx_slot.lock().await;
        slot.take()
            .ok_or_else(|| TuskError::Tx("no active transaction".into()))?
    };
    let res = sqlx::query(sql).execute(&mut *sticky.conn).await;
    let duration_ms = sticky.started_at.elapsed().as_millis() as i64;
    let (status, err) = match res {
        Ok(_) => (final_status.to_string(), None),
        Err(e) => ("error".to_string(), Some(e.to_string())),
    };
    store.update_history_entry_finalize(
        &sticky.history_entry_id,
        duration_ms,
        None,
        &status,
        err.as_deref(),
        sticky.statement_count as i64,
    )?;
    Ok(TxStateSnapshot {
        conn_id: connection_id.to_string(),
        active: false,
        tx_id: Some(sticky.tx_id),
        started_at: None,
        statement_count: sticky.statement_count,
        pid: None,
    })
}
