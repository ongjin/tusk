// src-tauri/src/commands/history.rs
use tauri::State;

use crate::db::state::{HistoryEntry, HistoryStatement, StateStore};
use crate::errors::TuskResult;

#[tauri::command]
pub async fn list_history(
    store: State<'_, StateStore>,
    connection_id: Option<String>,
    query: Option<String>,
    limit: Option<i64>,
) -> TuskResult<Vec<HistoryEntry>> {
    store.list_history(
        connection_id.as_deref(),
        query.as_deref(),
        limit.unwrap_or(200),
    )
}

#[tauri::command]
pub async fn list_history_statements(
    store: State<'_, StateStore>,
    entry_id: String,
) -> TuskResult<Vec<HistoryStatement>> {
    store.list_history_statements(&entry_id)
}
