# Week 5 — EXPLAIN Visualization + AI Interpretation + Index Recommendation (design spec)

> **Date:** 2026-04-29
> **Scope:** PLAN.md Week 5 전부 — EXPLAIN ANALYZE 트리 시각화 / 노드 클릭 → 상세 패널 / AI 해석 사이드바 / 인덱스 추천 (낮은 카디널리티 거름)
> **Status:** Drafted, awaiting user review
> **Builds on:** Week 4 (BYOK + Cmd+K + 스키마 RAG + DestructiveModal/runGate + AI history)

---

## 1. Goal & success criteria

PLAN.md Week 5 네 항목을 한 번에 끝낸다. 차별점은 두 개:

1. **결정적 인덱스 추천** — Rust가 `pg_stats`로 카디널리티를 검증한 후보만 내보낸다. AI 키 없이도 동작.
2. **선택적 LLM enrichment** — 사용자가 BYOK 정신에 맞게 비용을 선택한다. plan 한 번 = LLM 호출 한 번 (캐시).

End-user 기준 성공 조건:

1. 에디터에서 `Cmd+Shift+E` 또는 Run 옆 **"Explain"** 버튼을 누르면 활성 탭의 SQL을 EXPLAIN으로 감싸 실행한다. 결과 영역 상단에 `[Rows | Plan]` 토글이 등장하고, Rows 결과가 이미 있으면 둘 다 sticky하다.
2. SELECT/CTE-with-SELECT는 자동으로 `EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)` 실행. 사용자가 SQL에 이미 `EXPLAIN`을 직접 적었으면 그대로 둔다 (이중 래핑 금지).
3. INSERT/UPDATE/DELETE/DDL이 감지되면 자동으로 `EXPLAIN (FORMAT JSON)` (ANALYZE 없음)으로 실행 + 결과 헤더에 노란 배지 `Estimated only — would modify data`. 사용자가 명시적으로 **"ANALYZE anyway"**를 누르면 `runGate` (Week 4 destructive 가드)를 통과한 후 `BEGIN ... ROLLBACK` 래핑하여 ANALYZE 실행. 트랜잭션이 이미 활성이면 사용자 책임으로 그냥 실행 (자동 래핑 안 함).
4. 좌측에 들여쓰기 텍스트 트리. 각 행 좌측에 self-time inline bar (행 width의 0~100%, `node.actual_self_time / total_time`). 가장 비싼 노드(전체의 ≥ 30%)는 빨간 좌측 보더 + ⚠ 배지.
5. 노드 클릭 시 우측 상세 패널에 표시: 노드 타입 / actual vs estimated rows / loops / total_ms / self_ms / buffers (hit/read/written) / filter / index cond / join cond / output columns. 키보드 ↑↓로도 탐색.
6. 하단 AI 스트립 — 기본은 `Interpret with AI` 버튼. 누르면 plan JSON + 등장 relation의 DDL/index/stats를 컨텍스트로 streamText 호출 → 한 단락 요약 + 병목 식별 + 추천 인덱스 우선순위. Settings의 `auto-interpret` 토글이 ON이면 plan 결과 도착 즉시 자동 호출.
7. 인덱스 추천 — Rust가 plan JSON에서 `Seq Scan` + `Filter`/`Index Cond` 페어를 추출 → `pg_stats`의 `n_distinct`/`null_frac` 조회로 카디널리티 검증 → `verified_candidates` 리스트 산출. AI OFF여도 후보 표시. AI ON이면 LLM이 enrich (자연어 설명, composite/partial 변형 제안, 우선순위).
8. 같은 plan JSON SHA + 같은 model 조합에 대해 in-memory 캐시 (탭별 `lastPlan` 안). 재실행/Rows↔Plan 토글 왕복 시 LLM 재호출 없음.
9. EXPLAIN 호출 자체는 일반 history에 source=`editor`로 1건 기록 (다른 쿼리와 동일 경로). AI 해석 호출 시점에 `ai_explain` source로 별도 entry — payload에 plan SHA + interpretation 텍스트 + verified candidates + LLM enrichment + provider/model/tokens.
10. 모든 에러는 toast — destructive 가드 거부 / 네트워크 / 비-EXPLAIN-able 쿼리(파서 fail) / `pg_stats` 조회 실패. silent fail 금지.

이 10개 충족 시 Week 5 완료.

## 2. Out of scope (Week 6+)

- pgvector 컬럼 EXPLAIN 처리 / HNSW 노드 강조 — Week 6
- `pg_stat_statements` slow-query 패널 — v1.5
- Plan diff / 두 plan 비교 — v1.5
- 저장된 plan 컬렉션 / "이 쿼리 plan 어떻게 변했지" 히스토리 시각화 — v1.5 (`ai_explain` 데이터로 가능하지만 UI는 안 만듦)
- LLM이 **직접** index DDL 실행 — 안 함. Apply 시 사용자가 `CREATE INDEX` SQL을 에디터에 inject받아 직접 Run.
- Plan visualizer의 graph(d3/react-flow) 모드 — 들여쓰기 텍스트 + inline bar로만.
- Plan JSON에 등장하지 않은 relation까지 LLM에 첨부하는 fallback — 안 함. 등장 relation만.
- 토큰/비용 텔레메트리 대시보드 — `ai_history` 테이블에 raw 값만 기록.
- multi-statement (semicolon-separated) EXPLAIN — v1엔 첫 statement만 EXPLAIN 시도 + 경고. 나머지는 v1.5.

## 3. Architecture

```
┌── Frontend (React + zustand + Vercel AI SDK) ──────────────────┐
│  features/                                                      │
│    explain/             ← NEW                                   │
│      ExplainView.tsx          [Rows|Plan] 토글 컨테이너          │
│      PlanTree.tsx             들여쓰기 트리 + self-time bar      │
│      PlanNodeDetail.tsx       선택 노드 상세                     │
│      PlanAiStrip.tsx          하단 AI 해석 + 추천                │
│      IndexCandidates.tsx      verified candidates 리스트         │
│      explainGate.ts           SELECT/DML 분기 + ANALYZE 결정      │
│      planParse.ts             EXPLAIN JSON → 화면용 노드 모델    │
│    editor/                                                      │
│      EditorPane.tsx           (확장) Explain 버튼 + Cmd+Shift+E │
│    results/                                                     │
│      ResultsHeader.tsx        (확장) Rows/Plan 모드 표시         │
│  store/                                                         │
│    tabs.ts             (확장) Tab.lastPlan: PlanState           │
│    settings.ts         (확장) autoInterpret, indexAdviceOn      │
│  lib/                                                           │
│    explain/                                                     │
│      planSha.ts               안정적 plan JSON SHA              │
│      planTypes.ts             PlanNode/Plan/IndexCandidate 등   │
│    ai/                                                          │
│      explainPrompts.ts        EXPLAIN 해석 system prompt        │
│      explainStream.ts         streamText 래퍼 (구조화 출력)     │
└─────────────────────────────────────────────────────────────────┘
                          ↕ Tauri invoke()
┌── Rust (src-tauri) ─────────────────────────────────────────────┐
│  commands/                                                      │
│    explain.rs        NEW   run_explain (mode 판별 + 실행)       │
│                            extract_index_candidates             │
│                            record_ai_explain                    │
│    sqlast.rs         (확장) classify_for_explain — SELECT vs    │
│                            DML/DDL vs already-EXPLAIN 분기       │
│    history.rs        (확장) source='ai_explain' + ai_explain    │
│  db/                                                            │
│    explain_runner.rs NEW   sqlx로 EXPLAIN ... FORMAT JSON 실행 │
│                            (BEGIN/ROLLBACK 래핑 옵션)           │
│    pg_stats.rs       NEW   per-column n_distinct/null_frac 조회 │
│    state.rs          (migration 004_ai_explain)                  │
│  errors.rs           (확장) Explain                             │
└─────────────────────────────────────────────────────────────────┘
```

LLM 호출 분배는 Week 4 그대로: **generation = 프론트** (Vercel AI SDK streamText), **embedding = Rust**. Week 5에는 embedding 이슈가 없으니 Rust 측 LLM 변경 없음.

### 왜 EXPLAIN 실행을 별도 명령으로 두는가

`execute_query`에 옵션 플래그를 추가하지 않고 별도 `run_explain` 명령으로 분리하는 이유:

- Plan 결과는 `QueryResult`(rows/columns)와 다른 구조 (`Plan` JSON + verified candidates + warnings).
- BEGIN/ROLLBACK 래핑은 EXPLAIN 전용 정책이라 일반 쿼리 경로를 오염시키지 않음.
- destructive 가드를 호출 측이 명시적으로 부르게 함 → 코드 흐름이 grep 가능.

## 4. Libraries

기본적으로 **신규 의존성 없음**. 모든 기능을 in-house로 구현.

| 영역                  | 라이브러리                 | 사유                                                                                        |
| --------------------- | -------------------------- | ------------------------------------------------------------------------------------------- |
| Plan tree 렌더링      | (없음)                     | 들여쓰기 텍스트 + div width%로 self-time bar. 외부 graph 라이브러리 의도적으로 회피.        |
| Plan SHA              | Web Crypto `subtle.digest` | 브라우저 표준. 별도 crate 불필요.                                                           |
| Postgres EXPLAIN 실행 | 기존 `sqlx`                | `EXPLAIN (...) FORMAT JSON`은 단일 row TEXT/JSON 컬럼으로 반환됨. 일반 `query` 경로 그대로. |
| pg_stats 조회         | 기존 `sqlx`                | 추가 의존성 없음. `pg_meta.rs`의 패턴을 따름.                                               |
| AI streamText         | 기존 Vercel AI SDK         | Cmd+K 패턴 그대로. tool calling은 Week 5에서 사용 안 함 (plan + 컨텍스트가 정해져 있음).    |
| Plan JSON 파싱        | `serde_json`               | 이미 Cargo에 있음.                                                                          |

## 5. Data flow

### 5.1 Explain 실행

```
[User] Cmd+Shift+E or click "Explain"
  → frontend: explainGate.classify(activeTab.sql)
      → category: "select-analyze" | "dml-plan-only" | "ddl-plan-only"
                  | "already-explain-passthrough" | "unparseable"
  → frontend: setBusy + show "Explaining…" toast (delayed 500ms like queries)
  → invoke('run_explain', {
      connectionId, sql, requestedMode, allowAnalyzeAnyway: false,
    })

  Rust run_explain:
    1. sqlast::classify_for_explain(sql) — 더블 체크
    2. category 따라 wrapped SQL 생성:
       SELECT  → EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) <sql>
       DML/DDL → EXPLAIN (FORMAT JSON) <sql>
       already-EXPLAIN → 그대로
       unparseable → TuskError::Explain("cannot classify SQL")
    3. tx_slot 활성 여부 확인:
       활성이면: 그 안에서 그대로 실행 (래핑 안 함)
       비활성 + DML/DDL ANALYZE 요청 시:
         BEGIN; <wrapped EXPLAIN>; ROLLBACK 한 connection에서 순차 실행
       그 외: pool에서 단일 실행
    4. 첫 row 첫 컬럼을 JSON으로 파싱 → Vec<PlanNode>
    5. Plan에서 등장한 relation 집합 추출
    6. extract_index_candidates(&plan, &pool) — pg_stats 조회 포함
    7. history.rs: source='editor', sql_full=wrapped_sql 그대로 1 entry 기록.
       (execute_query의 history append 로직을 db/state.rs로 이미 빠진 helper 재사용 — 신규 helper 분리 X. tx 경로와 pool 경로의 분기 코드는 run_explain에 동일하게 작성.)
    8. emit query:started/completed (일반 경로와 동일)
    9. Return ExplainResult { plan, mode, warnings, verifiedCandidates, executedAt }

  Frontend:
    - tabs.setPlan(activeTabId, ExplainResult)
    - ResultsHeader에 [Rows|Plan] 토글 표시 (Plan 활성)
    - settings.autoInterpret == true ? 즉시 LLM 호출 : 사용자 클릭 대기
```

### 5.2 LLM 해석 흐름 (사용자 클릭 또는 auto-interpret)

```
PlanAiStrip.onInterpret():
  cacheKey = sha256(planJson + provider + model)
  if (tab.lastPlan.aiCacheByKey[cacheKey]) → 즉시 표시, return

  컨텍스트 빌드 (lib/ai/explainPrompts.ts):
    - relations = plan에 등장한 (schema, table) 집합
    - per relation: invoke('get_table_schema') + invoke('list_indexes')
    - per relation: invoke('pg_stats_summary') — n_distinct/null_frac per column
    - tokenBudget 검사 (총 length): 초과 시 DDL 생략, stats 요약만 유지
    - userInputBlock = compactPlan(plan)  // tree 텍스트 형태로 압축

  invoke('aiSecretGet', provider) → apiKey
  buildModel(provider, model, apiKey)
  streamGeneration({
    systemPrompt: SYSTEM_EXPLAIN_PROMPT,
    userPrompt: relations + plan + verifiedCandidates,
    onChunk: setStreamed,
  })

  파싱: structured 출력은 fences로 두 블록 — `summary`와 `recommendations`
    summary: 한 단락 자연어
    recommendations: JSON [{ relation, columns[], type: btree|composite|partial, reason, priority }]

  tab.lastPlan.aiCacheByKey[cacheKey] = { summary, recs, tokens }
  invoke('record_ai_explain', { connId, planSha, provider, model,
                                summary, candidates, recs, tokens })
```

### 5.3 인덱스 추천 (Rust 로컬, AI 무관)

```rust
// pseudo
struct IndexCandidate {
    schema: String,
    table: String,
    columns: Vec<String>,
    reason: CandidateReason,  // SeqScanFilter | LossyIndexCond | …
    plan_node_path: Vec<usize>,
    n_distinct: Option<f64>,
    null_frac: Option<f64>,
    rows_in_table: Option<i64>,
    selectivity_estimate: Option<f64>,
    verdict: Verdict,  // Likely | Maybe | Skip
}
```

선정 규칙:

1. plan 노드 traverse 중 `Seq Scan`이고 `Rows Removed by Filter ≥ 1000` 이거나 `actual_rows ≥ 0.5 * relation_rows`인 경우 → 후보.
2. `Filter` 또는 `Index Cond`의 단순 비교(`col = ?`, `col IN (?)`, `col BETWEEN ? AND ?`)에서 컬럼 추출. 함수 적용·복잡식은 후보에서 제외 (composite/expression 인덱스는 LLM enrichment 단계로 미룸).
3. `pg_stats`에서 해당 컬럼의 `n_distinct` 조회:
   - `n_distinct > 0` → 절대 distinct (positive). `selectivity = 1 / n_distinct`.
   - `n_distinct < 0` → 비율 (negative). `selectivity = abs(n_distinct)`.
   - `null` → unknown, verdict `Maybe`.
4. `selectivity ≤ 0.05` (≤ 5% 매치)이면 `Likely`. `0.05 < selectivity ≤ 0.2` 면 `Maybe`. `> 0.2`이면 `Skip` (낮은 카디널리티 거름).
5. 최종 `verified_candidates`는 `Likely` + `Maybe` 만 반환. `Skip`은 결과에 포함시키지 않음 (사용자 노출 안 함).

이 규칙은 결정적이고, AI 없이도 의미 있는 추천을 내놓는다. 단순함이 정확도보다 우선 — 복잡한 plan에서 false-negative가 있을 수 있고, LLM enrichment가 그걸 보강한다.

### 5.4 ANALYZE-anyway 흐름 (DML/DDL의 명시적 실행)

```
사용자: PlanAiStrip 또는 ResultsHeader의 "ANALYZE anyway" 버튼 클릭
  → runGate(originalSql) — Week 4 destructive 가드를 그대로 호출
  → 통과 시: invoke('run_explain', { ... allowAnalyzeAnyway: true })
  → Rust: tx_slot 비활성이면 BEGIN; <EXPLAIN ANALYZE wrapped>; ROLLBACK
          tx_slot 활성이면 그 트랜잭션 안에서 그대로 실행 (자동 ROLLBACK 안 함)
  → 결과 받으면 모드 표시는 "ANALYZE (rolled back)" 또는 "ANALYZE (in tx)"
```

## 6. Components & contracts

### 6.1 Frontend

#### `lib/explain/planTypes.ts`

```ts
export type ExplainMode =
  | "select-analyze"
  | "dml-plan-only"
  | "ddl-plan-only"
  | "analyze-anyway-rolledback"
  | "analyze-anyway-in-tx"
  | "passthrough";

export interface PlanNode {
  nodeType: string; // "Seq Scan", "Hash Join", …
  relationName?: string;
  schema?: string;
  alias?: string;
  startupCost: number;
  totalCost: number;
  planRows: number;
  planWidth: number;
  actualStartupTime?: number; // null in plan-only
  actualTotalTime?: number;
  actualLoops?: number;
  actualRows?: number;
  rowsRemovedByFilter?: number;
  filter?: string;
  indexCond?: string;
  joinType?: string;
  buffers?: { hit: number; read: number; written: number };
  output?: string[];
  children: PlanNode[];
  /** computed: actualTotalTime − sum(child.actualTotalTime). plan-only이면 null. */
  selfMs: number | null;
  /** computed: 0~1. plan-only이면 null. */
  selfTimeRatio: number | null;
  /** computed fallback for plan-only: (totalCost − sum(child.totalCost)) / rootTotalCost. analyze 모드에선 selfTimeRatio와 동등 의미를 갖되 시간 기준이 아닌 cost 기준. */
  selfCostRatio: number;
}

export interface IndexCandidate {
  schema: string;
  table: string;
  columns: string[];
  reason: "seq-scan-filter" | "lossy-index-cond" | "rows-removed-by-filter";
  verdict: "likely" | "maybe";
  selectivityEstimate: number | null;
  nDistinct: number | null;
  nullFrac: number | null;
}

export interface ExplainResult {
  mode: ExplainMode;
  plan: PlanNode;
  warnings: string[];
  verifiedCandidates: IndexCandidate[];
  rawJson: string; // for SHA + AI context
  executedAt: number; // ms
  totalMs: number | null;
}
```

#### `features/explain/explainGate.ts`

```ts
export async function runExplainGate(args: {
  connId: string;
  sql: string;
  allowAnalyzeAnyway?: boolean;
}): Promise<ExplainResult> { … }
```

`allowAnalyzeAnyway === true`일 때만 destructive 가드를 호출. 통상 `Cmd+Shift+E`/Explain 버튼은 항상 false (자동 plan-only fallback).

#### `features/explain/PlanTree.tsx`

- 들여쓰기 텍스트 트리, 각 행:
  - 좌측 self-time bar: `<div style={{ width: `${ratio \* 100}%` }} className="bg-amber-500/30" />` 행 배경 absolute layer.
  - 텍스트: `{indent}{nodeType} ({selfMs.toFixed(1)} ms · {actualRows ?? planRows} rows)`.
  - `selfTimeRatio ≥ 0.3` 이면 좌측 빨간 보더 + ⚠.
- 키보드 ↑↓ 탐색, Enter/Space로 선택 토글 (선택은 우측 PlanNodeDetail에 반영).
- plan-only 모드면 ms 대신 estimated cost 표시 + "estimated only" 헤더. self-time bar는 `total_cost - sum(child.total_cost)` 기반 비율로 그린다 (즉 selfTimeRatio가 null이면 selfCostRatio로 fallback).

#### `features/explain/PlanNodeDetail.tsx`

선택된 노드의 모든 메트릭/제약을 정형화된 표로 표시. 노드 미선택 시 "Click a node to inspect"만.

#### `features/explain/PlanAiStrip.tsx`

- top: ExplainResult.mode 배지 + "ANALYZE anyway" (mode가 plan-only일 때만)
- middle: AI summary (스트리밍 중이면 그대로 표시) + provider/model 라벨
- bottom: `IndexCandidates` (Rust 후보) + (있다면) LLM enrichment 머지

#### `features/explain/IndexCandidates.tsx`

- 카드 리스트. 각 카드:
  - `CREATE INDEX ON schema.table (col1, col2)` SQL을 회색 박스로 표시
  - "Insert into editor" 버튼 → 활성 탭 SQL에 inject (커서 위치 기준)
  - LLM enrichment 있으면 "Why" expander
- `verdict`(likely/maybe)별 색상 구분.

#### Tabs store extension

```ts
interface PlanState {
  result: ExplainResult;
  selectedNodePath: number[];      // index path into tree
  aiCacheByKey: Record<string, AiInterpretation>;
  activeAiKey: string | null;
}
interface Tab {
  …,
  lastPlan?: PlanState;
  resultMode: "rows" | "plan";     // sticky toggle
}
```

#### Settings extension

```ts
interface SettingsState {
  …,
  autoInterpretPlan: boolean;       // default false
  indexAdviceEnabled: boolean;      // default true (Rust-only도 가치 있음)
  explainTokenBudget: number;       // default 8000 (input tokens)
}
```

### 6.2 Backend (Rust)

#### `commands/explain.rs`

```rust
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunExplainArgs {
    pub connection_id: String,
    pub sql: String,
    pub allow_analyze_anyway: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplainResult {
    pub mode: String,                 // "select-analyze" | …
    pub plan_json: serde_json::Value, // raw EXPLAIN output (top-level Plan)
    pub warnings: Vec<String>,
    pub verified_candidates: Vec<IndexCandidate>,
    pub total_ms: Option<f64>,
    pub executed_at: i64,
}

#[tauri::command]
pub async fn run_explain(...) -> TuskResult<ExplainResult> { … }
```

판별 로직(`sqlast::classify_for_explain`):

- 다중 statement → 첫 statement만 분석 + warning.
- 첫 statement가 `EXPLAIN`으로 시작 (대소문자 무관 + 옵션 형식 허용) → `passthrough`.
- 첫 statement가 `SELECT`/`WITH ... SELECT`/`VALUES`/`TABLE ...` → `select-analyze`.
- `INSERT|UPDATE|DELETE|MERGE` → `dml-plan-only`.
- `CREATE|DROP|ALTER|TRUNCATE|GRANT|REVOKE|REFRESH MATERIALIZED VIEW` → `ddl-plan-only`.
- 그 외(예: `BEGIN`, `SET`) → `TuskError::Explain("not explainable")`.

`allow_analyze_anyway` true이고 mode가 plan-only이면 wrapped SQL을 ANALYZE 형태로 다시 만든다. 이때:

- `tx_slot` 활성이면 그 connection으로 그대로 실행 (warn: "Executed inside active transaction — caller responsibility for rollback").
- `tx_slot` 비활성이면 pool에서 single connection 잡고 `BEGIN; <wrapped>; ROLLBACK;` 순차 실행. 중간 에러 시 ROLLBACK 보장 (`Drop` 가드가 아니라 명시적 statement).

#### `db/explain_runner.rs`

`EXPLAIN (... FORMAT JSON)` 결과는 1 row 1 column TEXT 또는 JSON. sqlx로:

```rust
let row: (serde_json::Value,) = sqlx::query_as("EXPLAIN (FORMAT JSON, ...) ...")
    .fetch_one(...).await?;
```

top-level은 `[ { "Plan": {...}, "Planning Time": ..., "Execution Time": ... } ]`. `[0]`만 사용.

#### `db/pg_stats.rs`

```rust
pub async fn fetch_column_stats(
    pool: &sqlx::PgPool,
    schema: &str,
    table: &str,
    columns: &[String],
) -> TuskResult<HashMap<String, ColumnStats>> { … }

pub struct ColumnStats {
    pub n_distinct: Option<f64>,
    pub null_frac: Option<f64>,
}
```

쿼리: `SELECT attname, n_distinct, null_frac FROM pg_stats WHERE schemaname = $1 AND tablename = $2 AND attname = ANY($3)`.

`pg_stats`가 비어있을 수 있음 (ANALYZE 안 돌린 테이블). 그 경우 `verdict = "maybe"`로 떨어짐.

#### `commands/explain.rs`의 `extract_index_candidates`

plan JSON을 재귀 traverse하며 §5.3 규칙 적용. 각 후보에 대해 `pg_stats.fetch_column_stats` 호출하여 verdict 결정. `Skip`은 반환 리스트에서 빠짐.

#### `commands/history.rs` 확장

새 명령 `record_ai_explain`:

```rust
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordAiExplainPayload {
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

Week 4 `record_ai_generation`과 같은 트랜잭션 패턴:

1. `history_entry`에 source=`ai_explain`, sql_full = `-- EXPLAIN interpretation\n-- planSha=…`, statement_count=0.
2. `ai_explain` 신규 테이블에 full payload.

### 6.3 Migration `004_ai_explain`

```sql
CREATE TABLE ai_explain (
    entry_id TEXT PRIMARY KEY REFERENCES history_entry(id) ON DELETE CASCADE,
    plan_sha TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    summary TEXT NOT NULL,
    raw_plan_json TEXT NOT NULL,
    verified_candidates_json TEXT NOT NULL,    -- JSON array
    llm_recommendations_json TEXT NOT NULL,    -- JSON array (may be "[]" if AI skipped)
    prompt_tokens INTEGER,
    completion_tokens INTEGER,
    duration_ms INTEGER NOT NULL
);
CREATE INDEX idx_ai_explain_plan_sha ON ai_explain(plan_sha);
```

`source` 컬럼 CHECK가 만약 enum-like 제약이 있다면 `'ai_explain'`을 추가. (현재 `state.rs` 스키마는 자유 TEXT라 별도 변경 불필요. 코드에서만 'ai'/'editor'/'ai_explain' 사용.)

## 7. Prompts

`lib/ai/explainPrompts.ts`:

```ts
export const SYSTEM_EXPLAIN_PROMPT = `
You are a Postgres performance reviewer. You receive an EXPLAIN plan plus relation
context and produce two artefacts:

1. A single-paragraph plain-English summary identifying the dominant bottleneck.
   Mention specific node types and durations. Do not narrate the whole tree.
2. A JSON array of index recommendations. ONLY recommend an index if it is likely
   to help the supplied plan. Do not invent statistics. Prefer composites only when
   the plan shows multiple correlated filter columns. Skip recommendations whose
   selectivity is clearly poor (the user has already filtered low-cardinality
   candidates server-side).

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

If you have nothing useful to recommend, output an empty array \`[]\` in the json block.
`;
```

User prompt template:

```
Plan (compact tree):
<rendered plan with nodeType, ms, rows, filter>

Relations involved:
<for each relation: DDL (truncated to ~40 lines) + indexes + brief stats>

Verified candidates (server-side cardinality-filtered):
<JSON array>

Original SQL:
<the user's SQL>
```

토큰 budget 초과 (default 8000) 시 DDL 본문 생략, 인덱스 list와 stats 요약만 남긴다.

## 8. Error handling & edge cases

| 상황                                               | 동작                                                                                                                                     |
| -------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| 사용자가 SQL 비워둠                                | toast "SQL is empty"                                                                                                                     |
| 파서 실패                                          | toast "Cannot classify SQL for EXPLAIN" + 트리 영역에 raw 에러 메시지                                                                    |
| Multi-statement (semicolon)                        | 첫 statement만 EXPLAIN, warning에 "additional statements ignored"                                                                        |
| 사용자 SQL이 이미 `EXPLAIN`                        | mode=`passthrough`, ANALYZE-anyway 비활성, 결과 그대로 plan tree 렌더                                                                    |
| DML/DDL 자동 fallback                              | mode=`dml-plan-only`/`ddl-plan-only`, 노란 배지, "ANALYZE anyway" 버튼 노출                                                              |
| ANALYZE-anyway 클릭 + destructive cancel           | 아무 일 없음 (toast "Cancelled" 정도)                                                                                                    |
| ANALYZE-anyway 중 ROLLBACK 도중 에러               | toast(error) + plan 결과는 폐기. tx_slot 활성 중에는 자동 래핑 안 함이라 이 케이스 없음.                                                 |
| `pg_stats` 결과 비어 있음 (ANALYZE 안 돌린 테이블) | 후보의 verdict는 `maybe`, n_distinct/null_frac null. UI에서 "stats unavailable" hint.                                                    |
| 네트워크 LLM 실패                                  | streamGeneration이 throw → toast. AI strip은 "Failed — retry"로 복구. 캐시에 저장 안 함.                                                 |
| LLM JSON 파싱 실패                                 | summary는 그대로 표시, recommendations는 `[]`로 처리 + warning toast.                                                                    |
| 같은 plan 재실행                                   | plan SHA 동일 → AI 캐시 hit 재사용 (재호출 없음)                                                                                         |
| `pg_stats` 권한 없음                               | catch + verdict `maybe` + warnings에 "pg_stats inaccessible (insufficient privileges)" 추가                                              |
| Tab 전환                                           | resultMode 보존. 다른 탭으로 갔다 와도 lastPlan 유지.                                                                                    |
| Tab close                                          | lastPlan/캐시 휘발 (의도)                                                                                                                |
| 탭 sql이 plan 실행 후 변경됨                       | `[Rows                                                                                                                                   | Plan]` 토글은 그대로 (이전 plan은 stale일 수 있음 → 헤더에 "stale" 배지 노출) |
| Plan JSON이 매우 큼 (수백 노드)                    | 트리는 virtualized — 200 노드 미만이면 그대로 렌더, 이상이면 lazy expand 권장 (v1엔 단순 렌더 + 깊이 제한 100, 초과 시 자르기 + warning) |

## 9. Testing strategy

### Frontend (Vitest)

- `planParse.spec.ts`:
  - 단순 Seq Scan plan → 노드 1개 + selfMs 정확.
  - Hash Join 트리 + child Seq Scan → selfMs 보정 (parent total - sum(child total)) 정확.
  - plan-only(actualTotalTime 없음) → selfMs/selfTimeRatio null.
- `planSha.spec.ts`: 같은 JSON 두 번 hash 같음, 키 순서 다른 동등 JSON도 안정 hash (stable stringify).
- `IndexCandidates.spec.tsx`: verdict별 색/문구, "Insert into editor" 콜백.
- `ExplainView.spec.tsx`: Rows/Plan 토글, plan 없을 때 disabled.
- `PlanTree.spec.tsx`: 키보드 ↑↓로 selectedNodePath 변경, ⚠ 노드 표식.
- `explainPrompts.spec.ts`: token budget 초과 시 DDL truncation 동작.

### Backend (Rust unit + integration)

- `sqlast::classify_for_explain`: 모든 카테고리에 대해 fixture sql.
- `pg_stats::fetch_column_stats`: live PG 사용 (testcontainers — Week 2/4 패턴 따름). ANALYZE된 테이블 vs 안 한 테이블.
- `extract_index_candidates`: plan JSON fixture 입력 → 카디널리티 임계값별 Likely/Maybe/Skip 분류 검증.
- `explain_runner`:
  - SELECT plan-analyze 정상 실행.
  - DML(`INSERT`)에 대해 `allow_analyze_anyway=false` → plan-only.
  - DML에 대해 `allow_analyze_anyway=true` + tx_slot 비활성 → BEGIN/ROLLBACK 래핑 후 row count 0 (rolled back) 검증.
- `record_ai_explain`: 트랜잭션으로 history_entry+ai_explain 양쪽 기록, FK on delete cascade 동작.

### Manual verification (체크리스트는 별도 `manual-verification-week-5.md`)

1. SELECT 쿼리 → Cmd+Shift+E → 트리/상세/AI 흐름 전체.
2. UPDATE without WHERE → plan-only fallback 배지 확인 → "ANALYZE anyway" → DestructiveModal 통과 → 결과 받고 데이터 변경 없음 검증 (ROLLBACK 됨).
3. Active 트랜잭션 안에서 UPDATE EXPLAIN ANALYZE anyway → 자동 ROLLBACK 안 함 + 사용자가 직접 commit/rollback.
4. AI 키 없을 때 → AI strip은 "Set BYOK in Settings", 인덱스 추천(Rust 후보)은 그대로 표시.
5. auto-interpret ON → plan 결과 도착 즉시 LLM 호출.
6. 같은 plan 재실행 → 캐시 hit (LLM 재호출 없음, history에도 추가 entry 없음).
7. `pg_stats`가 권한 거부 → toast warning + `maybe` verdict로 fallback.

## 10. Implementation order (writing-plans에서 step별로 분해)

1. Rust: `sqlast::classify_for_explain` + `db/explain_runner.rs` + `commands/explain.rs` (run_explain, candidates 없이도 동작).
2. Rust: `db/pg_stats.rs` + `extract_index_candidates`. EXPLAIN 결과에 verifiedCandidates 채움.
3. Rust: `state.rs` migration 004_ai_explain + `record_ai_explain` 명령.
4. Frontend: `lib/explain/planTypes.ts` + `planParse.ts` + `planSha.ts` + tests.
5. Frontend: `tabs.ts`/`settings.ts` 확장 (PlanState, autoInterpretPlan, indexAdviceEnabled).
6. Frontend: `features/explain/ExplainView.tsx` + `PlanTree.tsx` + `PlanNodeDetail.tsx` + tests. EditorPane에 Cmd+Shift+E + Explain 버튼. ResultsHeader Rows/Plan 토글.
7. Frontend: `IndexCandidates.tsx` + 에디터 inject 동작.
8. Frontend: `lib/ai/explainPrompts.ts` + `lib/ai/explainStream.ts` + `PlanAiStrip.tsx` + 캐시.
9. Frontend → Rust: `record_ai_explain` 호출 wiring.
10. ANALYZE-anyway 흐름 (runGate + BEGIN/ROLLBACK) — 가장 위험하므로 마지막. 통합 테스트 + 수동 검증.
11. 폴리싱: virtualized 트리(노드 수 임계 시), 빈 상태, 로딩 스켈레톤, 키보드 단축키 도움말.
12. `manual-verification-week-5.md` 작성 + cargo fmt / prettier / clippy / test 모두 green.

## 11. Risks & mitigations

| 위험                                                    | 완화                                                                                                   |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| ANALYZE가 실제 데이터를 변경                            | 자동 fallback + 명시적 ANALYZE-anyway에만 destructive 가드 + BEGIN/ROLLBACK 래핑                       |
| LLM이 hallucinated 인덱스 추천                          | Rust verified candidates와 머지 시 LLM 추천 중 후보에 없는 컬럼은 시각적으로 "AI suggestion only" 표시 |
| 큰 plan JSON으로 토큰 폭발                              | 토큰 budget + DDL truncation + 등장 relation만 첨부                                                    |
| `pg_stats` 권한 부족 환경                               | verdict `maybe`로 graceful, 사용자에게 명시적 warning                                                  |
| Plan 재현성(같은 SQL 다른 시점) 비교 욕구               | v1.5로 미룸 — `ai_explain` 테이블에 raw_plan_json은 이미 저장되니 후속 기능에 데이터 자산은 남음       |
| 무한 재귀/순환 plan (불가능하지만 방어)                 | parse 시 깊이 100 초과 시 cut + warning                                                                |
| Multi-statement EXPLAIN 시도                            | 첫 statement만 + warning. v1.5에서 제대로.                                                             |
| destructive 가드 + transactioning 상호작용 (Week 3/4와) | 기존 `runGate`/`tx_slot` API를 그대로 사용. 신규 가드 만들지 않음.                                     |

## 12. Acceptance gate (Week 5 done = 모두 true)

- [ ] §1의 10개 success criteria 전부 manually verified
- [ ] cargo test / vitest / cargo clippy / prettier / cargo fmt 전부 green
- [ ] `docs/superpowers/plans/manual-verification-week-5.md` 작성 + 실행 결과 기록
- [ ] 신규 의존성 없음 (`package.json`/`Cargo.toml` diff 검토)
- [ ] PLAN.md Week 5 4개 sub-bullet 모두 충족

---

## 부록 A — JSON 구조 참조

`EXPLAIN (FORMAT JSON)` 출력 (Postgres 16 기준 핵심 필드만):

```json
[
  {
    "Plan": {
      "Node Type": "Hash Join",
      "Parallel Aware": false,
      "Join Type": "Inner",
      "Startup Cost": 0.00, "Total Cost": 1234.56,
      "Plan Rows": 1000, "Plan Width": 64,
      "Actual Startup Time": 0.123, "Actual Total Time": 12.345,
      "Actual Rows": 998, "Actual Loops": 1,
      "Hash Cond": "(a.id = b.a_id)",
      "Plans": [
        { "Node Type": "Seq Scan", "Relation Name": "users", "Schema": "public",
          "Filter": "(email ~~ '%@example.com'::text)",
          "Rows Removed by Filter": 49002, … },
        …
      ],
      "Buffers": { "Shared Hit Blocks": 410, "Shared Read Blocks": 88 }
    },
    "Planning Time": 0.456,
    "Execution Time": 12.789
  }
]
```

`Buffers`는 `BUFFERS` 옵션 켰을 때만. `Actual *` 필드는 `ANALYZE` 켰을 때만.

## 부록 B — 위협 모델 메모

EXPLAIN 자체는 SQL injection 면역인 것처럼 보이지만, `BEGIN/ROLLBACK` 래핑 시 wrapped SQL을 문자열 결합으로 만들기 때문에 **사용자 SQL을 quote/escape하지 않음** — 사용자 자신의 SQL이라 의미상 안전하지만, 코드 리뷰 시 명확히 표시 필요. `sqlx::query(&format!(...))` 패턴은 Week 5에서도 그대로 (프리페어드 statement는 EXPLAIN 자체에 적용 어려움).
