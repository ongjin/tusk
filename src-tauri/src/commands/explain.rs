//! `run_explain` Tauri command.
use std::collections::BTreeSet;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::{Emitter, State};

use crate::commands::sqlast::{classify_for_explain, ExplainCategory};
use crate::db::explain_runner::{
    category_to_exec_mode, run_wrapped_explain, wrap_for_explain, ExplainExecMode,
};
use crate::db::pg_stats::{fetch_column_stats, ColumnRef};
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

    let candidates: Vec<JsonValue> =
        match extract_index_candidates(&active.pool, &output.plan_value).await {
            Ok(list) => list
                .into_iter()
                .map(|c| serde_json::to_value(&c).unwrap_or(JsonValue::Null))
                .collect(),
            Err(e) => {
                warnings.push(format!("Index candidate extraction failed: {e}"));
                Vec::new()
            }
        };

    Ok(ExplainResult {
        mode,
        plan_json: output.plan_value,
        warnings,
        verified_candidates: candidates,
        total_ms: output.total_ms,
        executed_at: started_ms,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexCandidate {
    pub schema: String,
    pub table: String,
    pub columns: Vec<String>,
    pub reason: &'static str,
    pub verdict: &'static str,
    pub selectivity_estimate: Option<f64>,
    pub n_distinct: Option<f64>,
    pub null_frac: Option<f64>,
}

pub async fn extract_index_candidates(
    pool: &sqlx::PgPool,
    plan_root: &JsonValue,
) -> TuskResult<Vec<IndexCandidate>> {
    let mut raw: Vec<RawCandidate> = Vec::new();
    if let Some(plan) = plan_root.get("Plan") {
        walk(plan, &mut raw);
    }
    if raw.is_empty() {
        return Ok(Vec::new());
    }

    let mut seen: BTreeSet<(String, String, String)> = BTreeSet::new();
    raw.retain(|r| seen.insert((r.schema.clone(), r.table.clone(), r.column.clone())));

    let refs: Vec<ColumnRef> = raw
        .iter()
        .map(|r| ColumnRef {
            schema: r.schema.clone(),
            table: r.table.clone(),
            column: r.column.clone(),
        })
        .collect();

    let stats_map = fetch_column_stats(pool, &refs).await.unwrap_or_default();

    let mut out = Vec::with_capacity(raw.len());
    for r in raw {
        let stats = stats_map
            .get(&(r.schema.clone(), r.table.clone(), r.column.clone()))
            .cloned();
        let n_distinct = stats.as_ref().and_then(|s| s.n_distinct);
        let null_frac = stats.as_ref().and_then(|s| s.null_frac);

        let (verdict, selectivity) = match n_distinct {
            Some(v) if v > 0.0 => {
                let sel = 1.0 / v;
                if sel <= 0.05 {
                    ("likely", Some(sel))
                } else if sel <= 0.20 {
                    ("maybe", Some(sel))
                } else {
                    continue;
                }
            }
            Some(v) if v < 0.0 => {
                let sel = v.abs();
                if sel <= 0.05 {
                    ("likely", Some(sel))
                } else if sel <= 0.20 {
                    ("maybe", Some(sel))
                } else {
                    continue;
                }
            }
            _ => ("maybe", None),
        };

        out.push(IndexCandidate {
            schema: r.schema,
            table: r.table,
            columns: vec![r.column],
            reason: r.reason,
            verdict,
            selectivity_estimate: selectivity,
            n_distinct,
            null_frac,
        });
    }
    Ok(out)
}

#[derive(Debug, Clone)]
struct RawCandidate {
    schema: String,
    table: String,
    column: String,
    reason: &'static str,
}

fn walk(node: &JsonValue, out: &mut Vec<RawCandidate>) {
    let node_type = node.get("Node Type").and_then(|v| v.as_str()).unwrap_or("");
    let schema = node
        .get("Schema")
        .and_then(|v| v.as_str())
        .unwrap_or("public")
        .to_string();
    let relation = node
        .get("Relation Name")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    if node_type == "Seq Scan" {
        let rows_removed = node
            .get("Rows Removed by Filter")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let actual_rows = node
            .get("Actual Rows")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let scanned = actual_rows + rows_removed;
        let big_filter = rows_removed >= 1000;
        let dense = scanned > 0 && (actual_rows as f64) >= 0.5 * (scanned as f64);
        if (big_filter || dense) && relation.is_some() {
            if let Some(filter) = node.get("Filter").and_then(|v| v.as_str()) {
                for col in extract_simple_columns(filter) {
                    out.push(RawCandidate {
                        schema: schema.clone(),
                        table: relation.clone().unwrap(),
                        column: col,
                        reason: if big_filter {
                            "rows-removed-by-filter"
                        } else {
                            "seq-scan-filter"
                        },
                    });
                }
            }
        }
    } else if node_type == "Index Scan" || node_type == "Index Only Scan" {
        let rows_removed = node
            .get("Rows Removed by Index Recheck")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if rows_removed >= 1000 {
            if let (Some(table), Some(filter)) = (
                relation.clone(),
                node.get("Filter").and_then(|v| v.as_str()),
            ) {
                for col in extract_simple_columns(filter) {
                    out.push(RawCandidate {
                        schema: schema.clone(),
                        table: table.clone(),
                        column: col,
                        reason: "lossy-index-cond",
                    });
                }
            }
        }
    }

    if let Some(children) = node.get("Plans").and_then(|v| v.as_array()) {
        for child in children {
            walk(child, out);
        }
    }
}

fn extract_simple_columns(filter: &str) -> Vec<String> {
    let mut out = Vec::new();
    for piece in filter.split([',', ';']) {
        let p = piece.trim().trim_start_matches('(').trim_end_matches(')');
        let anchor = [
            " = ",
            " IN ",
            " BETWEEN ",
            " <= ",
            " >= ",
            " < ",
            " > ",
            " <> ",
            " != ",
        ]
        .iter()
        .filter_map(|op| p.find(op))
        .min();
        if let Some(idx) = anchor {
            let lhs = p[..idx].trim();
            if is_simple_ident(lhs) {
                out.push(strip_quotes(lhs).to_string());
            }
        }
    }
    out
}

fn is_simple_ident(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let candidate = s.split('.').next_back().unwrap_or(s);
    let candidate = candidate.trim_matches('"');
    if candidate.is_empty() {
        return false;
    }
    candidate.chars().all(|c| c.is_alphanumeric() || c == '_')
}

fn strip_quotes(s: &str) -> &str {
    s.trim_matches('"').rsplit('.').next().unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn walker_picks_seq_scan_with_big_rows_removed() {
        let plan = json!({
            "Plan": {
                "Node Type": "Seq Scan",
                "Schema": "public",
                "Relation Name": "users",
                "Filter": "(email = 'foo@example.com'::text)",
                "Actual Rows": 1,
                "Rows Removed by Filter": 50000
            }
        });
        let mut raw = Vec::new();
        walk(plan.get("Plan").unwrap(), &mut raw);
        assert_eq!(raw.len(), 1);
        assert_eq!(raw[0].table, "users");
        assert_eq!(raw[0].column, "email");
        assert_eq!(raw[0].reason, "rows-removed-by-filter");
    }

    #[test]
    fn walker_skips_seq_scan_with_small_filter_and_low_density() {
        let plan = json!({
            "Plan": {
                "Node Type": "Seq Scan",
                "Schema": "public",
                "Relation Name": "users",
                "Filter": "(email = 'foo@example.com'::text)",
                "Actual Rows": 1,
                "Rows Removed by Filter": 100
            }
        });
        let mut raw = Vec::new();
        walk(plan.get("Plan").unwrap(), &mut raw);
        assert!(raw.is_empty());
    }

    #[test]
    fn walker_picks_dense_seq_scan() {
        let plan = json!({
            "Plan": {
                "Node Type": "Seq Scan",
                "Schema": "public",
                "Relation Name": "audit_log",
                "Filter": "(level = 'error')",
                "Actual Rows": 800,
                "Rows Removed by Filter": 200
            }
        });
        let mut raw = Vec::new();
        walk(plan.get("Plan").unwrap(), &mut raw);
        assert_eq!(raw.len(), 1);
        assert_eq!(raw[0].column, "level");
    }

    #[test]
    fn extract_simple_handles_qualified_and_quoted() {
        assert_eq!(
            extract_simple_columns("(u.email = 'a')"),
            vec!["email".to_string()]
        );
        assert_eq!(
            extract_simple_columns("(\"first name\" = 'a')"),
            Vec::<String>::new()
        );
        assert_eq!(extract_simple_columns("(a >= 1)"), vec!["a".to_string()]);
    }

    #[test]
    fn walker_recurses_into_children() {
        let plan = json!({
            "Plan": {
                "Node Type": "Hash Join",
                "Plans": [
                    {
                        "Node Type": "Seq Scan",
                        "Schema": "public",
                        "Relation Name": "users",
                        "Filter": "(email = 'a')",
                        "Actual Rows": 1,
                        "Rows Removed by Filter": 5000
                    }
                ]
            }
        });
        let mut raw = Vec::new();
        walk(plan.get("Plan").unwrap(), &mut raw);
        assert_eq!(raw.len(), 1);
    }
}
