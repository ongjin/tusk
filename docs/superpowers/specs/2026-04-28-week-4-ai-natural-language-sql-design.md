# Week 4 — AI 1차: BYOK + 자연어→SQL + 스키마 RAG + Destructive Confirmation (design spec)

> **Date:** 2026-04-28
> **Scope:** PLAN.md Week 4 전부 — BYOK 설정 / Cmd+K 자연어→SQL / 스키마 RAG / destructive 쿼리 confirmation 모달
> **Status:** Drafted, awaiting user review

---

## 1. Goal & success criteria

Tusk를 "AI 1급 시민"이라는 차별점이 처음으로 표면에 드러나는 상태로 만든다. 사용자가 자연어를 입력하면 현재 연결된 DB의 실제 스키마를 LLM이 알고 있는 채로 SQL을 생성하고, destructive 변경은 항상 명시적 확인을 거친다.

End user 기준 성공 조건:

1. 첫 실행 시 Settings → Providers에서 OpenAI / Anthropic / Gemini / Ollama 중 하나(또는 여러) 키 입력 → OS keychain 저장 → 재시작 후 그대로 동작.
2. 모델 선택 (generation / embedding 분리). 각 provider별 합리적 default 제공. 모든 provider에서 NL→SQL 흐름 1회 이상 검증됨.
3. 에디터에서 `Cmd+K` → 입력창에 "find users that signed up last week" 같은 자연어 → 스트리밍으로 SQL 생성 → diff view → Apply 시 에디터 갱신, Reject 시 원복.
4. 스키마 컨텍스트가 자동 적용됨. 연결 시점에 스키마 임베딩 인덱스가 백그라운드에서 빌드되고, NL→SQL 호출마다 관련 테이블 top-K(default 8)가 system prompt에 DDL로 첨부된다. 사용자 입력에 등장한 테이블 이름은 무조건 포함.
5. AI가 만든 SQL에 destructive statement(`DROP/TRUNCATE/DELETE without WHERE/UPDATE without WHERE/ALTER ... DROP COLUMN/REVOKE ALL` 등)가 있으면 Apply 또는 Run 시점에 **DestructiveModal** — 영향 받는 객체 + statement preview + 이유 + ("Cancel" / "Run anyway") 또는 strict 모드에선 키워드 타이핑 confirm.
6. 사용자가 직접 친 SQL의 Cmd+Enter 실행도 동일한 destructive 가드를 통과한다 (AI 경로에만 있는 게 아니라 전체 실행 경로의 게이트).
7. AI는 도구 호출이 가능: `get_table_schema`, `list_indexes`. 즉, top-K RAG로 부족할 때 LLM이 스스로 추가 테이블 DDL을 가져올 수 있다. `sample_rows`는 Settings에서 명시적으로 켰을 때만 등장 (개인정보 노출 우려).
8. 모든 AI 호출은 history에 entry로 남는다. source = `ai`. 프롬프트 + 최종 SQL + provider/model + 토큰/비용(가능한 경우) 기록.
9. 임베딩 인덱스는 스키마 변경(테이블 add/drop/alter) 감지 시 자동 갱신. 수동 "Rebuild schema index" 버튼 제공.
10. 키 누락 시 Cmd+K는 Settings로 점프시키는 안내 모달. 네트워크/모델 오류는 toast + retry 유도. 절대 silent fail 금지.

이 10개가 만족되면 Week 4 완료.

## 2. Out of scope (Week 5+)

- EXPLAIN 시각화 / AI 해석 — Week 5
- pgvector 1급 시민 (vector 컬럼 시각화, 유사 row 찾기) — Week 6
- **쿼리 히스토리 의미 검색** — Week 7. Week 4에서는 same-connection 최근 5개의 성공 쿼리를 chronological few-shot으로 첨부 (간이판).
- Slow query 자동 분석 / pg_stat_statements 패널 — v1.5
- 대화형 multi-turn AI 채팅 사이드바 (single-shot Cmd+K만 지원)
- AI가 직접 statement 실행 (tool로 EXPLAIN 호출 등) — v1.5. 항상 사용자가 Apply 후 직접 Run.
- 토큰/비용 텔레메트리 대시보드 (history에 raw 값만 기록, 시각화는 안 함)
- **임베딩 BLOB 압축 / sqlite-vec extension** — v1엔 raw `f32[]` BLOB + Rust 인메모리 cosine. 100~1000 테이블 범위에서 충분.
- 스키마 graph 시각화 / FK 다이어그램
- 네트워크 LLM 호출 cancel (Cmd+K 모달의 Cancel은 UI 상태만 정리, 진행 중인 fetch는 AbortController로 끊는다 — 그 이상의 백엔드 cancel은 안 함)

## 3. Architecture

```
┌── Frontend (React + zustand + Vercel AI SDK) ───────────────┐
│  features/                                                   │
│    ai/                ← NEW                                  │
│      CmdKPalette.tsx        floating prompt + streaming UI  │
│      SqlDiffView.tsx        Monaco diff editor              │
│      DestructiveModal.tsx   confirmation gate               │
│      AiHistoryEntry.tsx     history list 'ai' source 렌더    │
│    settings/          ← NEW                                  │
│      SettingsDialog.tsx     provider keys + model picker    │
│      ProviderSection.tsx    per-provider form              │
│      ModelPicker.tsx        generation/embedding 분리       │
│    editor/, results/, transactions/, ... (기존 유지)          │
│  store/                                                      │
│    ai.ts             ← NEW   provider/model/last prompt      │
│    schemaIndex.ts    ← NEW   build progress mirror           │
│    settings.ts       (확장)  enabledProviders, embedProvider  │
│  lib/                                                        │
│    ai/                                                       │
│      providers.ts    AI SDK model factory (4 providers)     │
│      prompts.ts      system prompt + schema injection       │
│      tools.ts        AI SDK tool defs (Tauri invoke)        │
│      destructive.ts  AST-based detector (Rust mirror)       │
│      stream.ts       text→SQL streaming + parse             │
│    keychain.ts       ← NEW invoke wrapper for ai_secrets    │
└──────────────────────────────────────────────────────────────┘
                          ↕ Tauri invoke()
┌── Rust (src-tauri) ─────────────────────────────────────────┐
│  commands/                                                   │
│    ai_secrets.rs     NEW   set/get/delete provider keys     │
│    schema_index.rs   NEW   sync_schema_index / top_k         │
│    ai_tools.rs       NEW   get_table_schema / list_indexes  │
│                            sample_rows (gated)              │
│    destructive.rs    NEW   classify_destructive (AST)       │
│    history.rs        (확장) source='ai' + ai_meta            │
│    query.rs          (확장) execute_query에 destructive 가드 │
│  db/                                                         │
│    schema_embed.rs   NEW   build_table_ddl + checksum       │
│    embedding_store.rs NEW  rusqlite BLOB store + cosine     │
│    state.rs          (migration 003_ai)                      │
│  secrets.rs          (확장) provider별 keychain entry        │
│  errors.rs           (확장) Ai/SchemaIndex/Destructive       │
└──────────────────────────────────────────────────────────────┘
```

LLM generation 호출은 **프론트에서 직접** (Vercel AI SDK 스트리밍 활용). 임베딩 호출은 **Rust 백그라운드 작업**에서 (네트워크 fetch는 Rust `reqwest` 새로 추가). 이 분할은 PLAN.md 3장의 결정 그대로.

### 왜 generation은 프론트, embedding은 Rust?

- Generation은 사용자가 보고 있는 stream UI — Vercel AI SDK의 `streamText` UX가 압도적으로 강함. Rust → IPC chunk 흘리기보다 직결이 안전.
- Embedding은 batch + 백그라운드 + 인덱스 BLOB 직접 쓰기 — Rust에서 네트워크 fetch + sqlite write를 한 트랜잭션 안에서 처리하는 게 깔끔하고, UI 멈춤 없음.
- 동일 API key를 양쪽에서 읽어야 함: 프론트는 매 호출마다 `invoke('get_ai_secret', { provider })`로 가져와 SDK에 주입. 키는 메모리에 캐시 안 함 (앱 끄면 휘발).

## 4. Libraries (Week 4 추가)

| 영역                   | 라이브러리                                          | 사유                                                                                                      |
| ---------------------- | --------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| AI SDK (frontend)      | `ai` 6.x + `@ai-sdk/openai` + `@ai-sdk/anthropic`   | 이미 설치. streamText, generateText, tool calling 통합 인터페이스.                                        |
| Gemini provider        | `@ai-sdk/google` (Vercel AI SDK)                    | NEW dep. AI SDK 6와 호환되는 공식 패키지.                                                                 |
| Ollama provider        | `ollama-ai-provider-v2` (또는 동등 AI SDK 어댑터)   | NEW dep. AI SDK 6 호환. localhost endpoint 설정.                                                          |
| Monaco diff            | `@monaco-editor/react` 기존 + `DiffEditor` import   | 추가 dep 없음. 동일 패키지에 Diff 컴포넌트 포함.                                                          |
| Rust HTTP (embeddings) | `reqwest` 0.12 (rustls)                             | NEW dep. OpenAI/Gemini/Ollama embeddings REST. Anthropic은 임베딩 모델 없음 → fallback 안내.              |
| Rust JSON streams      | `tokio::sync::mpsc` 기존 + `tauri::Emitter` 기존    | 임베딩 진행률 emit. 추가 dep 없음.                                                                        |
| Cosine similarity      | 직접 작성 (`f32` SIMD optional)                     | 100~1000 벡터 × 1536 dim = 1.5M f32 곱 → 단일 코어 < 5ms. SIMD 불필요.                                    |
| Hash for DDL checksum  | stdlib `std::hash::DefaultHasher`                    | 추가 dep 없음. checksum은 동등성만 보장하면 충분 — 충돌 저항성 불필요. 결과는 hex 문자열로 직렬화. |

## 5. Data model

### 5.1 Frontend types (`src/lib/types.ts` 확장)

```ts
export type AiProvider = "openai" | "anthropic" | "gemini" | "ollama";

export interface ProviderConfig {
  provider: AiProvider;
  apiKeyPresent: boolean;        // never the key itself — UI flag only
  baseUrl?: string;              // ollama: "http://localhost:11434", custom OAI-compat URL
  generationModel: string;       // "gpt-4o-mini" | "claude-haiku-4-5" | "gemini-2.5-flash" | "llama3.1:8b" | ...
  embeddingModel?: string;       // "text-embedding-3-small" | "text-embedding-004" | "nomic-embed-text" | null
}

export interface AiSettings {
  enabledProviders: AiProvider[];
  defaultGenerationProvider: AiProvider;
  defaultEmbeddingProvider: AiProvider;     // anthropic 단독은 불가 — UI에서 막기
  toolsEnabled: { sampleRows: boolean };    // 다른 도구는 항상 ON
  destructiveStrict: boolean;               // true면 키워드 타이핑 confirm 강제
  ragTopK: number;                          // default 8
  schemaIndexAutoSync: boolean;             // default true
}

export type DestructiveKind =
  | "drop-database"
  | "drop-schema"
  | "drop-table"
  | "drop-column"
  | "drop-index"
  | "drop-view"
  | "drop-function"
  | "truncate"
  | "delete-no-where"
  | "update-no-where"
  | "alter-drop-constraint"
  | "grant-revoke-all"
  | "vacuum-full";

export interface DestructiveFinding {
  kind: DestructiveKind;
  statementIndex: number;
  message: string;        // "DELETE without WHERE will remove all rows from public.users"
  affectedObject?: string; // "public.users"
}

export interface AiHistoryMeta {
  source: "ai";
  provider: AiProvider;
  generationModel: string;
  embeddingModel?: string;
  prompt: string;            // user's NL input
  generatedSql: string;
  topKTables: string[];      // schema-qualified
  toolCalls: { name: string; args: unknown }[];
  promptTokens?: number;
  completionTokens?: number;
}

export interface SchemaIndexProgress {
  connId: string;
  state: "idle" | "running" | "done" | "error";
  totalTables: number;
  embeddedTables: number;
  errorMessage?: string;
  lastSyncedAt?: number;
}
```

### 5.2 Rust types

```rust
// commands/ai_secrets.rs
pub fn ai_secret_set(provider: &str, value: &str) -> TuskResult<()>;
pub fn ai_secret_get(provider: &str) -> TuskResult<Option<String>>;
pub fn ai_secret_delete(provider: &str) -> TuskResult<()>;
pub fn ai_secret_list_present() -> TuskResult<Vec<String>>;  // names only

// commands/schema_index.rs
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaTopK {
    pub tables: Vec<TopKTable>,
    pub coverage: TopKCoverage,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopKTable {
    pub schema: String,
    pub table: String,
    pub ddl: String,
    pub similarity: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopKCoverage {
    pub total_tables: usize,
    pub returned: usize,
    pub forced_includes: Vec<String>,  // tables matched by name in user prompt
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub embedded: usize,
    pub skipped_unchanged: usize,
    pub failed: Vec<String>,
}

// commands/destructive.rs
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DestructiveFinding {
    pub kind: String,                 // wire = kebab-case (DestructiveKind frontend과 동기)
    pub statement_index: usize,
    pub message: String,
    pub affected_object: Option<String>,
}

// commands/ai_tools.rs (LLM tool 호출용 — 프론트가 AI SDK tool result로 invoke)
pub async fn get_table_schema(connection_id: &str, schema: &str, table: &str)
    -> TuskResult<String>; // returns DDL

pub async fn list_indexes(connection_id: &str, schema: &str, table: &str)
    -> TuskResult<Vec<IndexRow>>;

pub async fn sample_rows(connection_id: &str, schema: &str, table: &str, limit: u32)
    -> TuskResult<QueryResult>; // gated by AiSettings.toolsEnabled.sampleRows
```

### 5.3 SQLite migration `003_ai.sql`

```sql
-- 한 connection 안의 테이블별 DDL 임베딩.
-- (conn_id, schema, table) UNIQUE. 임베딩은 raw little-endian f32 BLOB.
CREATE TABLE schema_embedding (
    id              TEXT PRIMARY KEY,
    conn_id         TEXT NOT NULL,
    schema          TEXT NOT NULL,
    table_name      TEXT NOT NULL,
    pg_relid        INTEGER NOT NULL,        -- oid; relid 변경 = 재embed
    ddl_checksum    TEXT NOT NULL,           -- hex of stable hash of (DDL + comments)
    embedding       BLOB NOT NULL,           -- f32[] little-endian
    embedding_dim   INTEGER NOT NULL,
    embedding_model TEXT NOT NULL,           -- "text-embedding-3-small" 등
    embedded_at     INTEGER NOT NULL,
    UNIQUE (conn_id, schema, table_name)
);
CREATE INDEX idx_schema_embedding_conn ON schema_embedding(conn_id);

-- AI 호출 메타. history_entry와 1:1 (entry_id FK).
CREATE TABLE ai_history (
    entry_id          TEXT PRIMARY KEY REFERENCES history_entry(id) ON DELETE CASCADE,
    provider          TEXT NOT NULL,
    generation_model  TEXT NOT NULL,
    embedding_model   TEXT,
    prompt            TEXT NOT NULL,
    generated_sql     TEXT NOT NULL,
    top_k_tables      TEXT NOT NULL,          -- JSON array of "schema.table"
    tool_calls        TEXT,                    -- JSON array
    prompt_tokens     INTEGER,
    completion_tokens INTEGER
);
```

`history_entry.source` enum에 `'ai'` 추가 (DB 레벨 CHECK 없음, 프론트/Rust enum만 확장).

### 5.4 OS keychain layout (provider 키)

| Service | Account                | 값                       |
| ------- | ---------------------- | ------------------------ |
| `tusk`  | `ai:openai`            | OpenAI API key           |
| `tusk`  | `ai:anthropic`         | Anthropic API key        |
| `tusk`  | `ai:gemini`            | Google AI Studio key     |
| `tusk`  | `ai:ollama`            | (선택) Ollama auth header — 보통 비어있음. baseUrl만 settings에. |

기존 connection 비밀번호 entry (`conn:<id>`)와 namespace 충돌 없음. `secrets.rs`에 `ai_entry(provider)` helper 추가.

## 6. 핵심 흐름

### 6.1 Provider 셋업

```
1. 사용자: Settings → Providers → OpenAI 카드
2. API key paste → Save
3. frontend: invoke('ai_secret_set', { provider: 'openai', value })
4. Rust: keyring 저장
5. frontend: store/ai.ts에 apiKeyPresent=true 갱신
6. ProviderSection이 모델 picker 렌더 (default: gpt-4o-mini)
7. 첫 키 저장 직후 Test 버튼 노출 → /v1/models 또는 짧은 generateText 1회 → 통과 시 토스트, 실패 시 키 유지하되 UI에 경고 표시
```

키 자체는 절대 frontend memory에 영구 저장 안 함. 매 AI 호출 직전에 `invoke('ai_secret_get')` → SDK에 inject → 응답 후 변수 drop.

### 6.2 스키마 임베딩 인덱스 빌드

```
Trigger:
  - 'connect' 성공 직후 (기본 — schemaIndexAutoSync=true)
  - 사용자: Settings → Schema index → Rebuild
  - 사용자가 만든 DDL 실행 후 (heuristic: history_statement 마지막이 CREATE/ALTER/DROP TABLE이면 mark dirty)

Rust 'sync_schema_index(connection_id)':
  1. embedding provider 결정: AiSettings.defaultEmbeddingProvider
     - 없거나 키 없음 → SyncReport{ failed: ['no embedding provider'] } 즉시 반환
  2. PG에서 모든 테이블 enumerate:
       SELECT n.nspname, c.relname, c.oid
       FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
       WHERE c.relkind IN ('r','p','m')           -- table, partitioned, matview
         AND n.nspname NOT IN ('pg_catalog','information_schema','pg_toast')
  3. 각 테이블에 대해:
     a. build_table_ddl(pool, schema, table) → CREATE TABLE 합성 + COMMENT ON 첨부 + 컬럼 타입 + PK/FK 표시
        (information_schema + pg_catalog 조합. pg_dump 호출은 안 함 — 환경 의존성)
     b. checksum = hash(ddl_string)
     c. existing = SELECT FROM schema_embedding WHERE (conn_id, schema, table) = ?
        - existing.pg_relid == c.oid AND existing.ddl_checksum == checksum
          AND existing.embedding_model == 현재 model
          → SKIP, skipped_unchanged++
        - 그 외 → embed(ddl_string) via reqwest → BLOB upsert
     d. 매 N개마다 emit('schema_index:progress', { connId, embedded, total })
  4. 마지막에 emit('schema_index:done', { connId, embedded, skipped, failed })

errors:
  - 네트워크 실패 한 테이블 → 그 테이블만 failed에 push, 나머지 계속
  - rate limit → exponential backoff 3회까지, 그래도 실패하면 abort + 사용자 retry 유도

privacy 주의:
  - DDL은 컬럼 이름 + 타입 + 코멘트만 포함. 데이터 row는 절대 포함 안 함.
  - 사용자가 원하면 settings에서 토글로 schemaIndexAutoSync=false (수동만).
```

### 6.3 Cmd+K → 자연어 → SQL

```
1. 사용자 에디터에서 텍스트 선택 (옵션) + Cmd+K
2. CmdKPalette가 cursor 근처 floating input 표시
   - 선택 텍스트 있음 → "Edit selected SQL" 모드 (system prompt에 selection을 base로)
   - 선택 없음 → "Generate SQL from scratch"
3. 사용자 자연어 입력 + Enter
4. frontend:
   a. invoke('schema_top_k', {
        connectionId, query: prompt, topK: ragTopK, forcedTables: extractTableNames(prompt)
      })
      → SchemaTopK { tables, coverage }
   b. invoke('list_recent_history', { connectionId, limit: 5, status: 'ok' })
      → 최근 성공 SQL 5건 (few-shot — Week 7 의미 검색은 차차)
   c. invoke('ai_secret_get', { provider }) → key
   d. assemble system prompt (lib/ai/prompts.ts):
        - PG 버전 + 활성 확장 (이미 있는 'pg_extension' 1회 fetch 캐시)
        - top-K DDL 블록
        - 최근 쿼리 few-shot
        - safety 가이드: destructive는 항상 명시적 + 사용자가 직접 검토 + JOIN 키 명시 등
   e. AI SDK streamText({ model, system, prompt, tools, maxSteps: 3 })
   f. 스트리밍 chunk를 SqlDiffView에 흘림 (right side)
5. 스트림 완료:
   a. lib/ai/destructive.ts.classify_local(sql)으로 1차 빠른 검출 (정규식)
      → 소수 false-positive 가능하지만 사용자에게 미리 경고 띄우기 위함
   b. 실제 게이트는 Apply / Run 시점 Rust classify_destructive (AST)
   c. SqlDiffView가 [Apply] [Reject] [Re-prompt] 표시
6. Apply:
   - selection 모드면 selection 영역만 replace, 아니면 cursor 위치에 insert (선택)
   - history_entry 'ai' source 기록 (Apply 후가 아니라 generation 직후 + Apply 액션 별도 entry)
   - 사용자가 그 다음 Cmd+Enter → 6.5 destructive 게이트로 들어감
7. Reject: 닫고 끝. history_entry는 'ai' source로 status='ok'로 남음 (generation 자체는 성공). ai_history.generated_sql만 보면 Apply 됐는지는 알 수 없으므로 v1엔 구분하지 않음 — Risk #6 참조.
8. Re-prompt: 같은 모달에서 prompt 재입력, top-K 재평가.
```

### 6.4 AI tool calling

```
AI SDK의 tools 매개변수:

const tools = {
  get_table_schema: tool({
    description: "Return the CREATE TABLE DDL for a specific schema-qualified table.",
    parameters: z.object({ schema: z.string(), table: z.string() }),
    execute: async ({ schema, table }) =>
      invoke('get_table_schema', { connectionId, schema, table })
  }),
  list_indexes: tool({
    description: "List indexes (name, columns, type) for a table.",
    parameters: z.object({ schema: z.string(), table: z.string() }),
    execute: ({ schema, table }) =>
      invoke('list_indexes', { connectionId, schema, table })
  }),
  // sample_rows은 settings.toolsEnabled.sampleRows=true일 때만 spread
  ...(toolsEnabled.sampleRows ? {
    sample_rows: tool({
      description: "Sample up to N rows from a table for context. Off by default for privacy.",
      parameters: z.object({
        schema: z.string(), table: z.string(), limit: z.number().int().min(1).max(20)
      }),
      execute: ({ schema, table, limit }) =>
        invoke('sample_rows', { connectionId, schema, table, limit })
    })
  } : {})
};

streamText({ ..., tools, maxSteps: 3 });
```

`maxSteps: 3` → 토큰 폭주 방어. 도구 결과는 항상 `<tool_result>...</tool_result>` 식으로 포맷되어 다음 step 에 들어감.

### 6.5 Destructive 가드 (Apply / Run 진입점)

```
모든 statement 실행 경로(Cmd+K Apply, 에디터 Cmd+Enter, palette 재실행)에서:

1. invoke('classify_destructive', { sql }) → DestructiveFinding[]
   - Rust sqlparser AST 순회. 단일 + 멀티 statement 둘 다.
   - statement 단위 결과. 한 multi-statement 안에 destructive가 1개라도 있으면 결과 길이 ≥ 1.
2. findings.length === 0 → 그대로 실행 흐름.
3. findings.length > 0 → DestructiveModal:
     - 위쪽: ⚠️ 아이콘 + "This statement contains destructive operations"
     - 각 finding을 요약 row로:
         "DELETE FROM public.users — DELETE without WHERE will remove all rows"
     - SQL preview (전체 또는 첫 destructive statement)
     - destructiveStrict=false: [Cancel] [Run anyway]
     - destructiveStrict=true: 키워드 타이핑 input — 첫 destructive statement의 첫 단어 (예: "TRUNCATE", "DROP TABLE") — 정확히 일치해야 [Run] 활성화
     - 모달이 항상 직전 입력 SQL을 그대로 표시 (사용자가 본 것과 실제 실행 SQL 일치 보증)
4. Confirm → 그대로 execute_query 진입.
   Cancel → 토스트 "Cancelled" + 에디터로 포커스.

검출 규칙 (sqlparser AST 기반):
  Statement::Drop { object_type: Database/Schema/Table/Index/View/Function, names } → drop-*
  Statement::Truncate                                                                → truncate
  Statement::Delete  { selection: None }                                              → delete-no-where
    (selection.is_none()는 WHERE 부재. USING절도 검증)
  Statement::Update  { selection: None }                                              → update-no-where
  Statement::AlterTable { operations } 안에:
    AlterTableOperation::DropColumn                                                    → drop-column
    AlterTableOperation::DropConstraint                                                → alter-drop-constraint
  Statement::Grant{..} OR Statement::Revoke{..} with all_privileges → grant-revoke-all
  Statement::Vacuum { full: true }                                                    → vacuum-full

파서 실패 시 (sqlparser가 못 파싱) → Findings는 비어있음으로 처리하지 않고 special "parser-failed" finding 1개 반환 + 모달 표시 (사용자가 손으로 OK 해야 진행). 보수적.
```

### 6.6 AI 호출 history 기록

```
Generation 직후 (Apply 안 했어도):
  history_entry: source='ai', sql_preview=generated_sql 첫 200자, sql_full=generated_sql
                 status='ok' (generation은 성공한 셈), duration_ms=streaming 총 시간
  ai_history: provider, generation_model, embedding_model, prompt, generated_sql,
              top_k_tables, tool_calls, prompt_tokens, completion_tokens

Apply 후 사용자가 Run하면 그건 별개 history_entry (source='editor', sql_full=...).
'ai' entry와 그 후속 'editor' entry 사이의 연결은 v1엔 안 함 (단순화).

History 패널 / Cmd+P 팔레트:
  source='ai'은 작은 ✨ 아이콘 + "AI: <prompt 첫 80자>"로 렌더.
  클릭 → 새 에디터 탭에 generated_sql 로드.
```

## 7. UI

### 7.1 SettingsDialog

```
┌─ Settings ─────────────────────────────────────────────────────┐
│  [General]  [Providers]  [Schema Index]  [Advanced]            │
├────────────────────────────────────────────────────────────────┤
│ Providers                                                       │
│                                                                │
│ ▣ OpenAI                                          [Test] [Save] │
│   API key  ********************                                 │
│   Generation model  [gpt-4o-mini ▼]                             │
│   Embedding model   [text-embedding-3-small ▼]                  │
│                                                                │
│ ▣ Anthropic                                                     │
│   API key  ********************                                 │
│   Generation model  [claude-haiku-4-5 ▼]                        │
│   Embedding model   — not provided —                            │
│   ⚠ Anthropic does not provide embeddings. Pick another        │
│     provider below for embedding model.                         │
│                                                                │
│ ☐ Gemini       (no key set)         [Add key]                   │
│ ☐ Ollama       baseUrl http://localhost:11434                   │
│   Generation model  [llama3.1:8b ▼]    [Refresh model list]     │
│   Embedding model   [nomic-embed-text ▼]                        │
│                                                                │
│ Default generation provider  [OpenAI ▼]                          │
│ Default embedding provider   [OpenAI ▼]                          │
│                                                                │
│ Tool calling                                                    │
│   ▣ get_table_schema    (always on)                             │
│   ▣ list_indexes        (always on)                             │
│   ☐ sample_rows         off — sends N rows to the LLM           │
│                                                                │
│ Destructive query confirmation                                  │
│   ◉ Standard (just confirm)                                     │
│   ○ Strict (type the keyword to confirm)                        │
└────────────────────────────────────────────────────────────────┘
```

각 provider 카드 expand. 키 입력은 마스킹. "Show key" 버튼은 의도적으로 안 만듦 (실수 유출 방지).

### 7.2 CmdKPalette

```
(에디터 위에 floating, cursor 근처)
┌─ Cmd+K ────────────────────────────────────────────────┐
│ ✦ users who signed up last week and paid by card_     │
│                                          [⏎ Generate]   │
│ Provider: OpenAI · Model: gpt-4o-mini · Top-K: 8        │
└────────────────────────────────────────────────────────┘
```

Enter 누르면 같은 위치에서 stream:

```
┌─ Cmd+K ────────────────────────────────────────────────┐
│ ✦ users who signed up last week and paid by card        │
├────────────────────────────────────────────────────────┤
│ SELECT u.id, u.email                                    │
│ FROM public.users u                                     │
│ JOIN public.payments p ON p.user_id = u.id              │
│ WHERE u.created_at >= now() - interval '7 days'         │
│   AND p.method = 'card'_                                │
├────────────────────────────────────────────────────────┤
│ Tools used: get_table_schema(public.payments)           │
│            [Apply] [Reject] [Re-prompt]                  │
└────────────────────────────────────────────────────────┘
```

선택 텍스트가 있으면 SqlDiffView로 전환 (좌: 원본 selection, 우: 새 SQL).

키 누락이면 위 입력창 자리에:

```
✦ AI not configured. Open Settings → Providers to add a key.
                                                [Open Settings]
```

### 7.3 DestructiveModal

```
┌─ Confirm destructive operations ───────────────────────────────┐
│ ⚠ This statement contains operations that may delete or        │
│   modify many rows.                                             │
│                                                                 │
│   • DELETE without WHERE — will remove all rows from            │
│     public.audit_log                                            │
│   • DROP TABLE — public.staging                                 │
│                                                                 │
│   --- SQL preview ---                                           │
│     DELETE FROM public.audit_log;                               │
│     DROP TABLE public.staging;                                  │
│                                                                 │
│  [Standard mode]                                                │
│                                       [Cancel] [Run anyway]     │
│                                                                 │
│  [Strict mode]                                                  │
│   Type 'DELETE' to confirm: ______                              │
│                                       [Cancel] [Run]            │
└─────────────────────────────────────────────────────────────────┘
```

### 7.4 Schema Index 패널 (Settings)

```
Schema index — connection 'prod-replica'
  Tables: 124   Embedded: 124   Last sync: 2 min ago
  Embedding model: text-embedding-3-small (1536 dim)
  ▣ Auto-sync on connect
                                       [Rebuild] [Clear]
  Recent failures: (none)
```

진행 중이면 progress bar + emit 받아 갱신.

## 8. Errors

```rust
#[derive(thiserror::Error, Debug, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum TuskError {
    // ... 기존

    #[error("AI provider error: {0}")]
    Ai(String),

    #[error("AI provider not configured: {0}")]
    AiNotConfigured(String),                // provider name

    #[error("Schema index error: {0}")]
    SchemaIndex(String),

    #[error("Embedding network error: {0}")]
    EmbeddingHttp(String),

    #[error("Destructive guard: parser failed")]
    DestructiveParserFailed,

    #[error("Destructive guard: confirmation required")]
    DestructiveConfirmRequired,             // 백엔드가 자체로 거부할 때만 — UI 가드 우회 방어
}
```

## 9. Decisions taken

이 spec의 큰 결정 사항 (사용자 지시: "PLAN.md Week 4 전부, 줄이지 마라"):

1. **스코프**: PLAN.md Week 4 4개 항목 전부 한 spec. 줄이거나 미루지 않음.
2. **Provider**: OpenAI / Anthropic / Gemini / Ollama 4개 모두 v1 지원. SDK 레벨에서 동일 인터페이스.
3. **Generation은 frontend, embedding은 Rust**. 키는 매 호출마다 keychain에서 fetch, frontend에 영구 저장 안 함.
4. **임베딩 저장**: rusqlite BLOB(`f32[]` little-endian) + 인메모리 cosine. sqlite-vec extension은 v1 미사용 (추가 빌드 마찰 회피, 100~1000 테이블 범위에서 5ms 이내).
5. **Schema sync trigger**: 연결 직후 자동 (default ON) + 수동 Rebuild + DDL statement 후 dirty 마킹. 임베딩 provider는 settings에서 별도 선택. Anthropic 단독은 임베딩 불가능 → UI에서 secondary embedding provider 강제.
6. **Few-shot 히스토리**: same-connection 최근 5개 성공 쿼리 chronological. Week 7의 의미 검색은 별도.
7. **Tool calling 범위**: get_table_schema + list_indexes 항상 ON. sample_rows는 명시적 opt-in (privacy).
8. **Destructive 검출**: Rust sqlparser AST. AI 경로 + 사용자 직접 실행 경로 둘 다 같은 게이트. 파서 실패는 보수적 confirm 강제.
9. **Destructive 모달**: 기본 Standard ([Cancel] [Run anyway]) + Strict (키워드 타이핑) 옵션.
10. **Cmd+K 모드**: selection 있으면 diff 모드 (좌 원본 / 우 신규), 없으면 insert 모드.
11. **Token/cost 기록**: history에 raw 숫자만 적재. 시각화/예산 UI는 Out of scope.
12. **Cancel**: AI 스트림은 AbortController로 끊고 finalize. Rust 임베딩 sync는 connection drop 시 자동 종료. PG cancel 통합은 Week 3에서 이미 처리.
13. **Diff UI**: Monaco DiffEditor 그대로 사용. 추가 dep 없음.

## 10. Risks

| #   | 위험                                              | 영향                          | 완화                                                                                                                                |
| --- | ------------------------------------------------- | ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| 1   | LLM이 잘못된 destructive SQL 생성 (DROP TABLE)    | 사용자 데이터 영구 손실       | Apply / Run 두 경로 모두 Rust AST 게이트. AI prompt에도 "destructive는 명시 동의 필요" 박음. Strict 모드에서 키워드 타이핑.         |
| 2   | sqlparser-rs가 PG 방언 못 파싱 (예: COPY ... PROGRAM) | destructive 게이트 우회       | 파서 실패 = 항상 confirm 강제 (DestructiveParserFailed 전용 모달 메시지). 절대 그냥 통과 X.                                          |
| 3   | 임베딩 호출 비용 폭주 (큰 스키마 매 connect)      | $$ 청구                       | (conn_id, schema, table, ddl_checksum, model) 매칭 시 SKIP. Auto-sync OFF 토글 제공. 매 sync 진행률 emit + Cancel 가능.             |
| 4   | API key가 frontend memory에 남음                  | 프로세스 dump 시 누출         | 매 호출 직전 invoke로 fetch, 변수 즉시 drop. frontend는 apiKeyPresent boolean만 영속화. zustand persist에 키 절대 안 들어감.        |
| 5   | top-K가 잘못된 테이블만 골라 LLM 헛소리           | 잘못된 SQL → 신뢰 손상        | 사용자 prompt의 단순 토큰 매칭으로 schema/table 명 추출 → forced_includes에 강제 포함. tool calling으로 LLM이 추가 fetch 가능.      |
| 6   | history_entry status enum이 'generated' 같은 새 값 필요 | 마이그레이션 vs 기존 enum     | v1엔 'ok'로 기록 + ai_history.generated_sql 존재 여부로 generation-only 식별. 새 status 도입은 v1.5 (마이그레이션 마찰 회피).        |
| 7   | Ollama localhost 미기동                           | sync 다 fail / generation 실패 | Test 버튼이 baseUrl에 GET / 또는 /api/tags 1회. 실패 시 분명한 에러 메시지 + retry. background sync은 첫 fail 후 abort.             |
| 8   | Vercel AI SDK 6.x breaking changes                | 컴파일 안 됨                  | AI SDK 6는 안정 stable 채널. 단, provider 패키지 호환 버전 lock. `@ai-sdk/openai ^3`, `@ai-sdk/anthropic ^3`, Gemini/Ollama 패키지는 6 호환 버전 명시. |
| 9   | DDL 빌더가 PG 17 신기능 누락 (예: virtual generated col) | RAG 컨텍스트 빈약             | information_schema + pg_catalog로 핵심(컬럼/타입/PK/FK/COMMENT)만 신뢰. 모르는 건 무시. spec 변경 안전. v1.5 강화.                 |
| 10  | sample_rows tool로 PII 누출                       | privacy / GDPR 우려            | Default OFF. Settings에 항상 표시 + 토글. 활성화 시 설명: "rows go to the LLM provider, no redaction".                              |
| 11  | 임베딩 provider 다양성 (모델별 dim 다름)          | 모델 바꾸면 인덱스 무효       | schema_embedding.embedding_model + embedding_dim 컬럼 보유. 매칭 안 되면 단순 SKIP/재embed. 자동.                                  |
| 12  | streamText 도중 사용자가 Cmd+K 다시 눌러 race     | 두 스트림 동시 진행 / UI 깨짐 | CmdKPalette는 단일 인스턴스. 새 호출은 이전 AbortController.abort() 먼저.                                                          |
| 13  | 사용자가 strict mode에서 매번 키워드 타이핑하기 귀찮 | 사용성 떨어짐 → 기본 OFF       | Default Standard. Strict는 opt-in. 모달 첫 진입 시 1회 안내.                                                                          |

## 11. Testing strategy

### Rust unit (인메모리 / no DB)

- `commands::destructive::classify_destructive` — DROP/TRUNCATE/DELETE-no-where/UPDATE-no-where/ALTER ... DROP COLUMN/REVOKE ALL/VACUUM FULL 각 1건씩. 멀티 statement 파일도 테스트.
- `db::schema_embed::build_table_ddl` — fixture pg_catalog row → 예상 DDL 문자열.
- `db::embedding_store::cosine_top_k` — known vectors → 정렬 안정성.
- `commands::ai_secrets` — 라운드트립 (`secrets::set_get_delete_roundtrip` 패턴 재사용).

### Rust integration (docker postgres:16-alpine)

- `tests/destructive.rs` — 실제 SQL 표본 30~50개를 분류기에 통과시켜 expected kind 비교. PG 함수 / DDL / DML 다양 표본.
- `tests/schema_index.rs` — 작은 fixture 스키마(테이블 5개) → mock embedding endpoint(httpmock) → BLOB 저장 → top_k 호출.
- `tests/ai_tools.rs` — get_table_schema / list_indexes / sample_rows 결과 형태.

### Frontend unit (vitest)

- `lib/ai/destructive.ts` — 정규식 빠른 검사기. Rust AST 결과와 mismatch 허용 범위 명시 (always-warn-not-always-block — UI 미리 경고용).
- `lib/ai/prompts.ts` — system prompt 합성 결과 snapshot.
- `lib/ai/providers.ts` — 4 provider 인스턴스 생성 + baseUrl override.
- `store/ai.ts` — provider toggle / default switching.
- `features/ai/DestructiveModal` — Standard / Strict 분기, 키워드 일치 검증.

### Manual verification

- `docs/superpowers/plans/manual-verification-week-4.md`:
  - 4 provider 각자 키 셋업 → Test 통과.
  - 빈 스키마 / 100 테이블 / 1000 테이블 환경에서 sync 진행률 + 시간.
  - Cmd+K 자연어 5개 prompt → 각 provider별 정상 SQL 생성.
  - tool calling: prompt에서 명시되지 않은 테이블 LLM이 follow-up으로 fetch.
  - destructive 모든 패턴 (DROP/TRUNCATE/DELETE-no-where/UPDATE-no-where/ALTER DROP COL/VACUUM FULL) Standard / Strict 둘 다.
  - 키 누락 / 네트워크 끊김 / 토큰 한도 초과 각 에러 UX.
  - DDL 실행 → 다음 Cmd+K가 새 테이블 인지하는지 (dirty re-sync 동작).
  - sample_rows OFF/ON 차이 검증.
- Week 2/3 manual verification 재실행 (회귀 가드).

## 12. Folder structure (신규/변경)

```
src/
  features/
    ai/                          NEW
      CmdKPalette.tsx
      SqlDiffView.tsx
      DestructiveModal.tsx
      AiHistoryEntry.tsx
    settings/                    NEW
      SettingsDialog.tsx
      ProviderSection.tsx
      ModelPicker.tsx
      SchemaIndexPanel.tsx
    editor/                       (확장: Cmd+K 단축키 진입점)
    history/                      (확장: source='ai' 렌더)
  store/
    ai.ts                        NEW
    schemaIndex.ts               NEW
    settings.ts                   (확장: enabledProviders 등)
  lib/
    ai/                          NEW
      providers.ts
      prompts.ts
      tools.ts
      destructive.ts
      stream.ts
    keychain.ts                  NEW

src-tauri/src/
  commands/
    ai_secrets.rs                NEW
    schema_index.rs              NEW
    ai_tools.rs                  NEW
    destructive.rs               NEW
    history.rs                    (확장: ai_history insert)
    query.rs                      (확장: classify_destructive 호출 진입점)
  db/
    schema_embed.rs              NEW
    embedding_store.rs           NEW
    state.rs                      (migration 003_ai)
  secrets.rs                      (확장: ai_entry helper)
  errors.rs                       (확장: Ai/SchemaIndex/...)

infra/postgres/                  (변경 없음 — PG 16 그대로)
docs/superpowers/
  specs/2026-04-28-week-4-ai-natural-language-sql-design.md
  plans/2026-04-28-week-4-ai-natural-language-sql.md            ← writing-plans에서 생성
  plans/manual-verification-week-4.md                            ← Phase 5
```

## 13. Implementation slice order (high level)

writing-plans skill에서 더 잘게 쪼개지지만, 의존성 순서:

### Phase 1 — Provider 인프라

1. **AI secrets storage** — `commands/ai_secrets.rs`, secrets.rs ai_entry, frontend `lib/keychain.ts`. unit test.
2. **Settings UI 셸** — `SettingsDialog` + `ProviderSection`. 4 provider 카드, key 입력/저장/삭제. Test 버튼은 stub.
3. **Provider factory** — `lib/ai/providers.ts`. AI SDK 4종 모델 인스턴스 생성. baseUrl override 지원. Gemini/Ollama provider 패키지 추가.
4. **Test 버튼 동작** — 짧은 generateText 1회로 provider liveness 검증.

### Phase 2 — Destructive 가드

5. **Rust classify_destructive** — `commands/destructive.rs` + sqlparser AST. unit test 30+ 케이스.
6. **DestructiveModal 컴포넌트** — Standard/Strict 분기. 키워드 일치 검증.
7. **execute_query 진입점에 게이트 통합** — Cmd+Enter / Cmd+K Apply / palette 재실행 모두.

### Phase 3 — 스키마 임베딩 인덱스

8. **migration 003_ai** + state.rs 확장.
9. **build_table_ddl** — pg_catalog 기반 DDL 합성. 단위 테스트 fixture.
10. **embedding HTTP** — `reqwest`로 OpenAI/Gemini/Ollama embeddings. provider별 어댑터.
11. **embedding_store 저장 + cosine top_k** — BLOB 저장 / 비교. 단위 테스트.
12. **sync_schema_index command + emit progress** — 진행률 이벤트 + 부분 실패 허용.
13. **SchemaIndexPanel UI** — Settings 안에서 진행률 표시 / Rebuild / Clear / Auto-sync 토글.

### Phase 4 — Cmd+K 자연어→SQL

14. **lib/ai/prompts.ts + tools.ts** — system prompt 합성, tool 정의.
15. **CmdKPalette 컴포넌트** — floating input, AbortController, streaming 표시.
16. **SqlDiffView** — Monaco DiffEditor wrapper.
17. **schema_top_k command** — embedding lookup + forced_includes 토큰 매칭.
18. **AI history 기록** — generation 직후 history_entry + ai_history insert.
19. **에디터 통합** — Cmd+K 단축키 → CmdKPalette 마운트 → Apply 시 selection replace / cursor insert.

### Phase 5 — 마무리

20. **vitest 추가 케이스** — destructive / providers / store/ai.
21. **수동 검증 체크리스트** — `manual-verification-week-4.md` 작성 + 1회 round-trip.
22. **Week 2/3 회귀 검증 재실행**.
23. **README / docs 짧은 업데이트** — Settings → Providers 안내 추가 (사용자 지시 있을 때만).

**의존성 핵심**:

- 1~4 (provider 인프라) → 14~19 (Cmd+K 흐름) 전제.
- 5~7 (destructive) → 7번이 19의 Run 진입점에 게이트 연결.
- 8~13 (RAG 인덱스) → 17 (top_k) → 18 (generation 호출 시 사용).
- 6 (DestructiveModal) ↔ 7 (사용자 직접 실행) ↔ 19 (AI Apply 후 실행) — 같은 모달이 세 진입점에서 재사용.
