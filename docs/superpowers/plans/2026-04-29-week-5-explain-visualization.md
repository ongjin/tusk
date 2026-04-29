# Week 5 — EXPLAIN Visualization + AI Interpretation + Index Recommendation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** PLAN.md Week 5 4개 sub-bullet (EXPLAIN ANALYZE 트리 시각화 / 노드 클릭 → 상세 패널 / AI 해석 사이드바 / 인덱스 추천)을 한 주에 모두 구현. AI 키 없이도 결정적 인덱스 추천이 동작하고, BYOK 정신에 맞춰 LLM 해석은 사용자가 선택해서 호출한다.

**Architecture:** Frontend는 `features/explain/*`에 새 모듈 군 + `Cmd+Shift+E`/Explain 버튼을 `EditorPane`에 추가. `Tab.lastPlan`이 `Tab.lastResult`와 sticky하게 공존하고 `[Rows | Plan]` 토글로 왕복. Rust는 `commands/explain.rs`(run_explain) + `db/explain_runner.rs`(EXPLAIN 실행) + `db/pg_stats.rs`(카디널리티 조회) + 신규 migration 004로 `ai_explain` 테이블 추가. ANALYZE-anyway 경로만 `runGate`(Week 4 destructive) 통과 + 비-tx 환경에선 `BEGIN ... ROLLBACK` 래핑.

**Tech Stack:**

- 신규 의존성: **없음**. 기존 `sqlx 0.8`, `serde_json`, `sqlparser 0.52`, `rusqlite 0.32`, AI SDK 6.x, Web Crypto `subtle.digest` 만 사용.
- Plan 트리는 들여쓰기 텍스트 + div width%로 self-time bar — d3/react-flow 의도적으로 회피.

**Reference spec:** `docs/superpowers/specs/2026-04-29-week-5-explain-visualization-design.md`.

**Working dir:** `/Users/cyj/workspace/personal/tusk` on `main`.

**Branching:** Week 4 implementation이 이미 main에 머지됨(commit `9d6d1cf` 등). 사용자 지시에 따라 main에서 직접 작업.

**Quality gates between tasks:**

```
pnpm typecheck && pnpm lint && pnpm format:check
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
cargo test --manifest-path src-tauri/Cargo.toml
```

Run only the gates relevant to the task (Rust task → rust:\* + cargo test; Frontend task → typecheck/lint/format + `pnpm build`). Final task runs the full set including docker integration tests.

**Integration tests with docker postgres:** `infra/postgres/docker-compose.yml`. Connection: `postgres://tusk:tusk@127.0.0.1:55432/tusk_test`. Same env as Week 2~4.

**Commit message convention:** Conventional commits (`feat`, `fix`, `chore`, `docs`, `refactor`, `test`). **Do NOT add `Co-Authored-By` trailers or "Generated with ..." lines.** Commit messages describe the change, nothing else.

---

## File structure (created during this plan)

```
src-tauri/src/
  commands/
    explain.rs            (T4)  — run_explain, candidate extractor
    sqlast.rs             (mod — T2) — classify_for_explain
    history.rs            (mod — T7) — record_ai_explain
    mod.rs                (mod — T4) — pub mod explain
  db/
    explain_runner.rs     (T3)  — wrapped EXPLAIN sqlx 실행 (with BEGIN/ROLLBACK option)
    pg_stats.rs           (T5)  — fetch_column_stats (n_distinct, null_frac)
    state.rs              (mod — T7) — migration 004_ai_explain, insert_ai_explain
    mod.rs                (mod — T3, T5) — pub mod explain_runner / pg_stats
  errors.rs               (mod — T1) — Explain variant
  lib.rs                  (mod — T4, T7) — invoke_handler 추가

src/
  lib/
    explain/
      planTypes.ts        (T9)  — PlanNode, ExplainResult, IndexCandidate, ExplainMode
      planParse.ts        (T9)  — raw EXPLAIN JSON → PlanNode 트리
      planSha.ts          (T9)  — Web Crypto subtle.digest 기반 안정적 plan SHA
    ai/
      explainPrompts.ts   (T16) — SYSTEM_EXPLAIN_PROMPT + user prompt builder + token budget
      explainStream.ts    (T16) — streamText 래퍼 + summary/recommendations 파싱
  store/
    tabs.ts               (mod — T10) — Tab.lastPlan, Tab.resultMode
    settings.ts           (mod — T10) — autoInterpretPlan, indexAdviceEnabled, explainTokenBudget
  features/
    explain/
      explainGate.ts        (T11) — invoke('run_explain') 래퍼
      ExplainView.tsx       (T12) — [Rows|Plan] 토글 + 빈 상태
      PlanTree.tsx          (T13) — 들여쓰기 트리 + self-time bar
      PlanNodeDetail.tsx    (T14) — 선택 노드 상세 패널
      IndexCandidates.tsx   (T15) — verified candidates + Insert into editor
      PlanAiStrip.tsx       (T17) — AI 해석 + 캐시
      AnalyzeAnywayButton.tsx (T19) — ANALYZE-anyway + runGate
    editor/
      EditorPane.tsx      (mod — T18) — Cmd+Shift+E + Explain 버튼
    results/
      ResultsHeader.tsx   (mod — T12) — Rows/Plan 모드 토글

docs/superpowers/plans/
  2026-04-29-week-5-explain-visualization.md
  manual-verification-week-5.md   (T21)
```

---

## Task 0: Prerequisite verification — Week 4 on main + clean working tree

**Goal:** Week 5 작업 시작 전 main이 Week 4 완료 상태인지, 작업 트리가 clean인지, 도커 postgres가 그대로 살아있는지 검증.

**Files:** none (operational task)

**Steps:**

- [ ] **Step 1: Verify Week 4 commits present on main**

```bash
git log --oneline | head -10
```

Expected to include `9d6d1cf fix(week4): final review`, `7f974fb feat(week4): Cmd+K integration + AI history record`, etc.

- [ ] **Step 2: Verify clean working tree**

```bash
git status
```

Expected: `nothing to commit, working tree clean`.

- [ ] **Step 3: Run full quality gate baseline**

```bash
pnpm install --frozen-lockfile
pnpm typecheck && pnpm lint && pnpm format:check
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
cargo test --manifest-path src-tauri/Cargo.toml
```

All green expected. If any fail, fix before starting Week 5.

- [ ] **Step 4: Verify Week 5 spec landed**

```bash
ls docs/superpowers/specs/2026-04-29-week-5-explain-visualization-design.md
```

Should already exist (committed earlier in `707edc8`).

- [ ] **Step 5: Bring docker postgres up for integration tests**

```bash
docker compose -f infra/postgres/docker-compose.yml up -d
docker compose -f infra/postgres/docker-compose.yml ps
```

Expected: container is `healthy`. Port 55432 exposed.

- [ ] **Step 6: Sanity-check ANALYZE works on the dev DB**

```bash
PGPASSWORD=tusk psql -h 127.0.0.1 -p 55432 -U tusk -d tusk_test -c \
  "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) SELECT 1;"
```

Expected: 1 row JSON output containing `Plan`, `Planning Time`, `Execution Time`. If this fails, no point proceeding.

---

## Task 1: Foundation — error variant

**Goal:** Cross-cutting `TuskError::Explain` so later tasks can reference it without circular imports.

**Files:**

- Modify: `src-tauri/src/errors.rs`

**Steps:**

- [ ] **Step 1: Add `Explain` error variant**

Edit `src-tauri/src/errors.rs`. Append to the `TuskError` enum (after `Destructive*` variants):

```rust
    #[error("Explain error: {0}")]
    Explain(String),
```

- [ ] **Step 2: Verify Rust compiles**

```bash
pnpm rust:check
```

Expected: success.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/errors.rs
git commit -m "chore(week5): add TuskError::Explain variant"
```

---

## Task 2: Rust — `classify_for_explain` in `commands/sqlast.rs`

**Goal:** Decide ANALYZE vs plan-only mode for an arbitrary SQL string. Pure function, no DB access.

**Files:**

- Modify: `src-tauri/src/commands/sqlast.rs`

**Steps:**

- [ ] **Step 1: Add fixture-based unit tests** (TDD red)

Append a new `#[cfg(test)] mod explain_classifier_tests` to `src-tauri/src/commands/sqlast.rs`:

```rust
#[cfg(test)]
mod explain_classifier_tests {
    use super::*;

    fn cls(s: &str) -> ExplainCategory {
        classify_for_explain(s)
    }

    #[test]
    fn select_is_analyze() {
        assert_eq!(cls("SELECT 1"), ExplainCategory::SelectAnalyze);
        assert_eq!(cls("  select * from users  "), ExplainCategory::SelectAnalyze);
        assert_eq!(
            cls("WITH x AS (SELECT 1) SELECT * FROM x"),
            ExplainCategory::SelectAnalyze
        );
        assert_eq!(cls("VALUES (1),(2)"), ExplainCategory::SelectAnalyze);
        assert_eq!(cls("TABLE users"), ExplainCategory::SelectAnalyze);
    }

    #[test]
    fn dml_is_plan_only() {
        assert_eq!(cls("INSERT INTO t VALUES (1)"), ExplainCategory::DmlPlanOnly);
        assert_eq!(cls("UPDATE t SET a=1"), ExplainCategory::DmlPlanOnly);
        assert_eq!(cls("DELETE FROM t"), ExplainCategory::DmlPlanOnly);
        assert_eq!(
            cls("MERGE INTO t USING s ON t.id=s.id WHEN MATCHED THEN DO NOTHING"),
            ExplainCategory::DmlPlanOnly
        );
    }

    #[test]
    fn ddl_is_plan_only() {
        assert_eq!(cls("CREATE TABLE x (id int)"), ExplainCategory::DdlPlanOnly);
        assert_eq!(cls("DROP TABLE x"), ExplainCategory::DdlPlanOnly);
        assert_eq!(cls("ALTER TABLE x ADD COLUMN y int"), ExplainCategory::DdlPlanOnly);
        assert_eq!(cls("TRUNCATE x"), ExplainCategory::DdlPlanOnly);
    }

    #[test]
    fn already_explain_passthrough() {
        assert_eq!(cls("EXPLAIN SELECT 1"), ExplainCategory::Passthrough);
        assert_eq!(
            cls("EXPLAIN (ANALYZE, BUFFERS) SELECT 1"),
            ExplainCategory::Passthrough
        );
        assert_eq!(cls("  explain   select 1"), ExplainCategory::Passthrough);
    }

    #[test]
    fn unparseable_returns_error() {
        assert_eq!(cls(""), ExplainCategory::Unparseable);
        assert_eq!(cls("    "), ExplainCategory::Unparseable);
        assert_eq!(cls("not even sql !!"), ExplainCategory::Unparseable);
    }

    #[test]
    fn non_explainable_returns_error() {
        assert_eq!(cls("BEGIN"), ExplainCategory::NotExplainable);
        assert_eq!(cls("COMMIT"), ExplainCategory::NotExplainable);
        assert_eq!(cls("SET search_path = public"), ExplainCategory::NotExplainable);
    }

    #[test]
    fn multi_statement_uses_first() {
        assert_eq!(cls("SELECT 1; UPDATE t SET a=1"), ExplainCategory::SelectAnalyze);
        assert_eq!(cls("UPDATE t SET a=1; SELECT 1"), ExplainCategory::DmlPlanOnly);
    }
}
```

- [ ] **Step 2: Run the failing tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib commands::sqlast::explain_classifier_tests
```

Expected: compile errors / unresolved `classify_for_explain` and `ExplainCategory`.

- [ ] **Step 3: Implement `ExplainCategory` enum and `classify_for_explain`**

Append to `src-tauri/src/commands/sqlast.rs` (above the existing test mod, below `parse_select_target`):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExplainCategory {
    /// SELECT/CTE/VALUES — wrap with EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON).
    SelectAnalyze,
    /// DML (INSERT/UPDATE/DELETE/MERGE) — wrap with EXPLAIN (FORMAT JSON) only.
    DmlPlanOnly,
    /// DDL (CREATE/DROP/ALTER/TRUNCATE/GRANT/REVOKE) — wrap with EXPLAIN (FORMAT JSON) only.
    DdlPlanOnly,
    /// User already wrote `EXPLAIN ...` — execute as-is.
    Passthrough,
    /// SQL that the parser could not understand at all.
    Unparseable,
    /// Parsed fine, but Postgres won't accept this in EXPLAIN
    /// (e.g., BEGIN, COMMIT, SET, SHOW).
    NotExplainable,
}

/// Classify a SQL string for the EXPLAIN runner. Examines only the first
/// statement; multi-statement input is allowed but only the first decides
/// the category. Callers that wrap the SQL must wrap *only* the first
/// statement and surface a warning for any trailing statements.
pub fn classify_for_explain(sql: &str) -> ExplainCategory {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return ExplainCategory::Unparseable;
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("explain ") || lower.starts_with("explain(") {
        return ExplainCategory::Passthrough;
    }

    let stmts = match Parser::parse_sql(&PostgreSqlDialect {}, trimmed) {
        Ok(s) => s,
        Err(_) => return ExplainCategory::Unparseable,
    };
    let first = match stmts.into_iter().next() {
        Some(s) => s,
        None => return ExplainCategory::Unparseable,
    };

    use sqlparser::ast::Statement as S;
    match first {
        S::Query(_) => ExplainCategory::SelectAnalyze,
        S::Insert { .. } | S::Update { .. } | S::Delete { .. } | S::Merge { .. } => {
            ExplainCategory::DmlPlanOnly
        }
        S::CreateTable { .. }
        | S::CreateIndex { .. }
        | S::CreateView { .. }
        | S::CreateSchema { .. }
        | S::CreateExtension { .. }
        | S::CreateFunction { .. }
        | S::CreateMaterializedView { .. }
        | S::Drop { .. }
        | S::AlterTable { .. }
        | S::AlterIndex { .. }
        | S::Truncate { .. }
        | S::Grant { .. }
        | S::Revoke { .. } => ExplainCategory::DdlPlanOnly,
        _ => ExplainCategory::NotExplainable,
    }
}
```

If a `Statement::*` variant doesn't exist in the pinned sqlparser version (compile error), drop that arm — the `_ => NotExplainable` fallback covers it.

- [ ] **Step 4: Run tests to verify pass**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib commands::sqlast::explain_classifier_tests
```

Expected: all tests pass.

- [ ] **Step 5: Run full Rust gate**

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt
cargo test --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/sqlast.rs
git commit -m "feat(week5): classify_for_explain in commands/sqlast"
```

---

## Task 3: Rust — `db/explain_runner.rs`

**Goal:** Module that takes a connection + classified SQL + flags and returns parsed JSON plan. No history side effects, no candidates yet.

**Files:**

- Create: `src-tauri/src/db/explain_runner.rs`
- Modify: `src-tauri/src/db/mod.rs`

**Steps:**

- [ ] **Step 1: Wire the new module**

Edit `src-tauri/src/db/mod.rs`. Add after `pub mod schema_embed;`:

```rust
pub mod explain_runner;
```

- [ ] **Step 2: Write the module**

Create `src-tauri/src/db/explain_runner.rs`:

```rust
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

pub fn category_to_exec_mode(c: ExplainCategory, allow_analyze_anyway: bool) -> Option<ExplainExecMode> {
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
    let result: Result<(JsonValue,), sqlx::Error> = sqlx::query_as(wrapped_sql)
        .fetch_one(&mut **conn)
        .await;
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
        JsonValue::String(s) => serde_json::from_str(&s).map_err(|e| {
            TuskError::Explain(format!("EXPLAIN JSON parse failed: {e}"))
        }),
        other => Err(TuskError::Explain(format!("Unexpected EXPLAIN shape: {other}"))),
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
```

Note: `ActiveConnection` import path — verify in `src-tauri/src/db/pool.rs` that this is the public type name. If it's `ActiveDb` or similar, adjust.

- [ ] **Step 3: Run unit tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib db::explain_runner::tests
```

Expected: 5 tests pass.

- [ ] **Step 4: Run full Rust gate**

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt
cargo test --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/mod.rs src-tauri/src/db/explain_runner.rs
git commit -m "feat(week5): db::explain_runner — wrap_for_explain + run_wrapped_explain"
```

---

## Task 4: Rust — `commands/explain.rs` (run_explain, no candidates yet)

**Goal:** Tauri command that ties classification + execution + history together. Returns `ExplainResult` with empty `verifiedCandidates` (filled in T6).

**Files:**

- Create: `src-tauri/src/commands/explain.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Steps:**

- [ ] **Step 1: Wire module**

Edit `src-tauri/src/commands/mod.rs`. Add (alphabetical order, between `editing` and `export`):

```rust
pub mod explain;
```

- [ ] **Step 2: Implement command**

Create `src-tauri/src/commands/explain.rs`:

```rust
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

    let category = classify_for_explain(&sql);
    let exec_mode = category_to_exec_mode(category, allow_analyze_anyway).ok_or_else(|| {
        TuskError::Explain(match category {
            ExplainCategory::Unparseable => "SQL could not be parsed".into(),
            ExplainCategory::NotExplainable => "Statement is not explainable".into(),
            _ => "Cannot run EXPLAIN on this SQL".into(),
        })
    })?;

    let mut warnings: Vec<String> = Vec::new();
    if sql.trim_start().split(';').filter(|s| !s.trim().is_empty()).count() > 1 {
        warnings.push("Only the first statement is explained; subsequent statements ignored".into());
    }

    let wrapped = wrap_for_explain(&sql, exec_mode);
    let output = run_wrapped_explain(&active, &wrapped, exec_mode).await?;

    let _ = app_handle.emit(
        "query:started",
        serde_json::json!({
            "connId": connection_id,
            "pid": output.pid,
            "startedAt": chrono::Utc::now().timestamp_millis(),
        }),
    );
    let _ = app_handle.emit(
        "query:completed",
        serde_json::json!({
            "connId": connection_id,
            "pid": output.pid,
            "ok": true,
        }),
    );

    let entry_id = uuid::Uuid::new_v4().to_string();
    let preview: String = wrapped.chars().take(200).collect();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let duration_ms = started.elapsed().as_millis() as i64;
    if let Err(e) = store.insert_history_entry(&HistoryEntry {
        id: entry_id,
        conn_id: connection_id.clone(),
        source: "editor".into(),
        tx_id: None,
        sql_preview: preview,
        sql_full: Some(wrapped.clone()),
        started_at: now_ms,
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
        executed_at: now_ms,
    })
}
```

- [ ] **Step 3: Register in `lib.rs`**

Edit `src-tauri/src/lib.rs`. Add to `tauri::generate_handler![...]`:

```rust
            commands::explain::run_explain,
```

- [ ] **Step 4: Compile**

```bash
pnpm rust:check && pnpm rust:lint
```

Fix any signature mismatches with `ConnectionRegistry::handle` / `HistoryEntry` shape if they arise.

- [ ] **Step 5: Add integration smoke test**

Create `src-tauri/tests/explain_smoke.rs`:

```rust
use serde_json::Value as JsonValue;
use sqlx::postgres::PgPoolOptions;

use tusk_lib::commands::sqlast::{classify_for_explain, ExplainCategory};
use tusk_lib::db::explain_runner::{
    category_to_exec_mode, wrap_for_explain, ExplainExecMode,
};

#[tokio::test]
#[ignore]
async fn explain_select_against_live_pg() {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect("postgres://tusk:tusk@127.0.0.1:55432/tusk_test")
        .await
        .expect("connect");
    let wrapped = wrap_for_explain("SELECT 1", ExplainExecMode::SelectAnalyze);
    let row: (JsonValue,) = sqlx::query_as(&wrapped)
        .fetch_one(&pool)
        .await
        .expect("explain ok");
    let arr = row.0.as_array().expect("array");
    assert_eq!(arr.len(), 1);
    assert!(arr[0]["Plan"].is_object());
}

#[test]
fn classify_basic() {
    assert_eq!(
        category_to_exec_mode(classify_for_explain("SELECT 1"), false),
        Some(ExplainExecMode::SelectAnalyze)
    );
}
```

The crate is exposed as `tusk_lib` per Cargo.toml; verify with `grep '^name' src-tauri/Cargo.toml`. Adjust the imports if the crate name differs.

- [ ] **Step 6: Run integration test**

```bash
docker compose -f infra/postgres/docker-compose.yml up -d
cargo test --manifest-path src-tauri/Cargo.toml --test explain_smoke -- --include-ignored
```

Expected: pass.

- [ ] **Step 7: Run full gate**

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt
cargo test --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/commands/mod.rs src-tauri/src/commands/explain.rs \
        src-tauri/src/lib.rs src-tauri/tests/explain_smoke.rs
git commit -m "feat(week5): commands::explain::run_explain (no candidates yet)"
```

---

## Task 5: Rust — `db/pg_stats.rs` (`fetch_column_stats`)

**Goal:** Read `pg_stats.n_distinct` / `null_frac` for a set of (schema, table, column) triples in one round trip.

**Files:**

- Create: `src-tauri/src/db/pg_stats.rs`
- Modify: `src-tauri/src/db/mod.rs`

**Steps:**

- [ ] **Step 1: Wire module**

Edit `src-tauri/src/db/mod.rs`. Add after `pub mod pg_meta;`:

```rust
pub mod pg_stats;
```

- [ ] **Step 2: Implement**

Create `src-tauri/src/db/pg_stats.rs`:

```rust
//! Per-column cardinality lookup via `pg_stats`.
use std::collections::HashMap;

use serde::Serialize;

use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnStats {
    pub n_distinct: Option<f64>,
    pub null_frac: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct ColumnRef {
    pub schema: String,
    pub table: String,
    pub column: String,
}

pub async fn fetch_column_stats(
    pool: &sqlx::PgPool,
    refs: &[ColumnRef],
) -> TuskResult<HashMap<(String, String, String), ColumnStats>> {
    if refs.is_empty() {
        return Ok(HashMap::new());
    }

    let mut placeholders = String::new();
    for i in 0..refs.len() {
        if i > 0 {
            placeholders.push(',');
        }
        let base = i * 3 + 1;
        placeholders.push_str(&format!("(${}, ${}, ${})", base, base + 1, base + 2));
    }
    let sql = format!(
        r#"
        WITH input(schema_name, table_name, column_name) AS (
            VALUES {placeholders}
        )
        SELECT input.schema_name, input.table_name, input.column_name,
               s.n_distinct, s.null_frac
        FROM input
        LEFT JOIN pg_stats s
          ON s.schemaname = input.schema_name
         AND s.tablename = input.table_name
         AND s.attname = input.column_name
        "#
    );

    let mut q = sqlx::query_as::<_, (String, String, String, Option<f64>, Option<f32>)>(&sql);
    for r in refs {
        q = q.bind(&r.schema).bind(&r.table).bind(&r.column);
    }
    let rows = q
        .fetch_all(pool)
        .await
        .map_err(|e| TuskError::Explain(format!("pg_stats query failed: {e}")))?;

    let mut out = HashMap::with_capacity(rows.len());
    for (schema, table, column, n_distinct, null_frac_f32) in rows {
        out.insert(
            (schema, table, column),
            ColumnStats {
                n_distinct,
                null_frac: null_frac_f32.map(f64::from),
            },
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn missing_table_returns_none_pair() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect("postgres://tusk:tusk@127.0.0.1:55432/tusk_test")
            .await
            .unwrap();
        let refs = vec![ColumnRef {
            schema: "public".into(),
            table: "no_such_table_in_week5".into(),
            column: "no_such_column".into(),
        }];
        let m = fetch_column_stats(&pool, &refs).await.unwrap();
        let stats = m
            .get(&(
                "public".into(),
                "no_such_table_in_week5".into(),
                "no_such_column".into(),
            ))
            .unwrap();
        assert!(stats.n_distinct.is_none());
        assert!(stats.null_frac.is_none());
    }
}
```

- [ ] **Step 3: Run ignored integration tests against docker**

```bash
docker compose -f infra/postgres/docker-compose.yml up -d
cargo test --manifest-path src-tauri/Cargo.toml --lib db::pg_stats::tests -- --include-ignored
```

Expected: pass.

- [ ] **Step 4: Run full Rust gate**

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt
cargo test --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/mod.rs src-tauri/src/db/pg_stats.rs
git commit -m "feat(week5): db::pg_stats — fetch_column_stats"
```

---

## Task 6: Rust — `extract_index_candidates` integrated into `run_explain`

**Goal:** Walk plan JSON, score each Seq-Scan-with-Filter candidate via pg_stats, and return verified list inside ExplainResult.

**Files:**

- Modify: `src-tauri/src/commands/explain.rs`

**Steps:**

- [ ] **Step 1: Add the extractor + scorer**

Append to `src-tauri/src/commands/explain.rs`:

```rust
use std::collections::BTreeSet;

use crate::db::pg_stats::{fetch_column_stats, ColumnRef};

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
                if sel <= 0.05 { ("likely", Some(sel)) }
                else if sel <= 0.20 { ("maybe", Some(sel)) }
                else { continue; }
            }
            Some(v) if v < 0.0 => {
                let sel = v.abs();
                if sel <= 0.05 { ("likely", Some(sel)) }
                else if sel <= 0.20 { ("maybe", Some(sel)) }
                else { continue; }
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
        let actual_rows = node.get("Actual Rows").and_then(|v| v.as_i64()).unwrap_or(0);
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
                        reason: if big_filter { "rows-removed-by-filter" } else { "seq-scan-filter" },
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
    for piece in filter.split(|c: char| matches!(c, ',' | ';')) {
        let p = piece.trim().trim_start_matches('(').trim_end_matches(')');
        let anchor = [" = ", " IN ", " BETWEEN ", " <= ", " >= ", " < ", " > ", " <> ", " != "]
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
    let candidate = s.split('.').last().unwrap_or(s);
    let candidate = candidate.trim_matches('"');
    if candidate.is_empty() {
        return false;
    }
    candidate.chars().all(|c| c.is_alphanumeric() || c == '_')
}

fn strip_quotes(s: &str) -> &str {
    s.trim_matches('"').rsplit('.').next().unwrap_or(s)
}
```

- [ ] **Step 2: Plug into `run_explain`**

In `src-tauri/src/commands/explain.rs`, after `run_wrapped_explain` succeeds and before constructing `ExplainResult`, add:

```rust
    let candidates: Vec<JsonValue> = match extract_index_candidates(&active.pool, &output.plan_value).await {
        Ok(list) => list
            .into_iter()
            .map(|c| serde_json::to_value(&c).unwrap_or(JsonValue::Null))
            .collect(),
        Err(e) => {
            warnings.push(format!("Index candidate extraction failed: {e}"));
            Vec::new()
        }
    };
```

And replace the field assignment:

```rust
        verified_candidates: candidates,
```

- [ ] **Step 3: Add unit tests**

Append to `src-tauri/src/commands/explain.rs`:

```rust
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
        assert_eq!(extract_simple_columns("(u.email = 'a')"), vec!["email".to_string()]);
        assert_eq!(extract_simple_columns("(\"first name\" = 'a')"), Vec::<String>::new());
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
```

- [ ] **Step 4: Run tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib commands::explain::tests
```

Expected: 5 tests pass.

- [ ] **Step 5: Run full Rust gate**

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt
cargo test --manifest-path src-tauri/Cargo.toml
docker compose -f infra/postgres/docker-compose.yml up -d
cargo test --manifest-path src-tauri/Cargo.toml -- --include-ignored
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/explain.rs
git commit -m "feat(week5): extract_index_candidates with pg_stats verdict"
```

---

## Task 7: Rust — migration `004_ai_explain` + `record_ai_explain`

**Goal:** Persist AI-explain interpretations alongside Week 4's `ai_history`.

**Files:**

- Modify: `src-tauri/src/db/state.rs`
- Modify: `src-tauri/src/commands/history.rs`
- Modify: `src-tauri/src/lib.rs`

**Steps:**

- [ ] **Step 1: Extend the migration**

Edit `src-tauri/src/db/state.rs`. Inside `fn migrate(&self)`, after the existing `ai_history` block (around line 187), add:

```rust
        db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS ai_explain (
                entry_id                  TEXT PRIMARY KEY
                                          REFERENCES history_entry(id) ON DELETE CASCADE,
                plan_sha                  TEXT NOT NULL,
                provider                  TEXT NOT NULL,
                model                     TEXT NOT NULL,
                summary                   TEXT NOT NULL,
                raw_plan_json             TEXT NOT NULL,
                verified_candidates_json  TEXT NOT NULL,
                llm_recommendations_json  TEXT NOT NULL,
                prompt_tokens             INTEGER,
                completion_tokens         INTEGER,
                duration_ms               INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_ai_explain_plan_sha ON ai_explain(plan_sha);
            "#,
        )
        .map_err(|e| TuskError::State(e.to_string()))?;
```

- [ ] **Step 2: Add the insert helper + payload type**

Append to `src-tauri/src/db/state.rs` (next to `AiGenerationPayload`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiExplainPayload {
    pub conn_id: String,
    pub plan_sha: String,
    pub provider: String,
    pub model: String,
    pub summary: String,
    pub raw_plan_json: String,
    pub verified_candidates_json: String,
    pub llm_recommendations_json: String,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub duration_ms: i64,
}
```

Inside `impl StateStore` (after `insert_ai_generation`):

```rust
    pub fn insert_ai_explain(&self, payload: &AiExplainPayload) -> TuskResult<String> {
        let mut conn = self.db.lock().expect("state lock poisoned");

        let entry_id = uuid::Uuid::new_v4().to_string();
        let preview = format!(
            "-- EXPLAIN interpretation [planSha={}]",
            &payload.plan_sha[..16.min(payload.plan_sha.len())]
        );
        let now = chrono::Utc::now().timestamp_millis();

        let tx = conn.transaction().map_err(|e| TuskError::History(e.to_string()))?;

        tx.execute(
            "INSERT INTO history_entry
             (id, conn_id, source, tx_id, sql_preview, sql_full,
              started_at, duration_ms, row_count, status, error_message, statement_count)
             VALUES (?, ?, 'ai_explain', NULL, ?, ?, ?, ?, NULL, 'ok', NULL, 0)",
            rusqlite::params![
                entry_id,
                payload.conn_id,
                preview,
                payload.summary.chars().take(2000).collect::<String>(),
                now,
                payload.duration_ms,
            ],
        )
        .map_err(|e| TuskError::History(e.to_string()))?;

        tx.execute(
            "INSERT INTO ai_explain
             (entry_id, plan_sha, provider, model, summary, raw_plan_json,
              verified_candidates_json, llm_recommendations_json,
              prompt_tokens, completion_tokens, duration_ms)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                entry_id,
                payload.plan_sha,
                payload.provider,
                payload.model,
                payload.summary,
                payload.raw_plan_json,
                payload.verified_candidates_json,
                payload.llm_recommendations_json,
                payload.prompt_tokens,
                payload.completion_tokens,
                payload.duration_ms,
            ],
        )
        .map_err(|e| TuskError::History(e.to_string()))?;

        tx.commit().map_err(|e| TuskError::History(e.to_string()))?;
        Ok(entry_id)
    }
```

- [ ] **Step 3: Expose Tauri command**

Edit `src-tauri/src/commands/history.rs`. Add at the bottom:

```rust
use crate::db::state::AiExplainPayload;

#[tauri::command]
pub async fn record_ai_explain(
    store: tauri::State<'_, crate::db::state::StateStore>,
    payload: AiExplainPayload,
) -> crate::errors::TuskResult<String> {
    store.insert_ai_explain(&payload)
}
```

- [ ] **Step 4: Register in `lib.rs`**

Edit `src-tauri/src/lib.rs`. Add to `tauri::generate_handler![...]`:

```rust
            commands::history::record_ai_explain,
```

- [ ] **Step 5: Add round-trip unit test**

Append to `src-tauri/src/db/state.rs` under any existing `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn ai_explain_round_trip() {
        let store = StateStore::open_in_memory().unwrap();
        let payload = AiExplainPayload {
            conn_id: "conn-1".into(),
            plan_sha: "abcdef0123456789abcdef0123456789".into(),
            provider: "openai".into(),
            model: "gpt-4o-mini".into(),
            summary: "Seq scan dominates.".into(),
            raw_plan_json: "{}".into(),
            verified_candidates_json: "[]".into(),
            llm_recommendations_json: "[]".into(),
            prompt_tokens: Some(1234),
            completion_tokens: Some(56),
            duration_ms: 789,
        };
        let id = store.insert_ai_explain(&payload).expect("insert");
        assert!(!id.is_empty());

        let db = store.lock();
        let n: i64 = db
            .query_row("SELECT count(*) FROM ai_explain WHERE entry_id = ?", [&id], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }
```

- [ ] **Step 6: Run tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib db::state::tests::ai_explain_round_trip
```

- [ ] **Step 7: Run full Rust gate**

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt
cargo test --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/db/state.rs src-tauri/src/commands/history.rs src-tauri/src/lib.rs
git commit -m "feat(week5): migration 004_ai_explain + record_ai_explain"
```

---

## Task 8: Rust — verify ANALYZE-anyway path end-to-end

**Goal:** Confirm a DELETE EXPLAIN ANALYZE leaves the table untouched, even when the EXPLAIN errors mid-call.

**Files:**

- Modify: `src-tauri/tests/explain_smoke.rs`

**Steps:**

- [ ] **Step 1: Add seed helper + tests**

Append to `src-tauri/tests/explain_smoke.rs`:

```rust
async fn seed_demo_table(pool: &sqlx::PgPool) {
    sqlx::query("DROP TABLE IF EXISTS week5_demo")
        .fetch_all(pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE week5_demo (id int, label text)")
        .fetch_all(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO week5_demo VALUES (1,'a'),(2,'b'),(3,'c')")
        .fetch_all(pool)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore]
async fn analyze_anyway_rolls_back_delete() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect("postgres://tusk:tusk@127.0.0.1:55432/tusk_test")
        .await
        .unwrap();
    seed_demo_table(&pool).await;

    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("BEGIN").fetch_all(&mut *conn).await.unwrap();
    let r: Result<(JsonValue,), sqlx::Error> = sqlx::query_as(
        "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) DELETE FROM week5_demo",
    )
    .fetch_one(&mut *conn)
    .await;
    sqlx::query("ROLLBACK").fetch_all(&mut *conn).await.unwrap();
    assert!(r.is_ok(), "EXPLAIN ANALYZE DELETE should succeed");
    drop(conn);

    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM week5_demo")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 3, "DELETE inside EXPLAIN ANALYZE must be rolled back");
}

#[tokio::test]
#[ignore]
async fn analyze_anyway_rolls_back_on_error() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect("postgres://tusk:tusk@127.0.0.1:55432/tusk_test")
        .await
        .unwrap();
    seed_demo_table(&pool).await;

    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("BEGIN").fetch_all(&mut *conn).await.unwrap();
    let _ = sqlx::query("EXPLAIN ANALYZE DELETE FROM no_such_table_x9")
        .fetch_all(&mut *conn)
        .await;
    sqlx::query("ROLLBACK").fetch_all(&mut *conn).await.unwrap();
    drop(conn);

    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM week5_demo")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 3);
}
```

- [ ] **Step 2: Run integration tests**

```bash
docker compose -f infra/postgres/docker-compose.yml up -d
cargo test --manifest-path src-tauri/Cargo.toml --test explain_smoke -- --include-ignored
```

Expected: all pass.

- [ ] **Step 3: Run full Rust gate**

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt
cargo test --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tests/explain_smoke.rs
git commit -m "test(week5): verify ANALYZE-anyway BEGIN/ROLLBACK leaves table intact"
```

---

## Task 9: Frontend — `lib/explain/planTypes.ts` + `planParse.ts` + `planSha.ts`

**Goal:** Pure TS modules with no React dependencies — the trunk for everything else.

**Files:**

- Create: `src/lib/explain/planTypes.ts`
- Create: `src/lib/explain/planParse.ts`
- Create: `src/lib/explain/planSha.ts`
- Create: `src/lib/explain/planParse.test.ts`
- Create: `src/lib/explain/planSha.test.ts`

**Steps:**

- [ ] **Step 1: Write `planTypes.ts`**

```ts
export type ExplainMode =
  | "select-analyze"
  | "dml-plan-only"
  | "ddl-plan-only"
  | "passthrough"
  | "analyze-anyway-rolled-back"
  | "analyze-anyway-in-tx";

export interface RawExplainPlan {
  Plan: RawPlanNode;
  "Planning Time"?: number;
  "Execution Time"?: number;
}

export interface RawPlanNode {
  "Node Type": string;
  "Parallel Aware"?: boolean;
  "Join Type"?: string;
  "Relation Name"?: string;
  Schema?: string;
  Alias?: string;
  "Startup Cost": number;
  "Total Cost": number;
  "Plan Rows": number;
  "Plan Width": number;
  "Actual Startup Time"?: number;
  "Actual Total Time"?: number;
  "Actual Rows"?: number;
  "Actual Loops"?: number;
  "Rows Removed by Filter"?: number;
  Filter?: string;
  "Index Cond"?: string;
  "Hash Cond"?: string;
  "Merge Cond"?: string;
  "Recheck Cond"?: string;
  Output?: string[];
  "Shared Hit Blocks"?: number;
  "Shared Read Blocks"?: number;
  "Shared Written Blocks"?: number;
  Plans?: RawPlanNode[];
  Buffers?: {
    "Shared Hit Blocks"?: number;
    "Shared Read Blocks"?: number;
    "Shared Written Blocks"?: number;
  };
}

export interface PlanNode {
  nodeType: string;
  relationName?: string;
  schema?: string;
  alias?: string;
  startupCost: number;
  totalCost: number;
  planRows: number;
  planWidth: number;
  actualStartupTime: number | null;
  actualTotalTime: number | null;
  actualLoops: number | null;
  actualRows: number | null;
  rowsRemovedByFilter: number | null;
  filter?: string;
  indexCond?: string;
  joinType?: string;
  hashCond?: string;
  mergeCond?: string;
  output?: string[];
  buffers: { hit: number; read: number; written: number } | null;
  children: PlanNode[];
  selfMs: number | null;
  selfTimeRatio: number | null;
  selfCostRatio: number;
}

export interface IndexCandidate {
  schema: string;
  table: string;
  columns: string[];
  reason: "seq-scan-filter" | "rows-removed-by-filter" | "lossy-index-cond";
  verdict: "likely" | "maybe";
  selectivityEstimate: number | null;
  nDistinct: number | null;
  nullFrac: number | null;
}

export interface ExplainResult {
  mode: ExplainMode;
  planJson: RawExplainPlan;
  plan: PlanNode;
  warnings: string[];
  verifiedCandidates: IndexCandidate[];
  totalMs: number | null;
  executedAt: number;
}

export interface AiInterpretation {
  summary: string;
  recommendations: AiIndexRecommendation[];
  promptTokens?: number;
  completionTokens?: number;
  durationMs: number;
}

export interface AiIndexRecommendation {
  schema: string;
  table: string;
  columns: string[];
  type: "btree" | "composite" | "partial";
  where?: string;
  reason: string;
  priority: "high" | "medium" | "low";
}
```

- [ ] **Step 2: Write `planParse.test.ts`**

Create `src/lib/explain/planParse.test.ts` with the tests for selfMs, selfTimeRatio, plan-only mode, and depth cutoff (full content provided in T20 Step 2 — write a starter version here without the depth-limit test, then T20 adds it). Starter:

```ts
import { describe, expect, it } from "vitest";
import { parsePlan } from "./planParse";
import type { RawExplainPlan } from "./planTypes";

const fixture = {
  analyze: {
    Plan: {
      "Node Type": "Hash Join",
      "Startup Cost": 0,
      "Total Cost": 100,
      "Plan Rows": 10,
      "Plan Width": 64,
      "Actual Startup Time": 0.1,
      "Actual Total Time": 12.0,
      "Actual Rows": 100,
      "Actual Loops": 1,
      Plans: [
        {
          "Node Type": "Seq Scan",
          "Relation Name": "users",
          Schema: "public",
          "Startup Cost": 0,
          "Total Cost": 50,
          "Plan Rows": 50,
          "Plan Width": 32,
          "Actual Startup Time": 0.05,
          "Actual Total Time": 8.0,
          "Actual Rows": 50,
          "Actual Loops": 1,
          Filter: "(email = 'a')",
          "Rows Removed by Filter": 0,
        },
        {
          "Node Type": "Index Scan",
          "Relation Name": "orders",
          Schema: "public",
          "Startup Cost": 0,
          "Total Cost": 30,
          "Plan Rows": 100,
          "Plan Width": 32,
          "Actual Startup Time": 0.05,
          "Actual Total Time": 0.5,
          "Actual Rows": 100,
          "Actual Loops": 1,
        },
      ],
    },
  } satisfies RawExplainPlan,
  planOnly: {
    Plan: {
      "Node Type": "Seq Scan",
      "Relation Name": "users",
      Schema: "public",
      "Startup Cost": 0,
      "Total Cost": 100,
      "Plan Rows": 1000,
      "Plan Width": 32,
    },
  } satisfies RawExplainPlan,
};

describe("parsePlan — analyze", () => {
  it("computes selfMs as parent total minus child totals", () => {
    const root = parsePlan(fixture.analyze);
    expect(root.actualTotalTime).toBe(12);
    expect(root.selfMs).toBeCloseTo(12 - 8 - 0.5, 5);
  });

  it("computes selfTimeRatio against root total", () => {
    const root = parsePlan(fixture.analyze);
    const seqScan = root.children[0];
    expect(seqScan.selfTimeRatio).toBeCloseTo(8 / 12, 5);
  });

  it("populates relationName/schema/filter for leaves", () => {
    const root = parsePlan(fixture.analyze);
    expect(root.children[0]).toMatchObject({
      relationName: "users",
      schema: "public",
      filter: "(email = 'a')",
    });
  });
});

describe("parsePlan — plan-only", () => {
  it("leaves actual fields null and uses selfCostRatio as fallback", () => {
    const root = parsePlan(fixture.planOnly);
    expect(root.actualTotalTime).toBeNull();
    expect(root.selfMs).toBeNull();
    expect(root.selfTimeRatio).toBeNull();
    expect(root.selfCostRatio).toBeCloseTo(1, 5);
  });
});
```

- [ ] **Step 3: Run failing test**

```bash
pnpm test src/lib/explain/planParse.test.ts
```

Expected: ENOENT for `./planParse`.

- [ ] **Step 4: Implement `planParse.ts`**

```ts
import type { PlanNode, RawExplainPlan, RawPlanNode } from "./planTypes";

const MAX_DEPTH = 100;

export function parsePlan(raw: RawExplainPlan): PlanNode {
  const root = raw.Plan;
  const rootTotalMs =
    typeof root["Actual Total Time"] === "number"
      ? root["Actual Total Time"]
      : null;
  const rootTotalCost = root["Total Cost"] || 1;
  return walk(root, rootTotalMs, rootTotalCost, 0);
}

function walk(
  raw: RawPlanNode,
  rootTotalMs: number | null,
  rootTotalCost: number,
  depth: number,
): PlanNode {
  if (depth >= MAX_DEPTH) {
    return {
      nodeType: `${raw["Node Type"]} (truncated at depth ${MAX_DEPTH})`,
      startupCost: 0,
      totalCost: 0,
      planRows: 0,
      planWidth: 0,
      actualStartupTime: null,
      actualTotalTime: null,
      actualLoops: null,
      actualRows: null,
      rowsRemovedByFilter: null,
      buffers: null,
      children: [],
      selfMs: null,
      selfTimeRatio: null,
      selfCostRatio: 0,
    };
  }

  const children = (raw.Plans ?? []).map((c) =>
    walk(c, rootTotalMs, rootTotalCost, depth + 1),
  );
  const childTotalMs = children.reduce(
    (a, c) =>
      a !== null && c.actualTotalTime !== null ? a + c.actualTotalTime : null,
    children.length === 0 ? 0 : null,
  );
  const total =
    typeof raw["Actual Total Time"] === "number"
      ? raw["Actual Total Time"]
      : null;
  const selfMs =
    total !== null && childTotalMs !== null
      ? Math.max(0, total - childTotalMs)
      : null;
  const selfTimeRatio =
    selfMs !== null && rootTotalMs && rootTotalMs > 0
      ? selfMs / rootTotalMs
      : null;

  const childTotalCost = children.reduce((a, c) => a + c.totalCost, 0);
  const selfCost = Math.max(0, raw["Total Cost"] - childTotalCost);
  const selfCostRatio = rootTotalCost > 0 ? selfCost / rootTotalCost : 0;

  const buffersFromContainer = raw.Buffers;
  const buffersFromInline =
    raw["Shared Hit Blocks"] !== undefined ||
    raw["Shared Read Blocks"] !== undefined ||
    raw["Shared Written Blocks"] !== undefined
      ? {
          "Shared Hit Blocks": raw["Shared Hit Blocks"],
          "Shared Read Blocks": raw["Shared Read Blocks"],
          "Shared Written Blocks": raw["Shared Written Blocks"],
        }
      : undefined;
  const buffersRaw = buffersFromContainer ?? buffersFromInline;

  return {
    nodeType: raw["Node Type"],
    relationName: raw["Relation Name"],
    schema: raw.Schema,
    alias: raw.Alias,
    startupCost: raw["Startup Cost"],
    totalCost: raw["Total Cost"],
    planRows: raw["Plan Rows"],
    planWidth: raw["Plan Width"],
    actualStartupTime: raw["Actual Startup Time"] ?? null,
    actualTotalTime: total,
    actualLoops: raw["Actual Loops"] ?? null,
    actualRows: raw["Actual Rows"] ?? null,
    rowsRemovedByFilter: raw["Rows Removed by Filter"] ?? null,
    filter: raw.Filter,
    indexCond: raw["Index Cond"],
    joinType: raw["Join Type"],
    hashCond: raw["Hash Cond"],
    mergeCond: raw["Merge Cond"],
    output: raw.Output,
    buffers: buffersRaw
      ? {
          hit: buffersRaw["Shared Hit Blocks"] ?? 0,
          read: buffersRaw["Shared Read Blocks"] ?? 0,
          written: buffersRaw["Shared Written Blocks"] ?? 0,
        }
      : null,
    children,
    selfMs,
    selfTimeRatio,
    selfCostRatio,
  };
}
```

- [ ] **Step 5: Run tests pass**

```bash
pnpm test src/lib/explain/planParse.test.ts
```

Expected: all pass.

- [ ] **Step 6: Write `planSha.test.ts`**

```ts
import { describe, expect, it } from "vitest";
import { planSha, stableStringify } from "./planSha";

describe("stableStringify", () => {
  it("produces identical output for objects with different key order", () => {
    expect(stableStringify({ a: 1, b: 2 })).toBe(
      stableStringify({ b: 2, a: 1 }),
    );
  });

  it("preserves array order", () => {
    expect(stableStringify([1, 2])).not.toBe(stableStringify([2, 1]));
  });

  it("recurses into nested objects", () => {
    const a = { x: { c: 1, b: 2 }, y: [{ z: 1, a: 2 }] };
    const b = { y: [{ a: 2, z: 1 }], x: { b: 2, c: 1 } };
    expect(stableStringify(a)).toBe(stableStringify(b));
  });
});

describe("planSha", () => {
  it("produces a 64-char hex string", async () => {
    const sha = await planSha({ Plan: { "Node Type": "Seq Scan" } });
    expect(sha).toMatch(/^[0-9a-f]{64}$/);
  });

  it("is stable across key re-orderings", async () => {
    const a = await planSha({
      Plan: { "Node Type": "Seq Scan", "Total Cost": 1 },
    });
    const b = await planSha({
      Plan: { "Total Cost": 1, "Node Type": "Seq Scan" },
    });
    expect(a).toBe(b);
  });
});
```

- [ ] **Step 7: Implement `planSha.ts`**

```ts
export function stableStringify(value: unknown): string {
  if (value === null || typeof value !== "object") {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(stableStringify).join(",")}]`;
  }
  const obj = value as Record<string, unknown>;
  const keys = Object.keys(obj).sort();
  return `{${keys.map((k) => `${JSON.stringify(k)}:${stableStringify(obj[k])}`).join(",")}}`;
}

export async function planSha(value: unknown): Promise<string> {
  const data = new TextEncoder().encode(stableStringify(value));
  const digest = await crypto.subtle.digest("SHA-256", data);
  const bytes = new Uint8Array(digest);
  let hex = "";
  for (const b of bytes) hex += b.toString(16).padStart(2, "0");
  return hex;
}
```

- [ ] **Step 8: Run all tests**

```bash
pnpm test src/lib/explain/
```

Expected: all green.

- [ ] **Step 9: Run full FE gate**

```bash
pnpm typecheck && pnpm lint && pnpm format
```

- [ ] **Step 10: Commit**

```bash
git add src/lib/explain
git commit -m "feat(week5): plan types + parse + stable SHA"
```

---

## Task 10: Frontend — store extensions (`tabs.ts`, `settings.ts`)

**Goal:** Make `Tab` carry both Rows and Plan results, plus settings for AI/index toggles.

**Files:**

- Modify: `src/store/tabs.ts`
- Modify: `src/store/settings.ts`

**Steps:**

- [ ] **Step 1: Extend `Tab` shape and actions**

Edit `src/store/tabs.ts`. Add imports:

```ts
import type { ExplainResult, AiInterpretation } from "@/lib/explain/planTypes";
```

Add a helper interface above `Tab`:

```ts
export interface PlanState {
  result: ExplainResult;
  selectedNodePath: number[];
  aiCacheByKey: Record<string, AiInterpretation>;
  activeAiKey: string | null;
  sqlAtRun: string;
}
```

Inside the `Tab` interface add:

```ts
  lastPlan?: PlanState;
  resultMode: "rows" | "plan";
```

Update the initial tab and `newTab` to include `resultMode: "rows"`.

Add to `TabsState`:

```ts
  setPlan: (id: string, result: ExplainResult, sqlAtRun: string) => void;
  setSelectedNodePath: (id: string, path: number[]) => void;
  setActiveAiKey: (id: string, key: string | null) => void;
  cacheAi: (id: string, key: string, interpretation: AiInterpretation) => void;
  setResultMode: (id: string, mode: "rows" | "plan") => void;
```

Implement actions inside `create<TabsState>`:

```ts
  setPlan(id, result, sqlAtRun) {
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === id
          ? {
              ...t,
              lastPlan: {
                result,
                selectedNodePath: [],
                aiCacheByKey: {},
                activeAiKey: null,
                sqlAtRun,
              },
              resultMode: "plan",
              busy: false,
            }
          : t,
      ),
    }));
  },

  setSelectedNodePath(id, path) {
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === id && t.lastPlan
          ? { ...t, lastPlan: { ...t.lastPlan, selectedNodePath: path } }
          : t,
      ),
    }));
  },

  setActiveAiKey(id, key) {
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === id && t.lastPlan
          ? { ...t, lastPlan: { ...t.lastPlan, activeAiKey: key } }
          : t,
      ),
    }));
  },

  cacheAi(id, key, interpretation) {
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === id && t.lastPlan
          ? {
              ...t,
              lastPlan: {
                ...t.lastPlan,
                aiCacheByKey: { ...t.lastPlan.aiCacheByKey, [key]: interpretation },
                activeAiKey: key,
              },
            }
          : t,
      ),
    }));
  },

  setResultMode(id, mode) {
    set((s) => ({
      tabs: s.tabs.map((t) => (t.id === id ? { ...t, resultMode: mode } : t)),
    }));
  },
```

Update `setResult` to also set `resultMode: "rows"`:

```ts
  setResult(id, result) {
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === id
          ? { ...t, lastResult: result, lastError: undefined, busy: false, resultMode: "rows" }
          : t,
      ),
    }));
  },
```

- [ ] **Step 2: Extend `settings.ts`**

Edit `src/store/settings.ts`. Add fields, initial values, and actions:

```ts
  autoInterpretPlan: boolean;
  setAutoInterpretPlan: (v: boolean) => void;
  indexAdviceEnabled: boolean;
  setIndexAdviceEnabled: (v: boolean) => void;
  explainTokenBudget: number;
  setExplainTokenBudget: (n: number) => void;
```

```ts
  autoInterpretPlan: false,
  indexAdviceEnabled: true,
  explainTokenBudget: 8000,
```

```ts
  setAutoInterpretPlan(v) {
    set({ autoInterpretPlan: v });
  },
  setIndexAdviceEnabled(v) {
    set({ indexAdviceEnabled: v });
  },
  setExplainTokenBudget(n) {
    set({ explainTokenBudget: Math.max(1000, n) });
  },
```

If the existing `settings.ts` uses a `persist` wrapper, add the new keys to its `partialize` list.

- [ ] **Step 3: Typecheck + lint**

```bash
pnpm typecheck && pnpm lint && pnpm format
```

Expected: green.

- [ ] **Step 4: Commit**

```bash
git add src/store/tabs.ts src/store/settings.ts
git commit -m "feat(week5): tabs.lastPlan + settings autoInterpret/indexAdvice/tokenBudget"
```

---

## Task 11: Frontend — `runExplain` invoke wrapper + `explainGate.ts`

**Goal:** Typed wrapper around `invoke('run_explain')` returning a parsed `ExplainResult`, plus a small gate that handles `runGate` for ANALYZE-anyway.

**Files:**

- Modify: `src/lib/tauri.ts`
- Create: `src/features/explain/explainGate.ts`

**Steps:**

- [ ] **Step 1: Add the typed invoke wrapper**

Edit `src/lib/tauri.ts`. Add:

```ts
import { invoke } from "@tauri-apps/api/core";

import type {
  ExplainResult,
  IndexCandidate,
  RawExplainPlan,
  ExplainMode,
} from "@/lib/explain/planTypes";
import { parsePlan } from "@/lib/explain/planParse";

interface RawRunExplainResult {
  mode: ExplainMode;
  planJson: RawExplainPlan;
  warnings: string[];
  verifiedCandidates: IndexCandidate[];
  totalMs: number | null;
  executedAt: number;
}

export async function runExplain(args: {
  connectionId: string;
  sql: string;
  allowAnalyzeAnyway?: boolean;
}): Promise<ExplainResult> {
  const raw = await invoke<RawRunExplainResult>("run_explain", {
    args: {
      connectionId: args.connectionId,
      sql: args.sql,
      allowAnalyzeAnyway: args.allowAnalyzeAnyway ?? false,
    },
  });
  return {
    mode: raw.mode,
    planJson: raw.planJson,
    plan: parsePlan(raw.planJson),
    warnings: raw.warnings,
    verifiedCandidates: raw.verifiedCandidates,
    totalMs: raw.totalMs,
    executedAt: raw.executedAt,
  };
}
```

(The Rust signature uses `args: RunExplainArgs` as a single struct param, so the `invoke` payload wraps under `args`. Verify after running.)

- [ ] **Step 2: Implement `explainGate.ts`**

Create `src/features/explain/explainGate.ts`:

```ts
import { toast } from "sonner";

import { runGate } from "@/lib/ai/runGate";
import { runExplain } from "@/lib/tauri";
import type { ExplainResult } from "@/lib/explain/planTypes";

export async function runExplainGate(args: {
  connId: string;
  sql: string;
  allowAnalyzeAnyway?: boolean;
}): Promise<ExplainResult | null> {
  if (!args.sql.trim()) {
    toast.error("SQL is empty");
    return null;
  }
  if (args.allowAnalyzeAnyway) {
    const proceed = await runGate(args.sql);
    if (!proceed) return null;
  }
  try {
    return await runExplain({
      connectionId: args.connId,
      sql: args.sql,
      allowAnalyzeAnyway: args.allowAnalyzeAnyway,
    });
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    toast.error(`Explain failed: ${msg}`);
    return null;
  }
}
```

- [ ] **Step 3: Typecheck + lint**

```bash
pnpm typecheck && pnpm lint
```

- [ ] **Step 4: Commit**

```bash
git add src/lib/tauri.ts src/features/explain/explainGate.ts
git commit -m "feat(week5): runExplain typed wrapper + explainGate"
```

---

## Task 12: Frontend — `ExplainView` stub + `[Rows | Plan]` toggle

**Goal:** Make Plan results renderable. No tree yet — placeholder showing mode + node count.

**Files:**

- Create: `src/features/explain/ExplainView.tsx`
- Modify: `src/features/results/ResultsHeader.tsx`
- Modify: `src/features/editor/EditorPane.tsx`

**Steps:**

- [ ] **Step 1: Stub `ExplainView`**

```tsx
import type { ExplainResult } from "@/lib/explain/planTypes";

interface Props {
  result: ExplainResult;
}

export function ExplainView({ result }: Props) {
  return (
    <div className="flex h-full flex-col">
      <header className="border-border bg-muted/30 flex items-center gap-2 border-b px-3 py-1.5 text-xs">
        <span className="rounded bg-amber-500/20 px-2 py-0.5">
          {result.mode}
        </span>
        {result.totalMs !== null && (
          <span className="text-muted-foreground">
            {result.totalMs.toFixed(1)} ms
          </span>
        )}
        {result.warnings.length > 0 && (
          <span className="text-amber-600" title={result.warnings.join("\n")}>
            ⚠ {result.warnings.length} warning(s)
          </span>
        )}
      </header>
      <div className="flex-1 overflow-auto p-3 font-mono text-xs">
        Plan tree placeholder — node count: {countNodes(result.plan)}
      </div>
    </div>
  );
}

function countNodes(node: { children: unknown[] }): number {
  return (
    1 +
    (node.children as { children: unknown[] }[]).reduce(
      (a, c) => a + countNodes(c as never),
      0,
    )
  );
}
```

- [ ] **Step 2: Add the `Rows | Plan` toggle to `ResultsHeader`**

Edit `src/features/results/ResultsHeader.tsx`. Update `Props`:

```ts
interface Props {
  result?: QueryResult;
  error?: string;
  busy?: boolean;
  connId?: string | null;
  hasPlan?: boolean;
  resultMode?: "rows" | "plan";
  onModeChange?: (mode: "rows" | "plan") => void;
}
```

Add the toggle block before `{busy && ...}`:

```tsx
{
  (result || hasPlan) && (
    <div className="border-input flex overflow-hidden rounded-sm border text-[11px]">
      <button
        type="button"
        onClick={() => onModeChange?.("rows")}
        className={`px-2 py-0.5 ${resultMode === "rows" ? "bg-accent text-accent-foreground" : ""}`}
        disabled={!result}
      >
        Rows
      </button>
      <button
        type="button"
        onClick={() => onModeChange?.("plan")}
        className={`px-2 py-0.5 ${resultMode === "plan" ? "bg-accent text-accent-foreground" : ""}`}
        disabled={!hasPlan}
      >
        Plan
      </button>
    </div>
  );
}
```

- [ ] **Step 3: Wire `ExplainView` and the toggle into `EditorPane`**

Edit `src/features/editor/EditorPane.tsx`. Add import:

```tsx
import { ExplainView } from "@/features/explain/ExplainView";
```

Replace the result-area block with:

```tsx
<div className="flex max-h-[45vh] min-h-[120px] flex-col">
  <ResultsHeader
    result={activeTab.lastResult}
    error={activeTab.lastError}
    busy={activeTab.busy}
    connId={connectionForTab}
    hasPlan={!!activeTab.lastPlan}
    resultMode={activeTab.resultMode}
    onModeChange={(mode) =>
      useTabs.getState().setResultMode(activeTab.id, mode)
    }
  />
  {activeTab.resultMode === "plan" && activeTab.lastPlan ? (
    <ExplainView result={activeTab.lastPlan.result} />
  ) : (
    activeTab.lastResult &&
    connectionForTab && (
      <ResultsGrid result={activeTab.lastResult} connId={connectionForTab} />
    )
  )}
</div>
```

- [ ] **Step 4: Typecheck + lint**

```bash
pnpm typecheck && pnpm lint
```

- [ ] **Step 5: Commit**

```bash
git add src/features/explain/ExplainView.tsx src/features/results/ResultsHeader.tsx src/features/editor/EditorPane.tsx
git commit -m "feat(week5): ExplainView stub + Rows/Plan toggle"
```

---

## Task 13: Frontend — `PlanTree.tsx`

**Goal:** Indented tree + self-time bar + keyboard navigation.

**Files:**

- Create: `src/features/explain/PlanTree.tsx`
- Create: `src/features/explain/PlanTree.test.tsx`
- Modify: `src/features/explain/ExplainView.tsx`

**Steps:**

- [ ] **Step 1: Implement `PlanTree.tsx`**

```tsx
import { useEffect, useMemo, useRef } from "react";

import type { PlanNode } from "@/lib/explain/planTypes";

interface Props {
  root: PlanNode;
  selectedPath: number[];
  onSelect: (path: number[]) => void;
  planOnly: boolean;
}

interface FlatRow {
  node: PlanNode;
  depth: number;
  path: number[];
}

export function PlanTree({ root, selectedPath, onSelect, planOnly }: Props) {
  const rows = useMemo(() => flatten(root), [root]);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!containerRef.current?.contains(document.activeElement)) return;
      const idx = rows.findIndex((r) => samePath(r.path, selectedPath));
      if (e.key === "ArrowDown") {
        e.preventDefault();
        const next = rows[Math.min(rows.length - 1, idx + 1)] ?? rows[0];
        onSelect(next.path);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        const next = rows[Math.max(0, idx - 1)] ?? rows[0];
        onSelect(next.path);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [rows, selectedPath, onSelect]);

  return (
    <div
      ref={containerRef}
      tabIndex={0}
      className="font-mono text-xs outline-none"
      role="tree"
      aria-label="Explain plan tree"
    >
      {rows.map((r, i) => (
        <Row
          key={i}
          row={r}
          selected={samePath(r.path, selectedPath)}
          onSelect={onSelect}
          planOnly={planOnly}
        />
      ))}
    </div>
  );
}

function Row({
  row,
  selected,
  onSelect,
  planOnly,
}: {
  row: FlatRow;
  selected: boolean;
  onSelect: (path: number[]) => void;
  planOnly: boolean;
}) {
  const ratio = planOnly
    ? row.node.selfCostRatio
    : (row.node.selfTimeRatio ?? row.node.selfCostRatio);
  const heavy = ratio >= 0.3;
  const widthPct = Math.min(100, Math.max(0, ratio * 100));
  return (
    <button
      type="button"
      onClick={() => onSelect(row.path)}
      className={`relative flex w-full items-center gap-2 px-2 py-1 text-left ${
        selected ? "bg-accent/40" : "hover:bg-accent/20"
      } ${heavy ? "border-l-2 border-l-red-500" : "border-l-2 border-l-transparent"}`}
      role="treeitem"
      aria-selected={selected}
    >
      <span
        aria-hidden
        className="absolute inset-y-0 left-0 bg-amber-500/30"
        style={{ width: `${widthPct}%` }}
      />
      <span className="relative flex-1" style={{ paddingLeft: row.depth * 14 }}>
        <span className="text-muted-foreground">▸</span>{" "}
        <span className="font-medium">{row.node.nodeType}</span>
        {row.node.relationName && (
          <span className="text-muted-foreground">
            {" "}
            · {row.node.schema ?? "public"}.{row.node.relationName}
          </span>
        )}
        {!planOnly && row.node.selfMs !== null && (
          <span className="text-muted-foreground">
            {" "}
            · {row.node.selfMs.toFixed(1)} ms
          </span>
        )}
        <span className="text-muted-foreground">
          {" · "}
          {row.node.actualRows ?? row.node.planRows}{" "}
          {planOnly ? "est rows" : "rows"}
        </span>
        {heavy && <span className="text-red-500"> ⚠</span>}
      </span>
    </button>
  );
}

function flatten(node: PlanNode, depth = 0, path: number[] = []): FlatRow[] {
  const out: FlatRow[] = [{ node, depth, path }];
  node.children.forEach((c, i) => {
    out.push(...flatten(c, depth + 1, [...path, i]));
  });
  return out;
}

function samePath(a: number[], b: number[]): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}
```

- [ ] **Step 2: Test the tree**

Create `src/features/explain/PlanTree.test.tsx`:

```tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { PlanTree } from "./PlanTree";
import type { PlanNode } from "@/lib/explain/planTypes";

const tree: PlanNode = {
  nodeType: "Hash Join",
  startupCost: 0,
  totalCost: 100,
  planRows: 100,
  planWidth: 32,
  actualStartupTime: 0,
  actualTotalTime: 12,
  actualLoops: 1,
  actualRows: 100,
  rowsRemovedByFilter: null,
  buffers: null,
  children: [
    {
      nodeType: "Seq Scan",
      relationName: "users",
      schema: "public",
      startupCost: 0,
      totalCost: 50,
      planRows: 50,
      planWidth: 32,
      actualStartupTime: 0,
      actualTotalTime: 8,
      actualLoops: 1,
      actualRows: 50,
      rowsRemovedByFilter: 0,
      buffers: null,
      children: [],
      selfMs: 8,
      selfTimeRatio: 8 / 12,
      selfCostRatio: 0.5,
    },
  ],
  selfMs: 4,
  selfTimeRatio: 4 / 12,
  selfCostRatio: 0.5,
};

describe("PlanTree", () => {
  it("renders all nodes", () => {
    render(
      <PlanTree
        root={tree}
        selectedPath={[]}
        onSelect={() => {}}
        planOnly={false}
      />,
    );
    expect(screen.getByText(/Hash Join/)).toBeInTheDocument();
    expect(screen.getByText(/Seq Scan/)).toBeInTheDocument();
  });

  it("calls onSelect with the right path", () => {
    const onSelect = vi.fn();
    render(
      <PlanTree
        root={tree}
        selectedPath={[]}
        onSelect={onSelect}
        planOnly={false}
      />,
    );
    fireEvent.click(screen.getByText(/Seq Scan/));
    expect(onSelect).toHaveBeenCalledWith([0]);
  });

  it("flags heavy node with ⚠", () => {
    render(
      <PlanTree
        root={tree}
        selectedPath={[]}
        onSelect={() => {}}
        planOnly={false}
      />,
    );
    expect(screen.getAllByText(/⚠/).length).toBeGreaterThan(0);
  });
});
```

- [ ] **Step 3: Replace placeholder in `ExplainView`**

Edit `src/features/explain/ExplainView.tsx`:

```tsx
import { PlanTree } from "./PlanTree";
import { useTabs } from "@/store/tabs";

export function ExplainView({
  tabId,
  result,
}: {
  tabId: string;
  result: ExplainResult;
}) {
  const tab = useTabs((s) => s.tabs.find((t) => t.id === tabId));
  const selectedPath = tab?.lastPlan?.selectedNodePath ?? [];
  const setSelectedNodePath = useTabs((s) => s.setSelectedNodePath);
  const planOnly =
    result.mode === "dml-plan-only" || result.mode === "ddl-plan-only";

  return (
    <div className="flex h-full flex-col">
      <header className="border-border bg-muted/30 flex items-center gap-2 border-b px-3 py-1.5 text-xs">
        <span className="rounded bg-amber-500/20 px-2 py-0.5">
          {result.mode}
        </span>
        {result.totalMs !== null && (
          <span className="text-muted-foreground">
            {result.totalMs.toFixed(1)} ms
          </span>
        )}
        {result.warnings.length > 0 && (
          <span className="text-amber-600" title={result.warnings.join("\n")}>
            ⚠ {result.warnings.length} warning(s)
          </span>
        )}
      </header>
      <div className="grid flex-1 grid-cols-[1.5fr_1fr] overflow-hidden">
        <div className="overflow-auto border-r">
          <PlanTree
            root={result.plan}
            selectedPath={selectedPath}
            onSelect={(p) => setSelectedNodePath(tabId, p)}
            planOnly={planOnly}
          />
        </div>
        <div className="text-muted-foreground overflow-auto p-3 text-xs">
          Node detail — coming up in Task 14.
        </div>
      </div>
    </div>
  );
}
```

Update the call site in `EditorPane.tsx`:

```tsx
<ExplainView tabId={activeTab.id} result={activeTab.lastPlan.result} />
```

- [ ] **Step 4: Run frontend tests**

```bash
pnpm test src/features/explain/
```

Expected: green.

- [ ] **Step 5: Commit**

```bash
git add src/features/explain src/features/editor/EditorPane.tsx
git commit -m "feat(week5): PlanTree with self-time bar + keyboard navigation"
```

---

## Task 14: Frontend — `PlanNodeDetail.tsx`

**Goal:** Replace right-side placeholder with full node metrics.

**Files:**

- Create: `src/features/explain/PlanNodeDetail.tsx`
- Create: `src/features/explain/PlanNodeDetail.test.tsx`
- Modify: `src/features/explain/ExplainView.tsx`

**Steps:**

- [ ] **Step 1: Implement**

```tsx
import type { PlanNode } from "@/lib/explain/planTypes";

interface Props {
  node: PlanNode | null;
  planOnly: boolean;
}

function row(label: string, value: React.ReactNode | null | undefined) {
  if (value === null || value === undefined || value === "") return null;
  return (
    <div className="grid grid-cols-[120px_1fr] gap-2 py-0.5">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-mono">{value}</span>
    </div>
  );
}

export function PlanNodeDetail({ node, planOnly }: Props) {
  if (!node) {
    return (
      <div className="text-muted-foreground p-3 text-xs">
        Click a node to inspect.
      </div>
    );
  }

  const rowsRow = `${node.actualRows ?? "?"} actual / ${node.planRows} estimated`;
  const buffersRow = node.buffers
    ? `hit=${node.buffers.hit} read=${node.buffers.read} written=${node.buffers.written}`
    : null;

  return (
    <div className="p-3 text-xs">
      <h4 className="mb-2 text-sm font-semibold">
        {node.nodeType}
        {node.joinType && (
          <span className="text-muted-foreground"> · {node.joinType}</span>
        )}
      </h4>
      {row(
        "Relation",
        node.relationName
          ? `${node.schema ?? "public"}.${node.relationName}`
          : null,
      )}
      {row("Alias", node.alias)}
      {row("Filter", node.filter)}
      {row("Index Cond", node.indexCond)}
      {row("Hash Cond", node.hashCond)}
      {row("Merge Cond", node.mergeCond)}
      {row("Rows", rowsRow)}
      {!planOnly &&
        row(
          "Time",
          node.actualTotalTime !== null
            ? `total ${node.actualTotalTime.toFixed(2)} ms · self ${(node.selfMs ?? 0).toFixed(2)} ms`
            : null,
        )}
      {!planOnly && row("Loops", node.actualLoops)}
      {row(
        "Cost",
        `startup ${node.startupCost.toFixed(2)} · total ${node.totalCost.toFixed(2)}`,
      )}
      {row("Buffers", buffersRow)}
      {row(
        "Output",
        node.output && node.output.length > 0
          ? node.output.slice(0, 8).join(", ")
          : null,
      )}
    </div>
  );
}
```

- [ ] **Step 2: Test**

Create `src/features/explain/PlanNodeDetail.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";

import { PlanNodeDetail } from "./PlanNodeDetail";

describe("PlanNodeDetail", () => {
  it("renders 'click a node to inspect' when null", () => {
    render(<PlanNodeDetail node={null} planOnly={false} />);
    expect(screen.getByText(/Click a node/i)).toBeInTheDocument();
  });

  it("renders relation + filter + time", () => {
    render(
      <PlanNodeDetail
        node={{
          nodeType: "Seq Scan",
          relationName: "users",
          schema: "public",
          startupCost: 0,
          totalCost: 50,
          planRows: 50,
          planWidth: 32,
          actualStartupTime: 0,
          actualTotalTime: 8,
          actualLoops: 1,
          actualRows: 50,
          rowsRemovedByFilter: 0,
          filter: "(email = 'a')",
          buffers: null,
          children: [],
          selfMs: 8,
          selfTimeRatio: 8 / 12,
          selfCostRatio: 0.5,
        }}
        planOnly={false}
      />,
    );
    expect(screen.getByText(/public.users/)).toBeInTheDocument();
    expect(screen.getByText(/email = 'a'/)).toBeInTheDocument();
    expect(screen.getByText(/8\.00 ms/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 3: Wire into `ExplainView`**

Edit `src/features/explain/ExplainView.tsx`:

```tsx
import { PlanNodeDetail } from "./PlanNodeDetail";
```

Replace the right column placeholder with:

```tsx
<div className="overflow-auto">
  <PlanNodeDetail
    node={selectedNode(result.plan, selectedPath)}
    planOnly={planOnly}
  />
</div>
```

Helper at the bottom:

```tsx
function selectedNode(root: PlanNode, path: number[]): PlanNode | null {
  let cur: PlanNode | undefined = root;
  for (const idx of path) {
    cur = cur?.children[idx];
    if (!cur) return null;
  }
  return cur ?? null;
}
```

(Add `import type { PlanNode } from "@/lib/explain/planTypes";` if not already.)

- [ ] **Step 4: Run tests**

```bash
pnpm test src/features/explain
```

- [ ] **Step 5: Commit**

```bash
git add src/features/explain
git commit -m "feat(week5): PlanNodeDetail panel"
```

---

## Task 15: Frontend — `IndexCandidates.tsx`

**Goal:** Render verified candidates with `CREATE INDEX` SQL and "Insert into editor" action.

**Files:**

- Create: `src/features/explain/IndexCandidates.tsx`
- Modify: `src/features/explain/ExplainView.tsx`

**Steps:**

- [ ] **Step 1: Implement**

```tsx
import type { IndexCandidate } from "@/lib/explain/planTypes";

interface Props {
  candidates: IndexCandidate[];
  onInsert: (sql: string) => void;
}

export function IndexCandidates({ candidates, onInsert }: Props) {
  if (candidates.length === 0) {
    return (
      <div className="text-muted-foreground p-3 text-xs">
        No verified index candidates. (No high-selectivity Seq Scan filters
        detected.)
      </div>
    );
  }
  return (
    <div className="grid grid-cols-1 gap-2 p-3 text-xs md:grid-cols-2">
      {candidates.map((c, i) => {
        const sql = `CREATE INDEX ON ${escIdent(c.schema)}.${escIdent(c.table)} (${c.columns.map(escIdent).join(", ")});`;
        return (
          <div
            key={`${c.schema}.${c.table}.${c.columns.join(",")}.${i}`}
            className={`bg-muted/30 rounded border p-2 ${
              c.verdict === "likely" ? "border-amber-500/60" : "border-border"
            }`}
          >
            <div className="mb-1 flex items-center justify-between">
              <span className="font-mono">
                {c.schema}.{c.table}({c.columns.join(", ")})
              </span>
              <span
                className={`rounded px-2 py-0.5 text-[10px] ${
                  c.verdict === "likely"
                    ? "bg-amber-500/30"
                    : "bg-muted text-muted-foreground"
                }`}
              >
                {c.verdict}
              </span>
            </div>
            <pre className="bg-background mb-1 overflow-x-auto rounded p-2 text-[11px]">
              {sql}
            </pre>
            <div className="text-muted-foreground mb-1 text-[11px]">
              {c.reason} · selectivity{" "}
              {c.selectivityEstimate !== null
                ? c.selectivityEstimate.toFixed(3)
                : "unknown"}
              {c.nDistinct !== null && ` · n_distinct=${c.nDistinct}`}
            </div>
            <button
              type="button"
              className="border-input hover:bg-accent rounded border px-2 py-0.5 text-[11px]"
              onClick={() => onInsert(sql)}
            >
              Insert into editor
            </button>
          </div>
        );
      })}
    </div>
  );
}

function escIdent(s: string): string {
  if (/^[a-z_][a-z0-9_]*$/.test(s)) return s;
  return `"${s.replace(/"/g, '""')}"`;
}
```

- [ ] **Step 2: Render in `ExplainView`**

Edit `src/features/explain/ExplainView.tsx`. Add:

```tsx
import { IndexCandidates } from "./IndexCandidates";
```

Below the grid (still inside the outer flex column):

```tsx
<IndexCandidates
  candidates={result.verifiedCandidates}
  onInsert={(sql) => {
    const t = useTabs.getState();
    const tab = t.tabs.find((x) => x.id === tabId);
    if (!tab) return;
    const next = tab.sql + (tab.sql.endsWith("\n") ? "" : "\n") + sql + "\n";
    t.updateSql(tabId, next);
  }}
/>
```

- [ ] **Step 3: Typecheck + lint**

```bash
pnpm typecheck && pnpm lint
```

- [ ] **Step 4: Commit**

```bash
git add src/features/explain
git commit -m "feat(week5): IndexCandidates with Insert-into-editor"
```

---

## Task 16: Frontend — `lib/ai/explainPrompts.ts` + `explainStream.ts`

**Goal:** Build prompts (with relation context + token budget) and call streamText.

**Files:**

- Create: `src/lib/ai/explainPrompts.ts`
- Create: `src/lib/ai/explainStream.ts`
- Create: `src/lib/ai/explainPrompts.test.ts`

**Steps:**

- [ ] **Step 1: Write tests**

Create `src/lib/ai/explainPrompts.test.ts`:

```ts
import { describe, expect, it } from "vitest";

import { buildExplainUserPrompt, compactPlanText } from "./explainPrompts";
import type { ExplainResult, PlanNode } from "@/lib/explain/planTypes";

const seqScan: PlanNode = {
  nodeType: "Seq Scan",
  relationName: "users",
  schema: "public",
  startupCost: 0,
  totalCost: 50,
  planRows: 50,
  planWidth: 32,
  actualStartupTime: 0,
  actualTotalTime: 8,
  actualLoops: 1,
  actualRows: 50,
  rowsRemovedByFilter: 0,
  filter: "(email = 'a')",
  buffers: null,
  children: [],
  selfMs: 8,
  selfTimeRatio: 1,
  selfCostRatio: 1,
};

const result: ExplainResult = {
  mode: "select-analyze",
  planJson: { Plan: { "Node Type": "Seq Scan" } as never },
  plan: seqScan,
  warnings: [],
  verifiedCandidates: [
    {
      schema: "public",
      table: "users",
      columns: ["email"],
      reason: "rows-removed-by-filter",
      verdict: "likely",
      selectivityEstimate: 0.001,
      nDistinct: -1,
      nullFrac: 0,
    },
  ],
  totalMs: 8,
  executedAt: 0,
};

describe("compactPlanText", () => {
  it("indents children", () => {
    const text = compactPlanText(seqScan);
    expect(text).toContain("Seq Scan");
    expect(text).toContain("users");
  });
});

describe("buildExplainUserPrompt", () => {
  it("includes verified candidates JSON and original SQL", () => {
    const out = buildExplainUserPrompt({
      result,
      sql: "SELECT * FROM users WHERE email='a'",
      relations: [
        {
          schema: "public",
          table: "users",
          ddl: "CREATE TABLE public.users (id int, email text)",
          indexes: ["users_pkey ON id"],
          stats: { rowEstimate: 50000 },
        },
      ],
      tokenBudget: 8000,
    });
    expect(out).toContain("Verified candidates");
    expect(out).toContain("CREATE TABLE public.users");
    expect(out).toContain("SELECT * FROM users WHERE email='a'");
  });

  it("drops DDL bodies when over token budget", () => {
    const giantDdl = "x".repeat(40_000);
    const out = buildExplainUserPrompt({
      result,
      sql: "SELECT 1",
      relations: [
        {
          schema: "public",
          table: "users",
          ddl: giantDdl,
          indexes: [],
          stats: { rowEstimate: 50000 },
        },
      ],
      tokenBudget: 1000,
    });
    expect(out).not.toContain(giantDdl);
    expect(out).toContain("(DDL omitted: token budget)");
  });
});
```

- [ ] **Step 2: Implement `explainPrompts.ts`**

```ts
import type { ExplainResult, PlanNode } from "@/lib/explain/planTypes";

export const SYSTEM_EXPLAIN_PROMPT = `You are a Postgres performance reviewer. You receive an EXPLAIN plan plus relation context and produce two artefacts:

1. A single-paragraph plain-English summary identifying the dominant bottleneck. Mention specific node types and durations. Do not narrate the whole tree.
2. A JSON array of index recommendations. ONLY recommend an index if it is likely to help the supplied plan. Do not invent statistics. Prefer composites only when the plan shows multiple correlated filter columns. Skip recommendations whose selectivity is clearly poor (the user has already filtered low-cardinality candidates server-side).

Output exactly two fenced blocks in this order:

\`\`\`summary
<one paragraph>
\`\`\`

\`\`\`json
[
  {
    "schema": "...", "table": "...", "columns": ["..."],
    "type": "btree" | "composite" | "partial",
    "where": "<partial predicate, optional>",
    "reason": "<short>",
    "priority": "high" | "medium" | "low"
  }
]
\`\`\`

If you have nothing useful to recommend, output an empty array \`[]\` in the json block.`;

export interface RelationContext {
  schema: string;
  table: string;
  ddl: string;
  indexes: string[];
  stats: { rowEstimate?: number };
}

export interface BuildExplainPromptArgs {
  result: ExplainResult;
  sql: string;
  relations: RelationContext[];
  tokenBudget: number;
}

export function buildExplainUserPrompt(args: BuildExplainPromptArgs): string {
  const { result, sql, relations } = args;
  const planText = compactPlanText(result.plan);
  const candidates = JSON.stringify(result.verifiedCandidates, null, 2);

  let relationsBlock = relations.map((r) => relationFull(r)).join("\n\n");
  let prompt = compose({ planText, relationsBlock, candidates, sql });

  if (estimateTokens(prompt) > args.tokenBudget) {
    relationsBlock = relations.map((r) => relationStatsOnly(r)).join("\n\n");
    prompt = compose({ planText, relationsBlock, candidates, sql });
  }
  return prompt;
}

function compose({
  planText,
  relationsBlock,
  candidates,
  sql,
}: {
  planText: string;
  relationsBlock: string;
  candidates: string;
  sql: string;
}): string {
  return [
    "Plan (compact tree):",
    planText,
    "",
    "Relations involved:",
    relationsBlock,
    "",
    "Verified candidates (server-side cardinality-filtered):",
    candidates,
    "",
    "Original SQL:",
    sql,
  ].join("\n");
}

function relationFull(r: RelationContext): string {
  return [
    `-- ${r.schema}.${r.table}`,
    r.ddl,
    "Indexes:",
    r.indexes.length === 0
      ? "  (none)"
      : r.indexes.map((i) => `  - ${i}`).join("\n"),
    `Stats: rows≈${r.stats.rowEstimate ?? "?"}`,
  ].join("\n");
}

function relationStatsOnly(r: RelationContext): string {
  return [
    `-- ${r.schema}.${r.table}`,
    "(DDL omitted: token budget)",
    "Indexes:",
    r.indexes.length === 0
      ? "  (none)"
      : r.indexes.map((i) => `  - ${i}`).join("\n"),
    `Stats: rows≈${r.stats.rowEstimate ?? "?"}`,
  ].join("\n");
}

function estimateTokens(s: string): number {
  return Math.ceil(s.length / 4);
}

export function compactPlanText(node: PlanNode, depth = 0): string {
  const indent = "  ".repeat(depth);
  const rel = node.relationName
    ? ` ${node.schema ?? "public"}.${node.relationName}`
    : "";
  const ms =
    node.actualTotalTime !== null
      ? ` ${node.actualTotalTime.toFixed(2)}ms`
      : ` cost=${node.totalCost.toFixed(0)}`;
  const rows = ` rows=${node.actualRows ?? node.planRows}`;
  const filter = node.filter
    ? ` filter=${node.filter}`
    : node.indexCond
      ? ` cond=${node.indexCond}`
      : "";
  const head = `${indent}${node.nodeType}${rel}${ms}${rows}${filter}`;
  const kids = node.children
    .map((c) => compactPlanText(c, depth + 1))
    .join("\n");
  return kids ? `${head}\n${kids}` : head;
}
```

- [ ] **Step 3: Implement `explainStream.ts`**

````ts
import { streamText, type LanguageModel } from "ai";

import type {
  AiInterpretation,
  AiIndexRecommendation,
} from "@/lib/explain/planTypes";

export interface StreamExplainArgs {
  model: LanguageModel;
  systemPrompt: string;
  userPrompt: string;
  signal?: AbortSignal;
  onChunk?: (text: string) => void;
}

export async function streamExplainInterpretation(
  args: StreamExplainArgs,
): Promise<AiInterpretation> {
  const started = performance.now();
  let buf = "";
  const r = await streamText({
    model: args.model,
    system: args.systemPrompt,
    prompt: args.userPrompt,
    abortSignal: args.signal,
  });
  for await (const chunk of r.textStream) {
    buf += chunk;
    args.onChunk?.(buf);
  }
  const usage = await r.usage;
  return {
    summary: extractFenced(buf, "summary") ?? buf.trim(),
    recommendations: parseRecommendations(extractFenced(buf, "json") ?? "[]"),
    promptTokens: usage?.inputTokens ?? undefined,
    completionTokens: usage?.outputTokens ?? undefined,
    durationMs: Math.round(performance.now() - started),
  };
}

function extractFenced(text: string, tag: string): string | null {
  const re = new RegExp("```" + tag + "\\s*([\\s\\S]+?)```", "m");
  const m = re.exec(text);
  return m?.[1]?.trim() ?? null;
}

function parseRecommendations(json: string): AiIndexRecommendation[] {
  try {
    const arr = JSON.parse(json);
    if (!Array.isArray(arr)) return [];
    return arr
      .filter(
        (x): x is AiIndexRecommendation =>
          typeof x === "object" &&
          x !== null &&
          typeof x.schema === "string" &&
          typeof x.table === "string" &&
          Array.isArray(x.columns),
      )
      .map((x) => ({
        schema: x.schema,
        table: x.table,
        columns: x.columns.filter((c): c is string => typeof c === "string"),
        type: (x.type as AiIndexRecommendation["type"]) ?? "btree",
        where: typeof x.where === "string" ? x.where : undefined,
        reason: typeof x.reason === "string" ? x.reason : "",
        priority:
          x.priority === "high" ||
          x.priority === "medium" ||
          x.priority === "low"
            ? x.priority
            : "medium",
      }));
  } catch {
    return [];
  }
}
````

- [ ] **Step 4: Run tests**

```bash
pnpm test src/lib/ai/explainPrompts.test.ts
```

Expected: green.

- [ ] **Step 5: Commit**

```bash
git add src/lib/ai/explainPrompts.ts src/lib/ai/explainStream.ts src/lib/ai/explainPrompts.test.ts
git commit -m "feat(week5): explainPrompts + explainStream"
```

---

## Task 17: Frontend — `PlanAiStrip.tsx`

**Goal:** Bottom strip that calls the LLM on demand (or auto when settings.autoInterpretPlan), caches by plan SHA + model, records to `ai_explain`.

**Files:**

- Create: `src/features/explain/PlanAiStrip.tsx`
- Modify: `src/features/explain/ExplainView.tsx`
- Modify: `src/features/editor/EditorPane.tsx`

**Steps:**

- [ ] **Step 1: Implement**

Create `src/features/explain/PlanAiStrip.tsx`:

```tsx
import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { buildModel } from "@/lib/ai/providers";
import { aiSecretGet } from "@/lib/keychain";
import {
  SYSTEM_EXPLAIN_PROMPT,
  buildExplainUserPrompt,
  type RelationContext,
} from "@/lib/ai/explainPrompts";
import { streamExplainInterpretation } from "@/lib/ai/explainStream";
import { planSha } from "@/lib/explain/planSha";
import type {
  AiInterpretation,
  ExplainResult,
  IndexCandidate,
} from "@/lib/explain/planTypes";
import { useAi } from "@/store/ai";
import { useSettings } from "@/store/settings";
import { useTabs } from "@/store/tabs";

interface Props {
  tabId: string;
  connId: string;
  result: ExplainResult;
  sql: string;
}

export function PlanAiStrip({ tabId, connId, result, sql }: Props) {
  const tab = useTabs((s) => s.tabs.find((t) => t.id === tabId));
  const cacheAi = useTabs((s) => s.cacheAi);
  const setActiveAiKey = useTabs((s) => s.setActiveAiKey);
  const ai = useAi((s) => s.providers);
  const settings = useSettings();
  const [busy, setBusy] = useState(false);
  const [streamed, setStreamed] = useState("");
  const ctrlRef = useRef<AbortController | null>(null);
  const [cacheKey, setCacheKey] = useState<string | null>(null);

  const provider = settings.defaultGenerationProvider;
  const cfg = ai[provider];
  const model = cfg.generationModel;

  useEffect(() => {
    let cancelled = false;
    void planSha({ plan: result.planJson, provider, model }).then((sha) => {
      if (!cancelled) setCacheKey(sha);
    });
    return () => {
      cancelled = true;
    };
  }, [result.planJson, provider, model]);

  const cached = useMemo(() => {
    if (!cacheKey) return null;
    return tab?.lastPlan?.aiCacheByKey[cacheKey] ?? null;
  }, [tab, cacheKey]);

  const doInterpret = async () => {
    if (!cacheKey) return;
    setBusy(true);
    setStreamed("");
    ctrlRef.current?.abort();
    const ctrl = new AbortController();
    ctrlRef.current = ctrl;

    try {
      const apiKey = await aiSecretGet(provider);
      if (!apiKey && provider !== "ollama") {
        toast.error(`${provider} key not set — open Settings`);
        return;
      }
      const relations = await fetchRelations(connId, result.verifiedCandidates);
      const userPrompt = buildExplainUserPrompt({
        result,
        sql,
        relations,
        tokenBudget: settings.explainTokenBudget,
      });
      const m = buildModel({
        provider,
        modelId: model,
        apiKey: apiKey ?? "",
        baseUrl: cfg.baseUrl,
      });

      const interp: AiInterpretation = await streamExplainInterpretation({
        model: m,
        systemPrompt: SYSTEM_EXPLAIN_PROMPT,
        userPrompt,
        signal: ctrl.signal,
        onChunk: setStreamed,
      });

      cacheAi(tabId, cacheKey, interp);
      setActiveAiKey(tabId, cacheKey);

      await invoke("record_ai_explain", {
        payload: {
          connId,
          planSha: cacheKey,
          provider,
          model,
          summary: interp.summary,
          rawPlanJson: JSON.stringify(result.planJson),
          verifiedCandidatesJson: JSON.stringify(result.verifiedCandidates),
          llmRecommendationsJson: JSON.stringify(interp.recommendations),
          promptTokens: interp.promptTokens ?? null,
          completionTokens: interp.completionTokens ?? null,
          durationMs: interp.durationMs,
        },
      }).catch((e) =>
        toast.error(
          `Failed to record AI explain: ${e instanceof Error ? e.message : e}`,
        ),
      );
    } catch (e) {
      if (ctrl.signal.aborted) {
        toast("Interpretation cancelled");
      } else {
        toast.error(
          `Interpretation failed: ${e instanceof Error ? e.message : e}`,
        );
      }
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    if (!cacheKey || cached || busy || !settings.autoInterpretPlan) return;
    void doInterpret();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cacheKey, cached, settings.autoInterpretPlan]);

  return (
    <div className="border-border bg-muted/30 flex flex-col gap-2 border-t p-3 text-xs">
      <div className="flex items-center gap-2">
        <span className="font-semibold">AI interpretation</span>
        <span className="text-muted-foreground">
          {provider} · {model}
        </span>
        {!cached && (
          <Button
            size="sm"
            disabled={busy}
            onClick={doInterpret}
            className="ml-auto"
          >
            {busy ? "Streaming…" : "Interpret with AI"}
          </Button>
        )}
        {cached && (
          <Button
            size="sm"
            variant="ghost"
            className="ml-auto"
            onClick={doInterpret}
            disabled={busy}
            title="Re-run interpretation"
          >
            Re-run
          </Button>
        )}
      </div>
      <div className="leading-relaxed whitespace-pre-wrap">
        {cached?.summary ?? streamed ?? ""}
      </div>
      {cached && cached.recommendations.length > 0 && (
        <div>
          <div className="text-muted-foreground mb-1">AI recommendations</div>
          <ul className="ml-4 list-disc">
            {cached.recommendations.map((r, i) => (
              <li key={i}>
                <span className="font-mono">
                  {r.schema}.{r.table}({r.columns.join(", ")})
                </span>{" "}
                — {r.priority} · {r.reason}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

async function fetchRelations(
  connId: string,
  candidates: IndexCandidate[],
): Promise<RelationContext[]> {
  const set = new Map<string, RelationContext>();
  for (const c of candidates) {
    const key = `${c.schema}.${c.table}`;
    if (set.has(key)) continue;
    try {
      const ddl = await invoke<string>("get_table_schema", {
        connectionId: connId,
        schema: c.schema,
        table: c.table,
      });
      const indexes = await invoke<{ name: string; def: string }[]>(
        "list_indexes",
        { connectionId: connId, schema: c.schema, table: c.table },
      ).catch(() => []);
      set.set(key, {
        schema: c.schema,
        table: c.table,
        ddl,
        indexes: indexes.map((i) => `${i.name} ${i.def}`),
        stats: {},
      });
    } catch {
      // best-effort — relation context is optional.
    }
  }
  return [...set.values()];
}
```

- [ ] **Step 2: Update `ExplainView` to accept connId + sql and render strip**

Edit `src/features/explain/ExplainView.tsx`. Update the props:

```tsx
export function ExplainView({
  tabId,
  connId,
  sql,
  result,
}: {
  tabId: string;
  connId: string;
  sql: string;
  result: ExplainResult;
}) {
```

Add import:

```tsx
import { PlanAiStrip } from "./PlanAiStrip";
```

After `IndexCandidates`, render `PlanAiStrip`:

```tsx
<PlanAiStrip tabId={tabId} connId={connId} result={result} sql={sql} />
```

- [ ] **Step 3: Pass connId + sql from `EditorPane`**

Edit `src/features/editor/EditorPane.tsx`. Replace the Plan branch:

```tsx
{
  activeTab.resultMode === "plan" && activeTab.lastPlan && connectionForTab ? (
    <ExplainView
      tabId={activeTab.id}
      connId={connectionForTab}
      sql={activeTab.sql}
      result={activeTab.lastPlan.result}
    />
  ) : (
    activeTab.lastResult &&
    connectionForTab && (
      <ResultsGrid result={activeTab.lastResult} connId={connectionForTab} />
    )
  );
}
```

- [ ] **Step 4: Typecheck + lint**

```bash
pnpm typecheck && pnpm lint
```

- [ ] **Step 5: Commit**

```bash
git add src/features/explain src/features/editor/EditorPane.tsx
git commit -m "feat(week5): PlanAiStrip — interpret + cache + record_ai_explain"
```

---

## Task 18: Frontend — `Cmd+Shift+E` + Explain button in `EditorPane`

**Goal:** Trigger `runExplainGate` and store the plan via `setPlan`.

**Files:**

- Modify: `src/features/editor/EditorPane.tsx`

**Steps:**

- [ ] **Step 1: Add the action**

Edit `src/features/editor/EditorPane.tsx`. Add import:

```tsx
import { runExplainGate } from "@/features/explain/explainGate";
```

Add a `runExplainAction` callback alongside `run`:

```tsx
const runExplainAction = useCallback(
  async (analyzeAnyway = false) => {
    if (!connectionForTab) {
      toast.error("Select a connected database first");
      return;
    }
    setBusy(activeTab.id, true);
    try {
      const r = await runExplainGate({
        connId: connectionForTab,
        sql: activeTab.sql,
        allowAnalyzeAnyway: analyzeAnyway,
      });
      if (r) useTabs.getState().setPlan(activeTab.id, r, activeTab.sql);
      else useTabs.getState().setBusy(activeTab.id, false);
    } catch (e) {
      const msg = e instanceof Error ? e.message : "Explain failed";
      setError(activeTab.id, msg);
      toast.error(msg);
    }
  },
  [activeTab.id, activeTab.sql, connectionForTab, setBusy, setError],
);
```

Extend the keyboard handler `useEffect` to bind `Cmd+Shift+E` (inside the same `onKey` body that already handles `Enter`/`t`/`w`/`k`):

```tsx
} else if (e.key.toLowerCase() === "e" && e.shiftKey) {
  e.preventDefault();
  runExplainAction(false);
}
```

Add `runExplainAction` to the `useEffect` dependency array.

Add a button in the toolbar next to Run:

```tsx
<Button
  size="sm"
  variant="outline"
  onClick={() => runExplainAction(false)}
  disabled={activeTab.busy}
>
  Explain (⌘⇧E)
</Button>
```

- [ ] **Step 2: Typecheck + lint**

```bash
pnpm typecheck && pnpm lint
```

- [ ] **Step 3: Smoke test in dev**

```bash
docker compose -f infra/postgres/docker-compose.yml up -d
pnpm tauri dev
```

Manually:

- Connect to docker DB.
- Type `SELECT 1;` → press `Cmd+Shift+E` → Plan tab populated.
- Run regular `SELECT 1;` with `Cmd+Enter` → Rows tab populates and is auto-selected.

- [ ] **Step 4: Commit**

```bash
git add src/features/editor/EditorPane.tsx
git commit -m "feat(week5): EditorPane — Cmd+Shift+E + Explain button"
```

---

## Task 19: Frontend — ANALYZE-anyway button (DML/DDL plan-only mode)

**Goal:** When mode is plan-only, expose "ANALYZE anyway" button that re-runs through `runGate` + ANALYZE-anyway path.

**Files:**

- Create: `src/features/explain/AnalyzeAnywayButton.tsx`
- Modify: `src/features/explain/ExplainView.tsx`

**Steps:**

- [ ] **Step 1: Implement the button**

Create `src/features/explain/AnalyzeAnywayButton.tsx`:

```tsx
import { Button } from "@/components/ui/button";
import { runExplainGate } from "./explainGate";
import { useTabs } from "@/store/tabs";

interface Props {
  tabId: string;
  connId: string;
  sql: string;
}

export function AnalyzeAnywayButton({ tabId, connId, sql }: Props) {
  const onClick = async () => {
    useTabs.getState().setBusy(tabId, true);
    const r = await runExplainGate({ connId, sql, allowAnalyzeAnyway: true });
    if (r) useTabs.getState().setPlan(tabId, r, sql);
    else useTabs.getState().setBusy(tabId, false);
  };
  return (
    <Button size="sm" variant="destructive" onClick={onClick}>
      ANALYZE anyway
    </Button>
  );
}
```

- [ ] **Step 2: Render badges + button in `ExplainView` header**

Edit `src/features/explain/ExplainView.tsx`. Add import:

```tsx
import { AnalyzeAnywayButton } from "./AnalyzeAnywayButton";
```

Append inside the header `<header>` block (after existing badges):

```tsx
{
  (result.mode === "dml-plan-only" || result.mode === "ddl-plan-only") && (
    <>
      <span className="rounded bg-yellow-500/20 px-2 py-0.5 text-yellow-700">
        Estimated only — would modify data
      </span>
      <AnalyzeAnywayButton tabId={tabId} connId={connId} sql={sql} />
    </>
  );
}
{
  result.mode === "analyze-anyway-rolled-back" && (
    <span className="rounded bg-green-500/20 px-2 py-0.5">
      ANALYZE (rolled back)
    </span>
  );
}
{
  result.mode === "analyze-anyway-in-tx" && (
    <span className="rounded bg-amber-500/20 px-2 py-0.5">
      ANALYZE (in active tx)
    </span>
  );
}
```

- [ ] **Step 3: Smoke test**

```bash
pnpm tauri dev
```

Manually:

1. Create a scratch table: `CREATE TABLE scratch (id int); INSERT INTO scratch VALUES (1),(2);` (Cmd+Enter).
2. New tab, type `UPDATE scratch SET id = id + 1` → `Cmd+Shift+E` → expect plan-only badge + ANALYZE-anyway button.
3. Click ANALYZE-anyway → DestructiveModal fires (UPDATE without WHERE) → confirm → result returns with `analyze-anyway-rolled-back` badge + Actual times present.
4. `SELECT * FROM scratch;` → values still 1, 2 (unchanged).

- [ ] **Step 4: Typecheck + lint**

```bash
pnpm typecheck && pnpm lint
```

- [ ] **Step 5: Commit**

```bash
git add src/features/explain
git commit -m "feat(week5): ANALYZE-anyway button + mode badges"
```

---

## Task 20: Polish — stale plan badge, depth limit test, empty state

**Goal:** Production polish before final manual verification.

**Files:**

- Modify: `src/features/explain/ExplainView.tsx`
- Modify: `src/lib/explain/planParse.test.ts`

**Steps:**

- [ ] **Step 1: Add stale-plan badge in `ExplainView`**

Edit `src/features/explain/ExplainView.tsx`. Inside the function, near the top:

```tsx
const stale = tab?.sql !== tab?.lastPlan?.sqlAtRun;
```

In the header, add:

```tsx
{
  stale && (
    <span className="rounded bg-orange-500/20 px-2 py-0.5">
      stale (sql edited)
    </span>
  );
}
```

- [ ] **Step 2: Add the depth-limit test**

Edit `src/lib/explain/planParse.test.ts`. Append:

```ts
it("cuts off at MAX_DEPTH", () => {
  let cur: any = {
    "Node Type": "X",
    "Startup Cost": 0,
    "Total Cost": 1,
    "Plan Rows": 1,
    "Plan Width": 1,
  };
  for (let i = 0; i < 110; i++) cur = { ...cur, Plans: [cur] };
  const root = parsePlan({ Plan: cur } as never);
  let depth = 0;
  let node = root;
  while (node.children.length) {
    node = node.children[0];
    depth++;
  }
  expect(depth).toBeLessThanOrEqual(100);
});
```

The implementation in T9 already enforces `MAX_DEPTH`; this test asserts it.

- [ ] **Step 3: Run full FE gate**

```bash
pnpm test
pnpm typecheck && pnpm lint && pnpm format
pnpm build
```

All green.

- [ ] **Step 4: Commit**

```bash
git add src/features/explain/ExplainView.tsx src/lib/explain/planParse.test.ts
git commit -m "chore(week5): stale plan badge + depth cutoff test"
```

---

## Task 21: Manual verification document

**Goal:** Single document a non-author can run to validate Week 5 end-to-end.

**Files:**

- Create: `docs/superpowers/plans/manual-verification-week-5.md`

**Steps:**

- [ ] **Step 1: Write the checklist**

Create `docs/superpowers/plans/manual-verification-week-5.md`:

```markdown
# Week 5 — Manual Verification Checklist

## Setup

- [ ] `pnpm install`
- [ ] `docker compose -f infra/postgres/docker-compose.yml up -d`
- [ ] Seed schema:
      CREATE TABLE w5_users (id serial primary key, email text, country text, signup_at timestamp);
      INSERT INTO w5_users (email, country, signup_at)
      SELECT 'u' || g || '@x.com',
      CASE WHEN g % 100 = 0 THEN 'KR' ELSE 'US' END,
      now() - (g || ' minutes')::interval
      FROM generate_series(1, 50000) g;
      ANALYZE w5_users;
- [ ] `pnpm tauri dev`
- [ ] Connect to `127.0.0.1:55432 / tusk_test / tusk / tusk`.

## Run

- [ ] `SELECT * FROM w5_users WHERE email = 'u1@x.com';` → Cmd+Shift+E
  - Plan tab opens, mode badge `select-analyze`, total ms shown.
  - Tree shows Seq Scan on w5_users with self-time bar near 100%.
  - Detail panel shows filter and rows.
  - Verified candidates: at least one card for `(email)` with verdict `likely`.
- [ ] Click "Insert into editor" on the email candidate → `CREATE INDEX ...` appears in editor.
- [ ] Run that CREATE INDEX with Cmd+Enter → no destructive modal (CREATE INDEX is non-destructive).
- [ ] Re-run the SELECT EXPLAIN → tree now has Index Scan, no candidates returned.

## DML

- [ ] `UPDATE w5_users SET country = 'KR' WHERE id = 1;` → Cmd+Shift+E
  - Mode badge: `dml-plan-only` + yellow `Estimated only — would modify data`.
  - "ANALYZE anyway" button visible.
- [ ] Click ANALYZE anyway. UPDATE has WHERE, so DestructiveModal does not fire.
- [ ] Mode badge updates to `analyze-anyway-rolled-back`. SELECT to verify country unchanged for id=1.
- [ ] `UPDATE w5_users SET country = 'KR';` (no WHERE) → Cmd+Shift+E → ANALYZE anyway → DestructiveModal fires → cancel → no execution.
- [ ] Click ANALYZE anyway again → confirm → mode `analyze-anyway-rolled-back`. Verify with SELECT that no rows actually changed.

## AI interpret

- [ ] With a key set in Settings, ensure auto-interpret OFF.
- [ ] Run a SELECT EXPLAIN → AI strip shows "Interpret with AI" button.
- [ ] Click → streamed summary populates → recommendations list appears.
- [ ] Re-run the same SELECT EXPLAIN → AI strip immediately shows the cached summary, no re-stream.
- [ ] Toggle auto-interpret ON in Settings → run a NEW EXPLAIN → AI auto-streams without click.
- [ ] Open `~/Library/Application Support/.../tusk.db` → query `SELECT count(*) FROM ai_explain;` → count increments only on first interpretation per cache key.

## Errors

- [ ] `BEGIN;` → Cmd+Shift+E → toast "Statement is not explainable".
- [ ] No connection → click Explain → toast "Select a connected database first".
- [ ] Missing AI key → click Interpret → toast pointing to Settings.

## Stale badge

- [ ] After running an EXPLAIN, edit the SQL in the editor → header shows `stale (sql edited)` badge until the next EXPLAIN.

## Regression

- [ ] Cmd+Enter run still works.
- [ ] DestructiveModal still fires on `DELETE FROM w5_users;` typed directly.
- [ ] Cmd+K still works.
- [ ] Cmd+P history palette includes the EXPLAIN entry under source `editor`.

## Cleanup

- [ ] `docker compose -f infra/postgres/docker-compose.yml down`.
```

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/plans/manual-verification-week-5.md
git commit -m "docs(week5): manual verification checklist"
```

---

## Task 22: Final gate + closing checklist

**Goal:** All gates green simultaneously; PR-ready.

**Files:** none

**Steps:**

- [ ] **Step 1: Run every gate**

```bash
pnpm install --frozen-lockfile
pnpm typecheck && pnpm lint && pnpm format:check
pnpm test
pnpm build
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
cargo test --manifest-path src-tauri/Cargo.toml
docker compose -f infra/postgres/docker-compose.yml up -d
cargo test --manifest-path src-tauri/Cargo.toml -- --include-ignored
```

All green expected.

- [ ] **Step 2: Run the manual checklist end-to-end**

Walk through every checkbox in `docs/superpowers/plans/manual-verification-week-5.md`.

- [ ] **Step 3: Closing checklist**

- [ ] §1 spec success criteria 10/10 verified manually.
- [ ] No new dependencies in `package.json` / `Cargo.toml` (`git diff main -- package.json src-tauri/Cargo.toml`).
- [ ] All commits use the convention from the header (no Co-Authored-By trailers).
- [ ] No `TODO` / `FIXME` left in shipped code (only intentional `MAX_DEPTH` etc.).
- [ ] PLAN.md Week 5 sub-bullets all checked.

**Done.**
