//! `run_explain` Tauri command.
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::{Emitter, State};

use crate::commands::sqlast::{classify_for_explain, ExplainCategory};
use crate::db::explain_runner::{
    category_to_exec_mode, run_wrapped_explain, wrap_for_explain, ExplainExecMode,
};
use crate::db::pool::ConnectionRegistry;
use crate::db::state::{HistoryEntry, StateStore};
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunExplainArgs {
    pub connection_id: String,
    pub sql: String,
    #[serde(default)]
    pub allow_analyze_anyway: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplainResult {
    pub mode: &'static str,
    pub plan_json: JsonValue,
    pub warnings: Vec<String>,
    pub verified_candidates: Vec<JsonValue>,
    /// Wall-clock time of the EXPLAIN call as measured inside `run_wrapped_explain`.
    /// This excludes command dispatch, SQL classification, and history-write overhead;
    /// the history entry's `duration_ms` captures the full command duration.
    pub total_ms: f64,
    pub executed_at: i64,
}

#[tauri::command]
pub async fn run_explain(
    app_handle: tauri::AppHandle,
    registry: State<'_, ConnectionRegistry>,
    store: State<'_, StateStore>,
    args: RunExplainArgs,
) -> TuskResult<ExplainResult> {
    let RunExplainArgs {
        connection_id,
        sql,
        allow_analyze_anyway,
    } = args;

    let active = registry.handle(&connection_id)?;
    let started = Instant::now();
    // Capture start time before execution so history `started_at` is the true start.
    let started_ms = chrono::Utc::now().timestamp_millis();

    let category = classify_for_explain(&sql);
    let exec_mode = category_to_exec_mode(category, allow_analyze_anyway).ok_or_else(|| {
        TuskError::Explain(match category {
            ExplainCategory::Unparseable => "SQL could not be parsed".into(),
            ExplainCategory::NotExplainable => "Statement is not explainable".into(),
            _ => "Cannot run EXPLAIN on this SQL".into(),
        })
    })?;

    let mut warnings: Vec<String> = Vec::new();
    if sql
        .trim_start()
        .split(';')
        .filter(|s| !s.trim().is_empty())
        .count()
        > 1
    {
        warnings
            .push("Only the first statement is explained; subsequent statements ignored".into());
    }

    let wrapped = wrap_for_explain(&sql, exec_mode);

    // Emit started before execution. pid is -1 because the connection (and its
    // backend PID) isn't acquired until run_wrapped_explain runs. Cancel-during-EXPLAIN
    // is not a v1 feature, so the placeholder is safe.
    let _ = app_handle.emit(
        "query:started",
        serde_json::json!({
            "connId": connection_id,
            "pid": -1_i32,
            "startedAt": started_ms,
        }),
    );

    let output = match run_wrapped_explain(&active, &wrapped, exec_mode).await {
        Ok(out) => {
            let _ = app_handle.emit(
                "query:completed",
                serde_json::json!({
                    "connId": connection_id,
                    "pid": out.pid,
                    "ok": true,
                }),
            );
            out
        }
        Err(e) => {
            let _ = app_handle.emit(
                "query:completed",
                serde_json::json!({
                    "connId": connection_id,
                    "pid": -1_i32,
                    "ok": false,
                }),
            );
            return Err(e);
        }
    };

    let entry_id = uuid::Uuid::new_v4().to_string();
    let preview: String = wrapped.chars().take(200).collect();
    let duration_ms = started.elapsed().as_millis() as i64;
    if let Err(e) = store.insert_history_entry(&HistoryEntry {
        id: entry_id,
        conn_id: connection_id.clone(),
        source: "editor".into(),
        tx_id: None,
        sql_preview: preview,
        sql_full: Some(wrapped.clone()),
        started_at: started_ms,
        duration_ms,
        row_count: None,
        status: "ok".into(),
        error_message: None,
        statement_count: 1,
    }) {
        eprintln!("failed to record explain history: {e}");
    }

    let mode = match (exec_mode, output.container) {
        (ExplainExecMode::SelectAnalyze, _) => "select-analyze",
        (ExplainExecMode::PlanOnly, _) => match category {
            ExplainCategory::DmlPlanOnly => "dml-plan-only",
            ExplainCategory::DdlPlanOnly => "ddl-plan-only",
            // category_to_exec_mode only yields PlanOnly for DML/DDL; this is defensive.
            _ => "dml-plan-only",
        },
        (ExplainExecMode::AnalyzeAnyway, "rolled-back") => "analyze-anyway-rolled-back",
        (ExplainExecMode::AnalyzeAnyway, "in-tx") => "analyze-anyway-in-tx",
        (ExplainExecMode::AnalyzeAnyway, _) => "analyze-anyway-rolled-back",
        (ExplainExecMode::Passthrough, _) => "passthrough",
    };

    Ok(ExplainResult {
        mode,
        plan_json: output.plan_value,
        warnings,
        verified_candidates: Vec::new(),
        total_ms: output.total_ms,
        executed_at: started_ms,
    })
}
