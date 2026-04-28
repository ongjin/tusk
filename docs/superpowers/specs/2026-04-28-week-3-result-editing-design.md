# Week 3 — Result 인라인 편집 + 명시적 트랜잭션 (design spec)

> **Date:** 2026-04-28
> **Scope:** PLAN.md Week 3 전체 — 인라인 편집 / 트랜잭션 / decoder 정상화 / 쿼리 취소 / 쿼리 히스토리 / Cmd+P 팔레트 / Export / 컨텍스트 메뉴
> **Status:** Drafted, awaiting user review

---

## 1. Goal & success criteria

Week 2까지의 read-only Postgres 클라이언트를 **편집 가능한 클라이언트 + 트랜잭션 1급 시민**으로 진화시킨다. PLAN.md Week 3 항목을 한 spec에 묶음 (3a/3b 분할 안 함).

End user 기준 성공 조건:

1. 단일 테이블 SELECT 결과 → 셀 더블클릭 → 타입별 위젯에서 편집 → Preview 모달이 **실제로 실행될 SQL**(리터럴 인라인) 표시 → Submit → commit 또는 충돌 시 명확한 안내.
2. Auto-commit OFF 토글 → 에디터 실행 + 인라인 편집 submit이 같은 sticky connection의 한 트랜잭션으로 묶임 → COMMIT/ROLLBACK 명시적 버튼 + 단축키 → 미커밋 상태로 종료 시도 시 경고.
3. `numeric/uuid/inet/timestamp/timestamptz/jsonb/bytea/array/enum` 모두 정확하게 표시되고 정확하게 편집됨. Week 2의 `<unsupported type>` 영구 폐기.
4. 모든 실행이 SQLite 히스토리 entry로 남고 (트랜잭션 묶음은 sub-statements로 펼쳐짐) Cmd+P 팔레트에서 텍스트 검색으로 과거 쿼리 재실행 가능.
5. CSV / JSON / SQL INSERT export — 선택 row 또는 전체.
6. 셀 우클릭 컨텍스트 메뉴: Copy / Copy as INSERT / Set NULL / Filter by this value.
7. 쿼리 취소: 토스트의 Cancel 버튼으로 long-running statement 즉시 종료.
8. 결과셋이 클 때도 편집된 row 수만큼만 메모리 카피 (전체 결과 deep copy 금지). 단, 결과 row > 10_000이면 편집 자체는 비활성 + 안내 (Risk #8 참고).

이 8개가 만족되면 Week 3 완료.

## 2. Out of scope (Week 4 이후)

- 자연어 → SQL (Cmd+K AI) — Week 4
- destructive 쿼리 confirmation 모달 (DROP/TRUNCATE) — Week 4 (AI 모달과 통합 설계)
- 의미 검색(쿼리 히스토리 임베딩) — Week 7
- 스키마-aware 자동완성 — Week 4+
- Vector 컬럼 편집 (Week 3엔 read-only + 차원 표시)
- FK dropdown의 cross-database lookup (같은 connection의 다른 schema/db만 지원)
- Multi-row paste / CSV 붙여넣기 bulk INSERT — v1.5
- 결과 페이지네이션 단위 편집 (Week 3엔 결과 row 수 ≤ 10_000일 때만 편집 활성)
- 컨텍스트 메뉴 "Find similar" (vector) — v1.5

## 3. Architecture

```
┌── Frontend (React + zustand) ──────────────────────────────┐
│  features/                                                  │
│    editing/        ← NEW: 인라인 편집 코어                  │
│      EditableCell.tsx     widgets/{Text,Int,Bigint,Numeric,│
│      PendingBadge.tsx       Bool,Date,Time,Timestamp,      │
│      PreviewModal.tsx       Uuid,Json,Bytea,Vector,        │
│      ConflictModal.tsx      Enum,Fk}.tsx                   │
│    transactions/   ← NEW: 명시적 tx 모드                    │
│      AutoCommitToggle.tsx   TxIndicator.tsx                │
│      TxSidePanel.tsx                                        │
│    history/        ← NEW                                    │
│      HistoryPalette.tsx     HistoryEntry.tsx               │
│    export/         ← NEW                                    │
│      ExportDialog.tsx                                       │
│    results/         (확장)                                  │
│      ResultsGrid.tsx        ← editable-mode 분기            │
│      cells.tsx              ← 새 Cell 형태 렌더 + 편집 진입 │
│      ContextMenu.tsx        ← NEW                           │
│    editor/, schema/, connections/  (기존 유지)              │
│  store/                                                     │
│    pendingChanges.ts        ← NEW                           │
│    transactions.ts          ← NEW                           │
│    history.ts               ← NEW                           │
│    settings.ts              ← Strict/PkOnly 토글 추가        │
│  lib/                                                       │
│    sqlAst.ts                ← invoke 래퍼                   │
│    pgLiterals.ts            ← (Rust와 동치 검증용 mirror)    │
│    types.ts                 ← Cell/PendingChange/Tx*/...    │
└─────────────────────────────────────────────────────────────┘
                          ↕ Tauri invoke()
┌── Rust (src-tauri) ────────────────────────────────────────┐
│  commands/                                                  │
│    query.rs                  ← typed dispatch 결과 응답      │
│    transactions.rs   NEW     ← begin/commit/rollback        │
│    editing.rs        NEW     ← submit/preview pending       │
│    history.rs        NEW     ← record/list/search           │
│    export.rs         NEW     ← csv/json/sql 직렬화 stream   │
│    sqlast.rs         NEW     ← parse_select_target          │
│    cancel.rs         NEW     ← cancel_query                 │
│  db/                                                        │
│    pool.rs                   ← ActiveConnection.tx_slot     │
│    state.rs                  ← migration 002_history        │
│    decoder.rs        NEW     ← OID-dispatch typed decoder   │
│    pg_literals.rs    NEW     ← PG 리터럴 인라인 직렬화       │
│    pg_meta.rs        NEW     ← PK / enum / FK 조회 + LRU    │
│  errors.rs                   ← Editing/Tx/History/...      │
└─────────────────────────────────────────────────────────────┘
```

### ActiveConnection 확장

```rust
pub struct ActiveConnection {
    pub pool: PgPool,
    pub tunnel: Option<TunnelHandle>,
    pub tx_slot: Mutex<Option<StickyTx>>,        // ← 추가
}

pub struct StickyTx {
    pub tx_id: String,                            // UUID
    pub conn: PoolConnection<Postgres>,           // 점유
    pub started_at: Instant,
    pub backend_pid: i32,
    pub statement_count: u32,
    pub history_entry_id: String,
}
```

`tx_slot`을 `Mutex`로 감싸는 이유: 명령 호출들이 직렬화돼도 한 connection의 statement는 어차피 직렬임. 사용자 입장에선 자연스러움.

`Drop` 구현: tx_slot 있으면 best-effort `ROLLBACK` (timeout 1s) → conn drop → tunnel drop.

## 4. Libraries (Week 3 추가)

| 영역            | 라이브러리                               | 사유                                                                                                                                  |
| --------------- | ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| SQL AST         | `sqlparser 0.50+` (postgres dialect)     | PK 감지 시 단일 테이블 SELECT 인식. 파싱 실패 = read-only로 fallback.                                                                 |
| Numeric         | `bigdecimal` (sqlx `bigdecimal` feature) | numeric 정밀도 보존. 응답은 `BigDecimal::to_string()` 그대로 (정규화 X) — `numeric(p,s)`의 trailing zero 보존 → 편집 round-trip 안전. |
| Date/Time       | 기존 `chrono` 유지 + sqlx `PgInterval`   | timestamp/date/time은 chrono. interval은 sqlx `PgInterval`을 직접 ISO 8601 duration 문자열로 직렬화.                                  |
| UUID            | 기존 `uuid` (sqlx feature)               |                                                                                                                                       |
| Network         | `ipnetwork` (sqlx `ipnetwork` feature)   | inet/cidr typed 디코드. 응답은 텍스트 표현.                                                                                           |
| Frontend SQL hl | Monaco 기존 사용 (위젯 미니 인스턴스)    |                                                                                                                                       |

## 5. Data model

### 5.1 Frontend types (`src/lib/types.ts`)

```ts
// 새 cell 모델 — Week 2의 raw JsonValue를 대체
export type Cell =
  | { kind: "null" }
  | { kind: "bool"; value: boolean }
  | { kind: "int"; value: number } // int2/int4
  | { kind: "bigint"; value: string } // int8 — JS number 정밀도 회피
  | { kind: "float"; value: number } // float4/float8
  | { kind: "numeric"; value: string } // 정밀도 보존
  | { kind: "text"; value: string } // text/varchar/bpchar
  | { kind: "bytea"; b64: string }
  | { kind: "uuid"; value: string }
  | { kind: "inet"; value: string } // inet/cidr
  | { kind: "date"; value: string } // YYYY-MM-DD
  | { kind: "time"; value: string }
  | { kind: "timetz"; value: string }
  | { kind: "timestamp"; value: string } // ISO no TZ
  | { kind: "timestamptz"; value: string } // ISO with offset
  | { kind: "interval"; iso: string } // ISO 8601 duration
  | { kind: "json"; value: unknown } // jsonb/json
  | { kind: "array"; elem: PgTypeName; values: Cell[] }
  | { kind: "enum"; typeName: string; value: string }
  | { kind: "vector"; dim: number; values: number[] }
  | { kind: "unknown"; oid: number; text: string };

export type PgTypeName =
  | "bool"
  | "int2"
  | "int4"
  | "int8"
  | "float4"
  | "float8"
  | "numeric"
  | "text"
  | "varchar"
  | "bpchar"
  | "bytea"
  | "uuid"
  | "inet"
  | "cidr"
  | "date"
  | "time"
  | "timetz"
  | "timestamp"
  | "timestamptz"
  | "interval"
  | "jsonb"
  | "json"
  | "enum"
  | "vector"
  | "unknown";

// 결과셋 메타 (sqlparser-rs + pg_meta enrich 결과)
export interface ResultMeta {
  editable: boolean;
  reason?:
    | "no-pk"
    | "multi-table"
    | "computed"
    | "pk-not-in-select"
    | "too-large"
    | "parser-failed"
    | "unknown-type";
  table?: { schema: string; name: string };
  pkColumns: string[];
  pkColumnIndices: number[]; // 결과 컬럼 인덱스로 매핑
  columnTypes: ColumnTypeMeta[];
}

export interface ColumnTypeMeta {
  name: string;
  oid: number;
  typeName: PgTypeName;
  nullable: boolean;
  enumValues?: string[];
  fk?: { schema: string; table: string; column: string };
}

// 인라인 편집 상태 — Map<rowKey, PendingChange>
export interface PendingChange {
  rowKey: string; // PK canonical JSON
  table: { schema: string; name: string };
  pk: { columns: string[]; values: Cell[] };
  edits: { column: string; original: Cell; next: Cell | { kind: "null" } }[];
  op: "update" | "insert" | "delete";
  capturedRow: Cell[]; // strict 모드용 (결과 컬럼 순서)
  capturedAt: number;
}

// 트랜잭션 — Rust가 SoT, 프론트는 mirror
export interface TxState {
  connId: string;
  active: boolean;
  txId?: string;
  startedAt?: number;
  statementCount: number;
  lastError?: string;
  pid?: number;
}

export interface HistoryEntry {
  id: string;
  connId: string;
  source: "editor" | "inline" | "palette";
  txId?: string;
  sqlPreview: string; // 첫 200자
  startedAt: number;
  durationMs: number;
  rowCount?: number;
  status: "ok" | "error" | "cancelled" | "rolled_back";
  errorMessage?: string;
  statementCount: number;
}

export interface HistoryStatement {
  id: string;
  entryId: string;
  ordinal: number;
  sql: string;
  durationMs: number;
  rowCount?: number;
  status: "ok" | "error";
  errorMessage?: string;
}
```

**메모리 추적 원칙**: `pendingChanges`는 `Map<rowKey, PendingChange>` 한 곳에만 보관. 결과 grid의 row 데이터(원본)는 결과셋 페이지 그대로 두고 **편집된 row만 capturedRow 복사**. 100k row 결과에서 5개 편집 = 카피 5개.

### 5.2 Rust types

```rust
// commands/editing.rs
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingBatch {
    pub batch_id: String,
    pub op: PendingOp,
    pub table: TableRef,
    pub pk_columns: Vec<String>,
    pub pk_values: Vec<JsonValue>,
    pub edits: Vec<ColumnEdit>,
    pub captured_row: Vec<JsonValue>,
    pub captured_columns: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnEdit { pub column: String, pub next: JsonValue }

#[derive(Debug, Deserialize)]
pub struct TableRef { pub schema: String, pub name: String }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PendingOp { Update, Insert, Delete }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConflictMode { PkOnly, Strict }

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitResult {
    pub batches: Vec<BatchResult>,
    pub tx_state: Option<TxStateSnapshot>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum BatchResult {
    Ok       { batch_id: String, affected: u64, executed_sql: String },
    Conflict { batch_id: String, executed_sql: String, current: Vec<JsonValue> },
    Error    { batch_id: String, executed_sql: String, message: String },
}

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
```

### 5.3 SQLite 마이그레이션 (`migration 002_history.sql`)

```sql
CREATE TABLE history_entry (
    id              TEXT PRIMARY KEY,
    conn_id         TEXT NOT NULL,
    source          TEXT NOT NULL,             -- editor | inline | palette
    tx_id           TEXT,                      -- nullable
    sql_preview     TEXT NOT NULL,             -- 첫 200자
    sql_full        TEXT,                      -- 단일 entry는 그대로, tx 묶음은 NULL
    started_at      INTEGER NOT NULL,
    duration_ms     INTEGER NOT NULL,
    row_count       INTEGER,
    status          TEXT NOT NULL,             -- ok | error | cancelled | rolled_back
    error_message   TEXT,
    statement_count INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_history_entry_conn_started ON history_entry(conn_id, started_at DESC);
CREATE INDEX idx_history_entry_tx ON history_entry(tx_id);

CREATE TABLE history_statement (
    id              TEXT PRIMARY KEY,
    entry_id        TEXT NOT NULL REFERENCES history_entry(id) ON DELETE CASCADE,
    ordinal         INTEGER NOT NULL,
    sql             TEXT NOT NULL,
    duration_ms     INTEGER NOT NULL,
    row_count       INTEGER,
    status          TEXT NOT NULL,             -- ok | error
    error_message   TEXT
);
CREATE INDEX idx_history_statement_entry ON history_statement(entry_id, ordinal);
```

Cmd+P 팔레트의 텍스트 검색은 v1엔 단순 LIKE. 인덱스는 conn별 시간순. Week 7에서 sqlite-vec.

### 5.4 PG 메타 조회 (`db/pg_meta.rs`)

쿼리 결과에 PK 정보 붙이는 단계 — SELECT 실행 후 1회:

```rust
pub async fn enrich_result_meta(
    pool: &PgPool,
    parsed: ParsedSelect,
    result_columns: &[ColumnMeta],
) -> Result<ResultMeta, TuskError>
```

수행 쿼리 (한 번에 join 또는 별도 호출):

- PK 컬럼 — `pg_index` JOIN `pg_attribute` WHERE `indisprimary`
- 컬럼 nullable / typoid — `pg_attribute` (이미 sqlx 응답에 OID 있으므로 컬럼명 매칭)
- enum 값 — 컬럼 typtype='e'면 `pg_enum`
- FK 타깃 — `pg_constraint` (contype='f')

이 조회는 결과셋 컬럼이 모두 같은 단일 테이블에서 온 경우에만 실행. 그 외엔 `editable: false` 즉시 응답.

LRU 캐시: key = `(connId, schema, table)`, TTL = 60s, 무효화 = disconnect 또는 사용자 새로고침.

## 6. 핵심 흐름

### 6.1 SELECT → 메타 enrich

```
사용자 Cmd+Enter (sql, connId)
 → frontend invoke("execute_query", { connId, sql })
 → Rust:
   a. lib::sqlast::parse_select_target(&sql) →
      ParsedSelect::SingleTable { schema, table } | NotEditable(reason)
   b. tx_slot 차있으면 sticky conn, 비어있으면 pool.acquire()
   c. sqlx::query(&sql).fetch_all() → Vec<PgRow>
   d. db::decoder::decode_row(&row, columns) → Vec<Cell>  (OID dispatch)
   e. 단일 테이블이면 db::pg_meta::enrich_result_meta(pool, parsed, &columns)
   f. history::record_single(...) 또는 sticky_tx.append_statement(...)
   g. 응답: QueryResult { columns, rows: Vec<Vec<Cell>>, durationMs, rowCount, meta, txState? }

오류:
- sqlparser 실패 → ParsedSelect::Failed → editable=false reason='parser-failed'
- decoder가 unknown OID → Cell::Unknown { oid, text } (전체 응답은 정상)
- enrich 쿼리 실패 → editable=false reason='unknown-type', 결과 본체는 정상 응답
```

### 6.2 인라인 편집 — Auto-commit ON, 단일 row UPDATE

```
1. 사용자 셀 더블클릭 → EditableCell이 widgets/<type>.tsx 마운트
2. 사용자 새 값 입력 → Enter
3. pendingChanges store: upsert(rowKey, PendingChange{ op:'update', edits:[...], capturedRow })
4. 사용자 Submit
5. invoke("submit_pending_changes", { connId, batches, mode:'pkOnly' })
6. Rust:
   a. tx_slot 비어있음 → pool.acquire() + sqlx::Transaction begin
   b. history_entry 생성 (source='inline', tx_id=None, statement_count=batches.len())
   c. for batch in batches:
        - editing::build_update(batch, PkOnly):
            sql = "UPDATE schema.table SET col1 = $1, col2 = $2 WHERE pk = $N"
            binds = [next_values..., pk_value]
        - editing::build_preview(batch, PkOnly):
            "UPDATE schema.table SET col1 = 'new' WHERE pk = 42"  ← 응답용
        - tx.execute(sql, binds) → affected
        - 충돌? PkOnly에선 affected=0이어도 conflict 아님 (그저 row가 사라진 것)
        - history_statement append
   d. 모든 배치 OK → tx.commit
   e. 응답: SubmitResult{ batches: [Ok{...}], txState: None }
7. frontend:
   - sonner toast "1 row updated"
   - pendingChanges.delete(rowKey)
   - 결과 grid의 해당 row를 새 값으로 갱신
```

### 6.3 인라인 편집 — Strict 모드 충돌 (Option X atomic)

```
1~5: PkOnly와 동일하되 mode:'strict'
6. Rust:
   a~b. 동일
   c. for batch in batches:
        - build_update(batch, Strict):
            sql = "UPDATE schema.table SET ... WHERE pk=$1
                   AND col1 IS NOT DISTINCT FROM $2
                   AND col2 IS NOT DISTINCT FROM $3 ..."
              (NULL-safe 비교; float 컬럼은 strict 비교에서 제외하고 응답에 안내)
        - tx.execute → affected
        - affected == 0 → 충돌!
            - tx.rollback (전체 atomic 무효화)
            - 같은 conn에서 server-current 다시 SELECT하여 current row 캡처
            - BatchResult::Conflict 기록, 다른 batch는 이 응답에서 build_preview만 채우고 Error("rolled back due to conflict in batch X")
            - break
   d. 충돌 없으면 tx.commit
   e. 응답
7. frontend:
   - 충돌 있으면 ConflictModal: "Row was modified by someone else. [Force overwrite] [Discard your edits] [Re-edit on top of server]"
   - "Force overwrite" = mode='pkOnly'로 다시 submit
   - "Re-edit on top of server" = capturedRow를 server current로 교체, edits 유지
```

### 6.4 명시적 트랜잭션 모드

```
AutoCommitToggle OFF:
  invoke("tx_begin", { connId })
  Rust:
    pool.acquire() → conn
    "BEGIN" 실행
    backend_pid = SELECT pg_backend_pid()
    history_entry 생성 (source='editor' or 'inline', tx_id=Uuid::new_v4(), statement_count=0, sql_full=NULL)
    tx_slot = Some(StickyTx{...})
  응답: TxStateSnapshot

이후 모든 execute_query / submit_pending_changes:
  tx_slot 차있음 → sticky.conn 사용
  statement 끝나면 history_statement append + sticky.statement_count += 1
  응답에 txState 갱신

Commit (Cmd+Shift+C):
  invoke("tx_commit", { connId })
  Rust: sticky.conn에 "COMMIT" → history_entry.status='ok', duration_ms 갱신, statement_count 확정
  tx_slot = None, conn drop

Rollback (Cmd+Shift+R) — 동일 흐름, status='rolled_back'

Abort 상태:
  중간 statement가 PG 에러로 abort → 다음 execute는 PG가 거부 → TxAborted 에러
  UI: "Transaction aborted. Roll back to continue."

종료 / disconnect 시:
  tx_slot 있으면 confirm 모달 (Commit / Rollback / Cancel)
  사용자가 Cancel 안 한 경우: 선택대로 호출 후 close
  ActiveConnection::drop에 best-effort ROLLBACK 1s timeout (앱 강제종료 대비)
```

### 6.5 쿼리 취소

```
1. invoke("execute_query", { connId, sql })
   - Rust 시작 시 pid 조회 → 'query:started' 이벤트 emit { connId, pid, sql, startedAt }
2. frontend: sonner toast "Running... [Cancel]"  (긴 쿼리만; 시작 후 500ms 지나면 표시)
3. 사용자 Cancel 클릭
   - invoke("cancel_query", { connId, pid, startedAt })
   - Rust:
     - registry의 그 connection이 여전히 같은 pid인지 (sticky tx면 동일, auto면 race 가능) 확인
     - 토큰 mismatch면 silent ignore
     - 별도 admin connection 잠시 잡아 SELECT pg_cancel_backend(pid) 실행 후 close
4. 첫 invoke: "canceling statement due to user request" 에러 → TuskError::QueryCancelled
5. history.status='cancelled'
6. 토스트 "Query cancelled"

tx 모드에서 cancel:
  - statement만 취소되고 tx는 abort 상태로 진입 (PG 동작)
  - UI: "Query cancelled. Transaction is now in aborted state — roll back to continue."
```

## 7. UI

### 7.1 결과 grid (편집 모드)

```
┌─ public.users ──────────────────────────── ✏️ Editable | 🟡 Tx (3 stmts) ─┐
│ Mode: [PK only ▼]   1 pending  [Preview] [Submit] [Revert]                │
├──────────────────────────────────────────────────────────────────────────┤
│  id │ email                          │ created_at                       │
│  42 │ new@example.com   ← yellow bg │ 2026-04-28T...                   │
│  43 │ b@example.com                  │ 2026-04-28T...                   │
│ ... │                                │                                  │
└──────────────────────────────────────────────────────────────────────────┘
```

- 좌상단 ✏️ = editable result. 우상단 🟡 = 트랜잭션 활성.
- "PK only / Strict" 모드 토글 (settings store, default PkOnly).
- Pending 셀: 노란 배경 + 호버 시 "Original: <value>" 툴팁.
- 편집 disabled 결과셋: ✏️ 자리에 회색 자물쇠 + 툴팁에 reason ("Read-only — multi-table SELECT").

### 7.2 PreviewModal

```
┌─ Preview pending changes ─────────────────────────────────────────────┐
│ 1 statement will be executed:                                          │
│                                                                        │
│   UPDATE public.users SET email = 'new@example.com' WHERE id = 42      │
│                                                                        │
│ Note: actual execution uses parameterized binds; this rendering       │
│ inlines literals using PG escape rules.                                │
│                                                                        │
│                                          [Cancel] [Submit Now]         │
└────────────────────────────────────────────────────────────────────────┘
```

### 7.3 트랜잭션 인디케이터

- 상단 바 (탭 영역 아래): `🟡 Transaction (3 stmts) · started 4s ago  [Commit] [Rollback]` — 모든 탭에서 공유.
- 각 탭 헤더에 작은 🟡 dot.
- 첫 진입(앱 lifetime 1회) 시 안내 모달: "Auto-commit OFF — every tab on this connection now executes inside the same transaction."

### 7.4 컨텍스트 메뉴 (셀 우클릭)

```
Copy
Copy as INSERT      ← pg_literals 재사용
Set NULL            ← nullable일 때만
Filter by this value
─────
(편집 가능 시) Edit cell
(편집 가능 시) Delete row
```

### 7.5 Cmd+P 명령 팔레트

- Cmd+P 단축키 → 모달.
- 입력 → SQLite LIKE on `history_entry.sql_preview` (현 connection 우선, 그 다음 전체).
- 결과 클릭 → 새 에디터 탭에 SQL 로드. 트랜잭션 entry는 sub-statements 펼쳐서 한 탭에 `;`로 join.

### 7.6 ExportDialog

- Format: CSV / JSON / SQL INSERT.
- Scope: Selected rows / All rows.
- Encoding: UTF-8 (CSV는 BOM 토글).
- SQL INSERT은 `pg_literals` 재사용해 단일 묶음 INSERT 또는 한 줄에 한 INSERT.

## 8. Errors

```rust
#[derive(thiserror::Error, Debug, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum TuskError {
    // ... 기존 (Connection, Query, Tunnel, Ssh, State, Secrets, Internal)

    #[error("Editing failed: {0}")]
    Editing(String),

    #[error("Conflict on batch")]
    Conflict {
        batch_id: String,
        executed_sql: String,
        current: serde_json::Value,        // server-current row
    },

    #[error("Transaction error: {0}")]
    Tx(String),

    #[error("Transaction aborted — only ROLLBACK is allowed")]
    TxAborted,

    #[error("Query cancelled")]
    QueryCancelled,

    #[error("History error: {0}")]
    History(String),

    #[error("Unsupported column type for editing: oid={oid}, name={name}")]
    UnsupportedEditType { oid: u32, name: String },
}
```

## 9. Decisions taken (Q&A from brainstorming)

이 spec의 큰 결정 사항:

1. **스코프**: 한 spec에 PLAN.md Week 3 전체 (3a/3b 분할 폐기).
2. **PK 감지**: sqlparser-rs로 단일 테이블 SELECT 인식 + `information_schema` PK 조회. JOIN/CTE/subquery는 read-only. 파서 실패 시 보수적으로 read-only. (Option B)
3. **충돌 감지**: WHERE에 원본 값 박는 방식. **PK only / Strict** 두 모드 토글, 기본 PK only. NULL은 `IS NOT DISTINCT FROM`. float은 strict 비교에서 제외. (Option A)
4. **트랜잭션 모델**: 연결 단위 sticky connection. 모든 탭이 같은 tx 공유 + 인디케이터로 명시. 탭별 tx는 v1.5. (Option A)
5. **위젯 범위**: PLAN.md 12개 타입 위젯 전부 + nullable Set NULL. (Option A)
6. **Decoder**: PG OID 기반 typed dispatch 전면 재작성. `<unsupported type>` 폐기, `Cell::Unknown` fallback. (Option A)
7. **Submit 실행 모델**: 프론트는 구조화 객체 전송. Rust가 parameterized 실행 + 별도로 리터럴 인라인 SQL 빌드해 Preview에 사용. 같은 모듈을 Copy as INSERT / SQL export에 재사용. (Option C)
8. **쿼리 히스토리**: 의도 단위 entry + 트랜잭션 묶음은 sub-statement 테이블. (Option B)
9. **쿼리 취소**: Week 3 포함. 토큰 매칭으로 race 회피.
10. **다중 batch 충돌 시**: Atomic — 충돌 1건이면 같은 submit의 모든 batch 롤백. (Option X)

## 10. Risks

| #   | 위험                                                           | 영향                     | 완화                                                                                                                                              |
| --- | -------------------------------------------------------------- | ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Decoder 재작성이 Week 2 결과 grid 회귀                         | 신뢰 즉사                | OID-dispatch 슬라이스에 golden test set (PG 16 모든 핵심 타입 round-trip). 미지원 OID는 `Cell::Unknown` fallback (`<unsupported type>` 영구 폐기) |
| 2   | sqlparser-rs가 PG 방언 일부 못 파싱 (RETURNING/ON CONFLICT 등) | 멀쩡한 쿼리가 read-only  | 파서 실패 = 무조건 `editable:false reason:'parser-failed'`. 보수적. 강화는 v1.5                                                                   |
| 3   | sticky tx 안 long-running 쿼리 → 다른 탭 stuck                 | 멀티탭 멘탈 모델 깨짐    | 탭 헤더 🟡 + 상단바 인디케이터 + Cancel 버튼 항시 노출. tx ON 첫 진입시 1회 안내                                                                  |
| 4   | strict 모드 NULL/float 비교 미묘                               | 같은데 충돌로 잘못 보고  | NULL은 `IS NOT DISTINCT FROM`. float은 strict 비교 제외 + 안내                                                                                    |
| 5   | Preview SQL과 실제 실행 SQL 미세 차이                          | 사용자 신뢰 손상         | `pg_literals` PG 16 정합 단위 테스트. Preview 푸터에 "actual execution uses parameterized binds" 명시                                             |
| 6   | tx 활성 상태 disconnect / 앱 종료                              | 서버 idle in tx          | drop에서 best-effort ROLLBACK (1s timeout) + 종료 confirm 모달                                                                                    |
| 7   | enum/FK 메타 조회 추가 RTT                                     | SELECT 느려짐            | enrich_result_meta 1회만. (connId, schema, table) LRU 60s                                                                                         |
| 8   | 100k+ row 결과에 편집 활성                                     | UI 복잡 + 메모리 폭발    | 결과 row > 10_000이면 편집 비활성 + "Reduce LIMIT to enable editing" 안내. v1.5에서 페이지 단위 편집                                              |
| 9   | Cmd+P 팔레트 큰 히스토리에서 느림                              | 검색 응답 지연           | SQLite LIKE는 10만 entry까지 OK. (conn_id, started_at DESC) 인덱스. Week 7에서 sqlite-vec                                                         |
| 10  | OID dispatch에 누락된 타입                                     | 컬럼 표시 안 됨          | `Cell::Unknown { oid, text }` 항상 fallback (raw bytes utf8 best-effort). 회색 italic + "Edit unavailable" 툴팁                                   |
| 11  | 쿼리 취소 race                                                 | 다음 statement 잘못 취소 | cancel_query는 `(pid, startedAt)` 토큰 매칭. mismatch silent ignore                                                                               |

## 11. Testing strategy

### Rust unit (인메모리 / no DB)

- `db::pg_literals` — PG 리터럴 인라인 round-trip (string escape, bytea hex, NULL, numeric, timestamptz, array, jsonb)
- `db::decoder` — OID → Cell 매핑 (mock raw bytes)
- `commands::sqlast::parse_select_target` — 단일 테이블 / JOIN / CTE / `*` 확장
- `commands::editing::build_update / build_insert / build_delete` — Strict / PkOnly SQL 생성

### Rust integration (docker postgres:16-alpine)

- `tests/decoder.rs` — 모든 핵심 타입 insert → fetch → 디코드 → assert
- `tests/editing.rs` — pkOnly / strict 4종 (정상 / 충돌 / NULL / FK 위반)
- `tests/transactions.rs` — tx_begin → execute × 2 → submit → tx_commit ; abort 케이스 ; drop 시 best-effort rollback
- `tests/cancel.rs` — `pg_sleep(10)` → cancel → QueryCancelled + tx aborted 검증
- `tests/history.rs` — 단일 entry / tx 묶음 entry+statements 정확성

### Frontend unit (vitest — Week 3 시작)

- `lib/sql.ts` (기존) + `lib/pgLiterals` mirror
- `store/pendingChanges` — upsert/revert/serialize
- `features/editing/widgets/{Numeric,Json,Bytea,Uuid}` — 검증/생성
- `features/transactions/TxIndicator` — tx state 변화 렌더

### Manual verification

- `docs/superpowers/plans/manual-verification-week-3.md`:
  - 12개 위젯 round-trip
  - Strict vs PkOnly 충돌 시뮬레이션 (`psql` 동시 접속)
  - tx ON에서 INSERT × 2 + 인라인 편집 → COMMIT / ROLLBACK
  - `pg_sleep(30)` → Cancel
  - CSV/JSON/SQL INSERT export round-trip
  - 100k row 결과셋에서 편집 비활성 + 안내
  - 종료 confirm 모달 (tx 활성 + disconnect)
- Week 2 manual verification 재실행 (회귀 가드)

## 12. Folder structure (신규/변경)

```
src/
  features/
    editing/                NEW
      EditableCell.tsx
      PendingBadge.tsx
      PreviewModal.tsx
      ConflictModal.tsx
      widgets/
        Text.tsx Int.tsx Bigint.tsx Numeric.tsx Bool.tsx
        Date.tsx Time.tsx Timestamp.tsx Uuid.tsx Json.tsx
        Bytea.tsx Vector.tsx Enum.tsx Fk.tsx
    transactions/           NEW
      AutoCommitToggle.tsx TxIndicator.tsx TxSidePanel.tsx
    history/                NEW
      HistoryPalette.tsx HistoryEntry.tsx
    export/                 NEW
      ExportDialog.tsx
    results/                (확장)
      ResultsGrid.tsx cells.tsx ContextMenu.tsx
  store/
    pendingChanges.ts       NEW
    transactions.ts         NEW
    history.ts              NEW
    settings.ts             (Strict/PkOnly 토글 추가)
  lib/
    sqlAst.ts               NEW
    pgLiterals.ts           NEW (Rust mirror, 검증용)
    types.ts                (Cell/PendingChange/Tx*/HistoryEntry 추가)

src-tauri/src/
  commands/
    transactions.rs         NEW
    editing.rs              NEW
    history.rs              NEW
    export.rs               NEW
    sqlast.rs               NEW
    cancel.rs               NEW
    query.rs                (응답에 meta + Cell 사용)
  db/
    decoder.rs              NEW
    pg_literals.rs          NEW
    pg_meta.rs              NEW
    pool.rs                 (ActiveConnection.tx_slot)
    state.rs                (migration 002_history)
  errors.rs                 (Editing/Conflict/Tx/...)

infra/postgres/             (기존 docker-compose 그대로)
```

## 13. Implementation slice order (high level)

writing-plans skill에서 더 잘게 쪼개지지만, 의존성 순서:

### Phase 1 — 기반 인프라

1. **Decoder 재작성** — `db/decoder.rs`, `Cell` enum. `<unsupported type>` 폐기. `cells.tsx` 렌더 갱신. golden test.
2. **PG 리터럴 인라인** — `db/pg_literals.rs`. unit test.
3. **SQL AST 파서 + PK/메타 조회** — `commands/sqlast.rs` + `db/pg_meta.rs` + 60s LRU. `execute_query` 응답에 `meta` 추가.
4. **History 마이그레이션 + 단일 entry 기록** — migration 002, `commands/history.rs`. `execute_query`가 entry 기록.

### Phase 2 — 명시적 트랜잭션

5. **Sticky tx 슬롯** — `tx_slot`, `commands/transactions.rs`, drop best-effort rollback.
6. **Tx 모드 UI** — `store/transactions.ts`, AutoCommitToggle/TxIndicator/TxSidePanel, 단축키, 종료 confirm.
7. **History tx 묶음 확장** — tx_begin이 entry 만들고 statement append.

### Phase 3 — 인라인 편집 코어

8. **PendingChanges store + EditableCell 셸** — text fallback만으로 흐름 검증.
9. **위젯 12종** — Text/Int/Bigint/Numeric/Bool/Date/Time/Timestamp/Uuid/Json/Bytea/Vector + nullable Set NULL.
10. **Enum / FK 위젯** — `pg_meta` LRU 공유. FK는 참조 PK + 첫 text 컬럼 검색.
11. **Submit + Preview** — `commands/editing.rs::submit_pending_changes` + `preview_pending_changes`. PkOnly default.
12. **Strict mode + 충돌 감지** — Strict 토글, ConflictModal, Atomic 전체 rollback.
13. **INSERT / DELETE 행** — `+ Row` 버튼, 우클릭 Delete row.

### Phase 4 — 보조 기능

14. **쿼리 취소** — `commands/cancel.rs` + 토스트 Cancel.
15. **Export** — `commands/export.rs` + ExportDialog.
16. **셀 컨텍스트 메뉴** — Copy / Copy as INSERT / Set NULL / Filter by this value.
17. **Cmd+P 명령 팔레트** — `features/history/HistoryPalette.tsx`.

### Phase 5 — 마무리

18. **vitest 셋업 + 핵심 프론트 테스트** — Week 2에서 미뤘던 것 도입.
19. **수동 검증 체크리스트** — `manual-verification-week-3.md`.
20. **회귀 가드** — Week 2 verification 재실행.

**의존성 핵심**:

- 1 (decoder) → 8~13 모두의 전제
- 3 (sqlast + pg_meta) → 8 (편집 진입 가능 여부 게이트)
- 2 (pg_literals) → 11, 15 (Preview / Copy as INSERT / SQL export 공유)
- 5~7 (tx) ↔ 11 (submit 흐름은 sticky tx 위/아래 다 동작해야 함)
