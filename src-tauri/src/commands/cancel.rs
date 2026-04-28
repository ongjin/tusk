// src-tauri/src/commands/cancel.rs
use tauri::State;

use crate::db::pool::ConnectionRegistry;
use crate::errors::{TuskError, TuskResult};

/// Issues `pg_cancel_backend(pid)` on a separate session checked out of the
/// existing pool. Reusing the pool means we automatically inherit the same
/// auth + tunnel routing as the victim query — no need to materialise the
/// connection URL just to spin a fresh single-connection pool.
#[tauri::command]
pub async fn cancel_query(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
    pid: i32,
) -> TuskResult<bool> {
    let active = registry.handle(&connection_id)?;
    let mut conn = active
        .pool
        .acquire()
        .await
        .map_err(|e| TuskError::Tx(format!("cancel pool: {e}")))?;
    let cancelled: bool = sqlx::query_scalar("SELECT pg_cancel_backend($1)")
        .bind(pid)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| TuskError::Tx(format!("pg_cancel_backend: {e}")))?;
    Ok(cancelled)
}
