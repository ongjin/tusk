// src-tauri/src/commands/history.rs
use serde::Deserialize;
use tauri::State;

use crate::db::state::{HistoryEntry, HistoryStatement, StateStore};
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiGenerationPayload {
    pub conn_id: String,
    pub prompt: String,
    pub generated_sql: String,
    pub provider: String,
    pub generation_model: String,
    pub embedding_model: Option<String>,
    pub top_k_tables: Vec<String>,
    pub tool_calls: serde_json::Value,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub duration_ms: i64,
}

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

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn list_recent_successful(
    store: State<'_, StateStore>,
    connection_id: String,
    limit: i64,
) -> TuskResult<Vec<HistoryEntry>> {
    store.list_recent_successful(&connection_id, limit)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn record_ai_generation(
    store: State<'_, StateStore>,
    payload: AiGenerationPayload,
) -> TuskResult<String> {
    store
        .insert_ai_generation(payload)
        .map_err(|e| TuskError::History(e.to_string()))
}
