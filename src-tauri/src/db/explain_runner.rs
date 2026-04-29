//! Wrap a user SQL string in EXPLAIN with the right options and run it.
use serde_json::Value as JsonValue;

use crate::commands::sqlast::ExplainCategory;
use crate::db::pool::ActiveConnection;
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplainExecMode {
    SelectAnalyze,
    PlanOnly,
    Passthrough,
    AnalyzeAnyway,
}

pub fn wrap_for_explain(sql: &str, mode: ExplainExecMode) -> String {
    let trimmed = sql.trim().trim_end_matches(';');
    match mode {
        ExplainExecMode::SelectAnalyze | ExplainExecMode::AnalyzeAnyway => {
            format!("EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {trimmed}")
        }
        ExplainExecMode::PlanOnly => {
            format!("EXPLAIN (FORMAT JSON) {trimmed}")
        }
        ExplainExecMode::Passthrough => trimmed.to_string(),
    }
}

pub fn category_to_exec_mode(
    c: ExplainCategory,
    allow_analyze_anyway: bool,
) -> Option<ExplainExecMode> {
    Some(match c {
        ExplainCategory::SelectAnalyze => ExplainExecMode::SelectAnalyze,
        ExplainCategory::DmlPlanOnly | ExplainCategory::DdlPlanOnly => {
            if allow_analyze_anyway {
                ExplainExecMode::AnalyzeAnyway
            } else {
                ExplainExecMode::PlanOnly
            }
        }
        ExplainCategory::Passthrough => ExplainExecMode::Passthrough,
        ExplainCategory::Unparseable | ExplainCategory::NotExplainable => return None,
    })
}

#[derive(Debug)]
pub struct ExplainOutput {
    pub plan_value: JsonValue,
    pub total_ms: f64,
    pub pid: i32,
    pub container: &'static str,
}

pub async fn run_wrapped_explain(
    active: &ActiveConnection,
    wrapped_sql: &str,
    mode: ExplainExecMode,
) -> TuskResult<ExplainOutput> {
    let started = std::time::Instant::now();
    let mut slot_guard = active.tx_slot.lock().await;
    let in_tx = slot_guard.is_some();

    if in_tx {
        let sticky = slot_guard.as_mut().expect("checked");
        let pid = sticky.backend_pid;
        let row: (JsonValue,) = sqlx::query_as(wrapped_sql)
            .fetch_one(&mut *sticky.conn)
            .await
            .map_err(|e| TuskError::Explain(e.to_string()))?;
        Ok(ExplainOutput {
            plan_value: extract_first(row.0)?,
            total_ms: started.elapsed().as_secs_f64() * 1000.0,
            pid,
            container: "in-tx",
        })
    } else {
        drop(slot_guard);
        let mut conn = active
            .pool
            .acquire()
            .await
            .map_err(|e| TuskError::Explain(e.to_string()))?;
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *conn)
            .await
            .unwrap_or(-1);

        let (plan_value, container) = if matches!(mode, ExplainExecMode::AnalyzeAnyway) {
            run_explain_with_rollback(&mut conn, wrapped_sql).await?
        } else {
            let row: (JsonValue,) = sqlx::query_as(wrapped_sql)
                .fetch_one(&mut *conn)
                .await
                .map_err(|e| TuskError::Explain(e.to_string()))?;
            (extract_first(row.0)?, "pool")
        };

        Ok(ExplainOutput {
            plan_value,
            total_ms: started.elapsed().as_secs_f64() * 1000.0,
            pid,
            container,
        })
    }
}

async fn run_explain_with_rollback(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
    wrapped_sql: &str,
) -> TuskResult<(JsonValue, &'static str)> {
    sqlx::query("BEGIN")
        .fetch_all(&mut **conn)
        .await
        .map_err(|e| TuskError::Explain(format!("BEGIN failed: {e}")))?;
    let result: Result<(JsonValue,), sqlx::Error> =
        sqlx::query_as(wrapped_sql).fetch_one(&mut **conn).await;
    let rb = sqlx::query("ROLLBACK").fetch_all(&mut **conn).await;
    let plan = result.map_err(|e| TuskError::Explain(e.to_string()))?;
    rb.map_err(|e| TuskError::Explain(format!("ROLLBACK failed: {e}")))?;
    Ok((extract_first(plan.0)?, "rolled-back"))
}

fn extract_first(v: JsonValue) -> TuskResult<JsonValue> {
    match v {
        JsonValue::Array(mut arr) => {
            if arr.is_empty() {
                Err(TuskError::Explain("EXPLAIN returned empty array".into()))
            } else {
                Ok(arr.remove(0))
            }
        }
        JsonValue::String(s) => serde_json::from_str(&s)
            .map_err(|e| TuskError::Explain(format!("EXPLAIN JSON parse failed: {e}"))),
        other => Err(TuskError::Explain(format!(
            "Unexpected EXPLAIN shape: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_select_analyze() {
        assert_eq!(
            wrap_for_explain("SELECT 1", ExplainExecMode::SelectAnalyze),
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) SELECT 1"
        );
    }

    #[test]
    fn wrap_plan_only_strips_trailing_semi() {
        assert_eq!(
            wrap_for_explain("UPDATE t SET a=1;", ExplainExecMode::PlanOnly),
            "EXPLAIN (FORMAT JSON) UPDATE t SET a=1"
        );
    }

    #[test]
    fn passthrough_unchanged_other_than_trim() {
        assert_eq!(
            wrap_for_explain("  EXPLAIN SELECT 1;  ", ExplainExecMode::Passthrough),
            "EXPLAIN SELECT 1"
        );
    }

    #[test]
    fn analyze_anyway_uses_full_options() {
        assert_eq!(
            wrap_for_explain("DELETE FROM t", ExplainExecMode::AnalyzeAnyway),
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) DELETE FROM t"
        );
    }

    #[test]
    fn category_to_exec_mode_basic() {
        assert_eq!(
            category_to_exec_mode(ExplainCategory::SelectAnalyze, false),
            Some(ExplainExecMode::SelectAnalyze)
        );
        assert_eq!(
            category_to_exec_mode(ExplainCategory::DmlPlanOnly, false),
            Some(ExplainExecMode::PlanOnly)
        );
        assert_eq!(
            category_to_exec_mode(ExplainCategory::DmlPlanOnly, true),
            Some(ExplainExecMode::AnalyzeAnyway)
        );
        assert_eq!(
            category_to_exec_mode(ExplainCategory::Passthrough, false),
            Some(ExplainExecMode::Passthrough)
        );
        assert_eq!(
            category_to_exec_mode(ExplainCategory::Unparseable, true),
            None
        );
    }
}
