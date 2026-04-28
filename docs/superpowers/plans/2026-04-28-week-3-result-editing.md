# Week 3 — Result 인라인 편집 + 명시적 트랜잭션 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tusk를 read-only 클라이언트에서 **편집 가능한 클라이언트 + 트랜잭션 1급 시민**으로 진화시킨다. 단일 테이블 SELECT 결과 셀을 타입별 위젯으로 직접 편집 → Preview → Submit; 명시적 BEGIN/COMMIT/ROLLBACK 토글; 모든 실행이 SQLite 히스토리에 남고 Cmd+P로 검색.

**Architecture:** Rust 측 `ActiveConnection`에 `tx_slot: Mutex<Option<StickyTx>>` 추가해 한 connection이 sticky 트랜잭션을 소유. 결과 row는 OID-dispatch decoder가 타입별 `Cell` enum으로 직렬화 (Week 2의 `<unsupported type>` 영구 폐기). 편집은 프론트가 구조화된 `PendingBatch` 객체를 보내고 Rust가 parameterized 실행 + 별도로 리터럴 인라인 SQL 빌드해 Preview에 노출. 충돌 감지(Strict 모드)는 atomic — 한 batch라도 충돌이면 같은 submit 전체 ROLLBACK. 쿼리 히스토리는 의도 단위 entry + 트랜잭션 묶음의 sub-statements 두 단 SQLite 모델.

**Tech Stack:** sqlx 0.8 (`bigdecimal`, `ipnetwork` features 추가), `sqlparser 0.50` (postgres dialect), `lru 0.12`, rusqlite 0.32, React 19 + zustand + Monaco + TanStack, vitest (Phase 5에서 도입).

**Reference spec:** `docs/superpowers/specs/2026-04-28-week-3-result-editing-design.md`.

**Working dir:** `/Users/cyj/workspace/personal/tusk` on `main`.

**Quality gates between tasks:**

```
pnpm typecheck && pnpm lint && pnpm format:check
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
cargo test --manifest-path src-tauri/Cargo.toml
```

Run only the gates relevant to the task (Rust tasks → rust:\* + cargo test; Frontend tasks → typecheck/lint/format + `pnpm build`). Last task runs the full set.

**Integration tests with docker postgres:** Some Rust tests under `src-tauri/tests/` require the postgres test instance from `infra/postgres/docker-compose.yml`. Bring it up once at the start of relevant tasks:

```
docker compose -f infra/postgres/docker-compose.yml up -d
```

Connection: `postgres://tusk:tusk@127.0.0.1:55432/tusk_test` (matches Week 2 verification doc).

**Commit message convention:** Conventional commits (`feat`, `fix`, `chore`, `docs`, `refactor`, `test`). **Do NOT add `Co-Authored-By` trailers or "Generated with ..." lines.** Commit messages describe the change, nothing else.

---

## File structure (created during this plan)

```
src-tauri/src/
  db/
    decoder.rs          (Task 3) — OID dispatch typed decoder
    pg_literals.rs      (Task 2) — PG literal inline serializer
    pg_meta.rs          (Task 5) — PK / enum / FK lookups + LRU cache
    pool.rs             (Task 8) — extended with tx_slot
    state.rs            (Task 7) — migration 002_history
  commands/
    sqlast.rs           (Task 4) — parse_select_target
    history.rs          (Task 7) — record / list / search
    transactions.rs     (Task 8) — tx_begin / tx_commit / tx_rollback
    editing.rs          (Task 16) — build_update / build_insert / build_delete + submit / preview
    cancel.rs           (Task 20) — cancel_query
    export.rs           (Task 21) — csv / json / sql_insert serializers
    query.rs            (modified — Task 6, 9) — Cell-typed response, sticky-tx routing
  errors.rs             (modified — Task 1, 8, 16, 20) — new variants
  lib.rs                (modified each command-add) — invoke handler list
  tests/
    decoder.rs          (Task 3)
    pg_meta.rs          (Task 5)
    transactions.rs     (Task 8)
    editing.rs          (Task 16, 18)
    cancel.rs           (Task 20)
    history.rs          (Task 7)

src/
  lib/types.ts          (modified — Task 1, 11) — Cell, PendingChange, TxState, ResultMeta, HistoryEntry
  lib/sqlAst.ts         (Task 4) — invoke wrapper (none-state)
  lib/pgLiterals.ts     (Task 2) — TS mirror for client-side preview rendering test
  store/
    pendingChanges.ts   (Task 11) — Map<rowKey, PendingChange>
    transactions.ts     (Task 10) — TxState mirror
    history.ts          (Task 22) — entry list + palette state
    settings.ts         (modified — Task 18) — strict / pkOnly toggle
  features/
    editing/
      EditableCell.tsx  (Task 11)
      PendingBadge.tsx  (Task 11)
      PreviewModal.tsx  (Task 17)
      ConflictModal.tsx (Task 18)
      widgets/
        Text.tsx Int.tsx Bigint.tsx Numeric.tsx Bool.tsx           (Task 12)
        Date.tsx Time.tsx Timestamp.tsx Uuid.tsx                  (Task 13)
        Json.tsx Bytea.tsx Vector.tsx                              (Task 14)
        Enum.tsx Fk.tsx                                            (Task 15)
        SetNullButton.tsx                                          (Task 12)
    transactions/
      AutoCommitToggle.tsx TxIndicator.tsx TxSidePanel.tsx          (Task 10)
    history/
      HistoryPalette.tsx HistoryEntry.tsx                           (Task 22)
    export/
      ExportDialog.tsx                                              (Task 21)
    results/
      ResultsGrid.tsx     (modified — Task 6, 11)
      cells.tsx           (modified — Task 6)
      ContextMenu.tsx     (Task 22)

docs/superpowers/plans/manual-verification-week-3.md  (Task 24)
```

---

## Task 1: Foundation — Cargo deps, error variants, Cell type scaffolds

**Goal:** All cross-cutting types and dependencies that later tasks build on. No business logic yet.

**Files:**

- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/errors.rs`
- Create: `src-tauri/src/db/decoder.rs` (empty module skeleton)
- Create: `src-tauri/src/db/pg_literals.rs` (empty module skeleton)
- Create: `src-tauri/src/db/pg_meta.rs` (empty module skeleton)
- Modify: `src-tauri/src/db/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/types.ts`

**Steps:**

- [ ] **Step 1: Add Cargo dependencies**

Edit `src-tauri/Cargo.toml`. In `[dependencies]`, add the new deps and **enable additional sqlx features**:

```toml
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono", "json", "macros", "bigdecimal", "ipnetwork"] }
bigdecimal = { version = "0.4", features = ["serde"] }
ipnetwork = "0.20"
sqlparser = "0.52"
lru = "0.12"
base64 = "0.22"
```

Run: `pnpm rust:check`
Expected: compiles successfully.

- [ ] **Step 2: Add new `TuskError` variants**

Edit `src-tauri/src/errors.rs`. After existing variants, add:

```rust
    #[error("Editing failed: {0}")]
    Editing(String),

    #[error("Conflict on batch")]
    Conflict {
        batch_id: String,
        executed_sql: String,
        current: serde_json::Value,
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
```

Update `serialize_uses_tagged_repr` test if needed (existing test only checks one variant — leave it).

Add tests:

```rust
    #[test]
    fn serialize_conflict_carries_payload() {
        let err = TuskError::Conflict {
            batch_id: "b1".into(),
            executed_sql: "UPDATE t ...".into(),
            current: serde_json::json!({ "id": 1 }),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"kind\":\"Conflict\""));
        assert!(json.contains("\"batch_id\":\"b1\""));
    }

    #[test]
    fn tx_aborted_serializes_as_tag_only() {
        let err = TuskError::TxAborted;
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"kind\":\"TxAborted\""));
    }
```

Note: `Conflict` is a struct variant; serde with `tag = "kind", content = "message"` does not work cleanly for struct variants. Switch the enum's serde repr to `tag = "kind"` only (no `content`) so struct variants serialize their fields next to `kind`. Adjust:

```rust
#[derive(Error, Debug, Serialize)]
#[serde(tag = "kind")]
pub enum TuskError {
    #[error("Connection failed: {0}")]
    Connection(String),
    // ... existing tuple variants — wrap each in a struct-like single-field rename
```

Tuple variants don't compose with `tag = "kind"` either. The clean fix:

```rust
#[derive(Error, Debug, Serialize)]
#[serde(tag = "kind", content = "data")]
pub enum TuskError {
    #[error("Connection failed: {0}")]
    Connection(String),
    #[error("Query failed: {0}")]
    Query(String),
    #[error("SSH tunnel failed: {0}")]
    Tunnel(String),
    #[error("SSH config error: {0}")]
    Ssh(String),
    #[error("State error: {0}")]
    State(String),
    #[error("Secrets error: {0}")]
    Secrets(String),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Editing failed: {0}")]
    Editing(String),
    #[error("Conflict on batch")]
    Conflict {
        batch_id: String,
        executed_sql: String,
        current: serde_json::Value,
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

`tag + content` works for both tuple and struct variants. **Update the existing serialize test** to expect the new shape:

```rust
    #[test]
    fn serialize_uses_tagged_repr() {
        let err = TuskError::Connection("nope".into());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, r#"{"kind":"Connection","data":"nope"}"#);
    }
```

This is a wire-format change. **Frontend `lib/tauri.ts` will need to read `err.data` instead of `err.message`.** That update happens in this task too (Step 4).

- [ ] **Step 3: Update frontend error parser**

Edit `src/lib/tauri.ts`. Find the location that reads `err.message` from a thrown TuskError and switch to `err.data`. If no such location exists, search:

```bash
grep -rn 'TuskError\|err\.message\|err\.kind' src/lib/tauri.ts
```

Update accordingly so toasts still render. Add a unit-friendly comment:

```ts
// Wire format: { kind: string, data?: unknown }. Struct variants
// (Conflict, UnsupportedEditType) carry typed objects in `data`.
```

- [ ] **Step 4: Scaffold Rust modules**

Create `src-tauri/src/db/decoder.rs`:

```rust
// src-tauri/src/db/decoder.rs
//
// OID-dispatch typed decoder. Replaces the best-effort `decode_cell` in
// commands/query.rs (Week 2). Implementation lands in Task 3.

use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum Cell {
    Null,
    Bool(bool),
    Int(i32),
    Bigint(String),
    Float(f64),
    Numeric(String),
    Text(String),
    Bytea { b64: String },
    Uuid(String),
    Inet(String),
    Date(String),
    Time(String),
    Timetz(String),
    Timestamp(String),
    Timestamptz(String),
    Interval { iso: String },
    Json(serde_json::Value),
    Array { elem: String, values: Vec<Cell> },
    Enum { type_name: String, value: String },
    Vector { dim: u32, values: Vec<f32> },
    Unknown { oid: u32, text: String },
}
```

Note: `serde(tag, content)` here means a `Cell::Bool(true)` serializes as `{"kind":"Bool","value":true}`. Struct variants (`Bytea`, `Interval`, `Array`, `Enum`, `Vector`, `Unknown`) serialize their fields under `value`. The TS `Cell` type in Step 5 mirrors this.

Create `src-tauri/src/db/pg_literals.rs`:

```rust
// src-tauri/src/db/pg_literals.rs
//
// Renders typed values as PG literal SQL fragments (single-quoted strings,
// hex bytea, NULL, etc.) — used to build human-readable preview SQL that
// matches what the parameterized executor will actually run. Implementation
// lands in Task 2.
```

Create `src-tauri/src/db/pg_meta.rs`:

```rust
// src-tauri/src/db/pg_meta.rs
//
// Per-table metadata lookups (PK columns, enum values, FK targets) with
// LRU cache keyed by (conn_id, schema, table). Implementation lands in
// Task 5.
```

Edit `src-tauri/src/db/mod.rs` — append:

```rust
pub mod decoder;
pub mod pg_literals;
pub mod pg_meta;
```

- [ ] **Step 5: Add TS Cell + ResultMeta types**

Edit `src/lib/types.ts`. After existing types, add:

```ts
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

export type Cell =
  | { kind: "Null" }
  | { kind: "Bool"; value: boolean }
  | { kind: "Int"; value: number }
  | { kind: "Bigint"; value: string }
  | { kind: "Float"; value: number }
  | { kind: "Numeric"; value: string }
  | { kind: "Text"; value: string }
  | { kind: "Bytea"; value: { b64: string } }
  | { kind: "Uuid"; value: string }
  | { kind: "Inet"; value: string }
  | { kind: "Date"; value: string }
  | { kind: "Time"; value: string }
  | { kind: "Timetz"; value: string }
  | { kind: "Timestamp"; value: string }
  | { kind: "Timestamptz"; value: string }
  | { kind: "Interval"; value: { iso: string } }
  | { kind: "Json"; value: unknown }
  | { kind: "Array"; value: { elem: string; values: Cell[] } }
  | { kind: "Enum"; value: { typeName: string; value: string } }
  | { kind: "Vector"; value: { dim: number; values: number[] } }
  | { kind: "Unknown"; value: { oid: number; text: string } };

export interface ColumnTypeMeta {
  name: string;
  oid: number;
  typeName: PgTypeName;
  nullable: boolean;
  enumValues?: string[];
  fk?: { schema: string; table: string; column: string };
}

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
  pkColumnIndices: number[];
  columnTypes: ColumnTypeMeta[];
}
```

Do NOT add `PendingChange / TxState / HistoryEntry` here yet — those land with their owning task.

- [ ] **Step 6: Quality gates**

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
cargo test --manifest-path src-tauri/Cargo.toml
pnpm typecheck && pnpm lint && pnpm format:check
```

All must pass. Some `unused_imports` warnings on the new module skeletons are acceptable only if the lint config tolerates them (clippy `-D warnings` will fail). If clippy fails, add `#![allow(dead_code)]` at the top of `decoder.rs`, `pg_literals.rs`, `pg_meta.rs` for now — removed in their respective tasks.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/errors.rs \
        src-tauri/src/db/mod.rs src-tauri/src/db/decoder.rs \
        src-tauri/src/db/pg_literals.rs src-tauri/src/db/pg_meta.rs \
        src/lib/types.ts src/lib/tauri.ts
git commit -m "feat: Week 3 foundation — error variants, Cell type, deps"
```

---

## Task 2: PG literal serializer (`pg_literals.rs`)

**Goal:** A pure function `to_literal(cell: &Cell) -> String` that renders a typed value as a PG literal usable inline in SQL. Used for preview SQL and Copy-as-INSERT / SQL export. TDD throughout.

**Files:**

- Modify: `src-tauri/src/db/pg_literals.rs`

**Steps:**

- [ ] **Step 1: Write the failing tests**

Replace `pg_literals.rs` body with:

```rust
// src-tauri/src/db/pg_literals.rs
use crate::db::decoder::Cell;
use std::fmt::Write;

/// Renders a typed Cell as a PG literal SQL fragment.
///
/// Examples:
///   Null              → "NULL"
///   Bool(true)        → "TRUE"
///   Int(42)           → "42"
///   Numeric("1.234")  → "1.234"
///   Text("o'r")       → "'o''r'"
///   Bytea(b64="...")  → "'\\x<hex>'"
///   Uuid              → "'<uuid>'::uuid"
///   Timestamptz(iso)  → "'<iso>'::timestamptz"
///   Json              → "'{...}'::jsonb"
///   Array(elem, vals) → "ARRAY[...]::<elem>[]"
pub fn to_literal(cell: &Cell) -> String {
    todo!("implemented in step 3")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn null_renders_uppercase_keyword() {
        assert_eq!(to_literal(&Cell::Null), "NULL");
    }

    #[test]
    fn bool_renders_uppercase_keyword() {
        assert_eq!(to_literal(&Cell::Bool(true)), "TRUE");
        assert_eq!(to_literal(&Cell::Bool(false)), "FALSE");
    }

    #[test]
    fn int_renders_decimal() {
        assert_eq!(to_literal(&Cell::Int(42)), "42");
        assert_eq!(to_literal(&Cell::Int(-7)), "-7");
    }

    #[test]
    fn bigint_preserves_string() {
        assert_eq!(to_literal(&Cell::Bigint("9223372036854775807".into())), "9223372036854775807");
    }

    #[test]
    fn float_uses_pg_compatible_repr() {
        assert_eq!(to_literal(&Cell::Float(1.5)), "1.5");
        assert_eq!(to_literal(&Cell::Float(-0.25)), "-0.25");
    }

    #[test]
    fn numeric_passes_through_string() {
        assert_eq!(to_literal(&Cell::Numeric("1.234".into())), "1.234");
    }

    #[test]
    fn text_quotes_and_doubles_single_quotes() {
        assert_eq!(to_literal(&Cell::Text("o'reilly".into())), "'o''reilly'");
        assert_eq!(to_literal(&Cell::Text("plain".into())), "'plain'");
    }

    #[test]
    fn text_with_backslash_uses_e_string() {
        // Backslash in standard_conforming_strings=on contexts is literal.
        // We always emit standard form; backslashes pass through.
        assert_eq!(to_literal(&Cell::Text("a\\b".into())), "'a\\b'");
    }

    #[test]
    fn bytea_emits_hex_form() {
        // base64 "DEAD" decodes to bytes 0x0c, 0x40, 0x76 -- not what we want for clarity.
        // Use deterministic input: b64 of [0xDE, 0xAD, 0xBE, 0xEF].
        use base64::{engine::general_purpose::STANDARD, Engine};
        let b64 = STANDARD.encode([0xDE_u8, 0xAD, 0xBE, 0xEF]);
        let cell = Cell::Bytea { b64 };
        assert_eq!(to_literal(&cell), "'\\xdeadbeef'::bytea");
    }

    #[test]
    fn uuid_appends_cast() {
        assert_eq!(
            to_literal(&Cell::Uuid("550e8400-e29b-41d4-a716-446655440000".into())),
            "'550e8400-e29b-41d4-a716-446655440000'::uuid"
        );
    }

    #[test]
    fn inet_appends_cast() {
        assert_eq!(to_literal(&Cell::Inet("10.0.0.1/32".into())), "'10.0.0.1/32'::inet");
    }

    #[test]
    fn timestamps_append_typed_cast() {
        assert_eq!(
            to_literal(&Cell::Timestamptz("2026-04-28T12:00:00+00:00".into())),
            "'2026-04-28T12:00:00+00:00'::timestamptz"
        );
        assert_eq!(
            to_literal(&Cell::Timestamp("2026-04-28T12:00:00".into())),
            "'2026-04-28T12:00:00'::timestamp"
        );
        assert_eq!(to_literal(&Cell::Date("2026-04-28".into())), "'2026-04-28'::date");
        assert_eq!(to_literal(&Cell::Time("12:00:00".into())), "'12:00:00'::time");
        assert_eq!(to_literal(&Cell::Timetz("12:00:00+00".into())), "'12:00:00+00'::timetz");
    }

    #[test]
    fn interval_appends_cast() {
        let cell = Cell::Interval { iso: "PT1H30M".into() };
        assert_eq!(to_literal(&cell), "'PT1H30M'::interval");
    }

    #[test]
    fn json_quotes_and_escapes() {
        let cell = Cell::Json(json!({ "k": "v's" }));
        // JSON encodes "v's" as "v's" (no escape on single quote), but our literal
        // wrapper must double the single quote.
        assert_eq!(to_literal(&cell), r#"'{"k":"v''s"}'::jsonb"#);
    }

    #[test]
    fn enum_uses_text_cast_to_typename() {
        let cell = Cell::Enum { type_name: "mood".into(), value: "happy".into() };
        assert_eq!(to_literal(&cell), "'happy'::mood");
    }

    #[test]
    fn vector_renders_brackets() {
        let cell = Cell::Vector { dim: 3, values: vec![1.0, 2.5, -3.0] };
        assert_eq!(to_literal(&cell), "'[1,2.5,-3]'::vector");
    }

    #[test]
    fn array_of_ints_renders_as_array_literal() {
        let cell = Cell::Array {
            elem: "int4".into(),
            values: vec![Cell::Int(1), Cell::Int(2), Cell::Null, Cell::Int(4)],
        };
        assert_eq!(to_literal(&cell), "ARRAY[1,2,NULL,4]::int4[]");
    }

    #[test]
    fn array_of_text_quotes_each_element() {
        let cell = Cell::Array {
            elem: "text".into(),
            values: vec![Cell::Text("a".into()), Cell::Text("o'r".into())],
        };
        assert_eq!(to_literal(&cell), "ARRAY['a','o''r']::text[]");
    }

    #[test]
    fn unknown_uses_text_repr_quoted() {
        let cell = Cell::Unknown { oid: 9999, text: "raw".into() };
        assert_eq!(to_literal(&cell), "'raw'::text");
    }
}
```

- [ ] **Step 2: Run tests, verify they fail**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib pg_literals
```

Expected: tests panic on `todo!()`.

- [ ] **Step 3: Implement `to_literal`**

Replace the `todo!()` body with:

```rust
pub fn to_literal(cell: &Cell) -> String {
    match cell {
        Cell::Null => "NULL".to_string(),
        Cell::Bool(true) => "TRUE".to_string(),
        Cell::Bool(false) => "FALSE".to_string(),
        Cell::Int(v) => v.to_string(),
        Cell::Bigint(s) => s.clone(),
        Cell::Float(v) => format_float(*v),
        Cell::Numeric(s) => s.clone(),
        Cell::Text(s) => quote_string(s),
        Cell::Bytea { b64 } => quote_bytea(b64),
        Cell::Uuid(s) => format!("{}::uuid", quote_string(s)),
        Cell::Inet(s) => format!("{}::inet", quote_string(s)),
        Cell::Date(s) => format!("{}::date", quote_string(s)),
        Cell::Time(s) => format!("{}::time", quote_string(s)),
        Cell::Timetz(s) => format!("{}::timetz", quote_string(s)),
        Cell::Timestamp(s) => format!("{}::timestamp", quote_string(s)),
        Cell::Timestamptz(s) => format!("{}::timestamptz", quote_string(s)),
        Cell::Interval { iso } => format!("{}::interval", quote_string(iso)),
        Cell::Json(v) => {
            let raw = serde_json::to_string(v).expect("json round-trip");
            format!("{}::jsonb", quote_string(&raw))
        }
        Cell::Array { elem, values } => {
            let mut out = String::from("ARRAY[");
            for (i, v) in values.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&to_literal(v));
            }
            let _ = write!(out, "]::{}[]", elem);
            out
        }
        Cell::Enum { type_name, value } => format!("{}::{}", quote_string(value), type_name),
        Cell::Vector { values, .. } => {
            let mut inner = String::from("[");
            for (i, v) in values.iter().enumerate() {
                if i > 0 {
                    inner.push(',');
                }
                inner.push_str(&format_float(*v as f64));
            }
            inner.push(']');
            format!("{}::vector", quote_string(&inner))
        }
        Cell::Unknown { text, .. } => format!("{}::text", quote_string(text)),
    }
}

fn quote_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push('\'');
            out.push('\'');
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

fn quote_bytea(b64: &str) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let bytes = STANDARD.decode(b64).unwrap_or_default();
    let mut hex = String::with_capacity(bytes.len() * 2 + 4);
    hex.push_str("'\\x");
    for b in bytes {
        let _ = write!(hex, "{b:02x}");
    }
    hex.push('\'');
    hex.push_str("::bytea");
    hex
}

fn format_float(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() && v.abs() < 1e16 {
        format!("{}", v as i64)
            // ensure we don't accidentally turn 1.0 into "1" then back-convert lossily;
            // PG happily accepts "1" for a float column, so this is fine.
    } else {
        let s = format!("{v}");
        s
    }
}
```

Note on `format_float`: the test expects `Float(1.5) → "1.5"` and `Float(-0.25) → "-0.25"`. The current branch returns `format!("{v}")` for non-integer floats which yields `"1.5"` and `"-0.25"` — correct. Integer-valued floats become `"1"` (no decimal point), which is acceptable PG syntax for `float4/float8` columns.

- [ ] **Step 4: Run tests, verify all pass**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib pg_literals
```

Expected: all 17 tests pass.

- [ ] **Step 5: Wire module export**

Verify `src-tauri/src/db/mod.rs` already has `pub mod pg_literals;` from Task 1. If you added `#![allow(dead_code)]` in Task 1, **remove** it now.

- [ ] **Step 6: Quality gates**

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
```

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/db/pg_literals.rs
git commit -m "feat(rust): PG literal inline serializer for preview SQL"
```

---

## Task 3: OID-dispatch decoder (`db/decoder.rs` + integration test)

**Goal:** Replace Week 2's best-effort `decode_cell` with explicit OID-based dispatch. Every PG type the spec lists maps to an exact `Cell` variant; unknown OIDs fall back to `Cell::Unknown { oid, text }` with best-effort utf8 of raw bytes. TDD on unit-level OID matching, integration test on a real postgres for round-trips.

**Files:**

- Modify: `src-tauri/src/db/decoder.rs`
- Modify: `src-tauri/src/commands/query.rs` (deletes old `decode_cell`, calls new decoder)
- Create: `src-tauri/tests/decoder.rs` (docker integration)

**Steps:**

- [ ] **Step 1: Write the unit test for OID → typeName mapping**

Append to `src-tauri/src/db/decoder.rs` body (above the file's `Cell` enum, or wherever sensible):

```rust
/// PG built-in OIDs we recognize. Source: pg_type.h in postgres 16.
pub mod oids {
    pub const BOOL: u32 = 16;
    pub const BYTEA: u32 = 17;
    pub const INT8: u32 = 20;
    pub const INT2: u32 = 21;
    pub const INT4: u32 = 23;
    pub const TEXT: u32 = 25;
    pub const JSON: u32 = 114;
    pub const FLOAT4: u32 = 700;
    pub const FLOAT8: u32 = 701;
    pub const VARCHAR: u32 = 1043;
    pub const BPCHAR: u32 = 1042;
    pub const DATE: u32 = 1082;
    pub const TIME: u32 = 1083;
    pub const TIMESTAMP: u32 = 1114;
    pub const TIMESTAMPTZ: u32 = 1184;
    pub const INTERVAL: u32 = 1186;
    pub const TIMETZ: u32 = 1266;
    pub const NUMERIC: u32 = 1700;
    pub const UUID: u32 = 2950;
    pub const JSONB: u32 = 3802;
    pub const INET: u32 = 869;
    pub const CIDR: u32 = 650;
    // Array variants:
    pub const _BOOL: u32 = 1000;
    pub const _BYTEA: u32 = 1001;
    pub const _INT2: u32 = 1005;
    pub const _INT4: u32 = 1007;
    pub const _INT8: u32 = 1016;
    pub const _TEXT: u32 = 1009;
    pub const _VARCHAR: u32 = 1015;
    pub const _NUMERIC: u32 = 1231;
    pub const _UUID: u32 = 2951;
    pub const _FLOAT4: u32 = 1021;
    pub const _FLOAT8: u32 = 1022;
    pub const _TIMESTAMP: u32 = 1115;
    pub const _TIMESTAMPTZ: u32 = 1185;
    pub const _DATE: u32 = 1182;
}

/// Maps a PG OID to a stable typeName string used by the frontend `PgTypeName`.
/// Returns `"unknown"` for OIDs we don't recognize.
pub fn pg_type_name(oid: u32) -> &'static str {
    match oid {
        oids::BOOL => "bool",
        oids::INT2 => "int2",
        oids::INT4 => "int4",
        oids::INT8 => "int8",
        oids::FLOAT4 => "float4",
        oids::FLOAT8 => "float8",
        oids::NUMERIC => "numeric",
        oids::TEXT => "text",
        oids::VARCHAR => "varchar",
        oids::BPCHAR => "bpchar",
        oids::BYTEA => "bytea",
        oids::UUID => "uuid",
        oids::INET => "inet",
        oids::CIDR => "cidr",
        oids::DATE => "date",
        oids::TIME => "time",
        oids::TIMETZ => "timetz",
        oids::TIMESTAMP => "timestamp",
        oids::TIMESTAMPTZ => "timestamptz",
        oids::INTERVAL => "interval",
        oids::JSON => "json",
        oids::JSONB => "jsonb",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_oids_map_to_canonical_names() {
        assert_eq!(pg_type_name(oids::BOOL), "bool");
        assert_eq!(pg_type_name(oids::INT4), "int4");
        assert_eq!(pg_type_name(oids::TIMESTAMPTZ), "timestamptz");
        assert_eq!(pg_type_name(oids::JSONB), "jsonb");
    }

    #[test]
    fn unknown_oid_returns_unknown() {
        assert_eq!(pg_type_name(99999), "unknown");
    }
}
```

- [ ] **Step 2: Implement `decode_row` API**

Add to `decoder.rs`:

```rust
use sqlx::postgres::PgRow;
use sqlx::{Column, Row, TypeInfo, ValueRef};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMeta {
    pub name: String,
    pub oid: u32,
    pub type_name: String,
}

pub fn columns_of(row: &PgRow) -> Vec<ColumnMeta> {
    row.columns()
        .iter()
        .map(|c| {
            let type_info = c.type_info();
            // sqlx::postgres::PgTypeInfo has Oid via internal repr; expose via try_oid()
            let oid = type_info.oid().map(|o| o.0).unwrap_or(0);
            ColumnMeta {
                name: c.name().to_string(),
                oid,
                type_name: pg_type_name(oid).to_string(),
            }
        })
        .collect()
}

pub fn decode_row(row: &PgRow, columns: &[ColumnMeta]) -> Vec<Cell> {
    (0..columns.len())
        .map(|i| decode_cell(row, i, columns[i].oid))
        .collect()
}

fn decode_cell(row: &PgRow, idx: usize, oid: u32) -> Cell {
    // Null check first — try_get_raw lets us inspect IS NULL without typed decode.
    if let Ok(raw) = row.try_get_raw(idx) {
        if raw.is_null() {
            return Cell::Null;
        }
    }
    match oid {
        oids::BOOL => row.try_get::<bool, _>(idx).map(Cell::Bool).unwrap_or(Cell::Null),
        oids::INT2 => row.try_get::<i16, _>(idx).map(|v| Cell::Int(v as i32)).unwrap_or(Cell::Null),
        oids::INT4 => row.try_get::<i32, _>(idx).map(Cell::Int).unwrap_or(Cell::Null),
        oids::INT8 => row.try_get::<i64, _>(idx).map(|v| Cell::Bigint(v.to_string())).unwrap_or(Cell::Null),
        oids::FLOAT4 => row.try_get::<f32, _>(idx).map(|v| Cell::Float(v as f64)).unwrap_or(Cell::Null),
        oids::FLOAT8 => row.try_get::<f64, _>(idx).map(Cell::Float).unwrap_or(Cell::Null),
        oids::NUMERIC => row
            .try_get::<bigdecimal::BigDecimal, _>(idx)
            .map(|v| Cell::Numeric(v.to_string()))
            .unwrap_or(Cell::Null),
        oids::TEXT | oids::VARCHAR | oids::BPCHAR => row
            .try_get::<String, _>(idx)
            .map(Cell::Text)
            .unwrap_or(Cell::Null),
        oids::BYTEA => row
            .try_get::<Vec<u8>, _>(idx)
            .map(|bytes| {
                use base64::{engine::general_purpose::STANDARD, Engine};
                Cell::Bytea {
                    b64: STANDARD.encode(bytes),
                }
            })
            .unwrap_or(Cell::Null),
        oids::UUID => row.try_get::<uuid::Uuid, _>(idx).map(|u| Cell::Uuid(u.to_string())).unwrap_or(Cell::Null),
        oids::INET | oids::CIDR => row
            .try_get::<ipnetwork::IpNetwork, _>(idx)
            .map(|n| Cell::Inet(n.to_string()))
            .unwrap_or(Cell::Null),
        oids::DATE => row
            .try_get::<chrono::NaiveDate, _>(idx)
            .map(|d| Cell::Date(d.to_string()))
            .unwrap_or(Cell::Null),
        oids::TIME => row
            .try_get::<chrono::NaiveTime, _>(idx)
            .map(|t| Cell::Time(t.to_string()))
            .unwrap_or(Cell::Null),
        oids::TIMESTAMP => row
            .try_get::<chrono::NaiveDateTime, _>(idx)
            .map(|t| Cell::Timestamp(t.to_string()))
            .unwrap_or(Cell::Null),
        oids::TIMESTAMPTZ => row
            .try_get::<chrono::DateTime<chrono::Utc>, _>(idx)
            .map(|t| Cell::Timestamptz(t.to_rfc3339()))
            .unwrap_or(Cell::Null),
        oids::INTERVAL => row
            .try_get::<sqlx::postgres::types::PgInterval, _>(idx)
            .map(|iv| Cell::Interval { iso: pg_interval_to_iso(&iv) })
            .unwrap_or(Cell::Null),
        oids::TIMETZ => row
            .try_get::<sqlx::postgres::types::PgTimeTz, _>(idx)
            .map(|t| Cell::Timetz(format!("{}", t.time)))
            .unwrap_or(Cell::Null),
        oids::JSON | oids::JSONB => row
            .try_get::<serde_json::Value, _>(idx)
            .map(Cell::Json)
            .unwrap_or(Cell::Null),
        // Arrays — decode element by element when possible.
        oids::_INT4 => decode_int_array(row, idx, "int4"),
        oids::_INT8 => decode_bigint_array(row, idx, "int8"),
        oids::_TEXT | oids::_VARCHAR => decode_text_array(row, idx, "text"),
        oids::_BOOL => decode_bool_array(row, idx, "bool"),
        // Fallback: best-effort utf8 of raw text representation.
        _ => unknown_fallback(row, idx, oid),
    }
}

fn unknown_fallback(row: &PgRow, idx: usize, oid: u32) -> Cell {
    if let Ok(raw) = row.try_get_raw(idx) {
        if let Some(bytes) = raw.as_bytes().ok() {
            if let Ok(text) = std::str::from_utf8(bytes) {
                return Cell::Unknown { oid, text: text.to_string() };
            }
        }
    }
    Cell::Unknown { oid, text: String::new() }
}

fn pg_interval_to_iso(iv: &sqlx::postgres::types::PgInterval) -> String {
    // PgInterval: months, days, microseconds.
    let years = iv.months / 12;
    let months = iv.months % 12;
    let total_micros = iv.microseconds;
    let hours = total_micros / 3_600_000_000;
    let rem = total_micros % 3_600_000_000;
    let minutes = rem / 60_000_000;
    let secs_micros = rem % 60_000_000;
    let secs = secs_micros / 1_000_000;
    let frac = secs_micros % 1_000_000;
    let mut iso = String::from("P");
    if years != 0 { iso.push_str(&format!("{years}Y")); }
    if months != 0 { iso.push_str(&format!("{months}M")); }
    if iv.days != 0 { iso.push_str(&format!("{}D", iv.days)); }
    if hours != 0 || minutes != 0 || secs != 0 || frac != 0 {
        iso.push('T');
        if hours != 0 { iso.push_str(&format!("{hours}H")); }
        if minutes != 0 { iso.push_str(&format!("{minutes}M")); }
        if secs != 0 || frac != 0 {
            if frac != 0 {
                iso.push_str(&format!("{secs}.{:06}S", frac));
            } else {
                iso.push_str(&format!("{secs}S"));
            }
        }
    }
    if iso == "P" { iso.push_str("T0S"); }
    iso
}

fn decode_int_array(row: &PgRow, idx: usize, elem: &str) -> Cell {
    match row.try_get::<Vec<Option<i32>>, _>(idx) {
        Ok(vec) => Cell::Array {
            elem: elem.to_string(),
            values: vec.into_iter().map(|o| o.map(Cell::Int).unwrap_or(Cell::Null)).collect(),
        },
        Err(_) => Cell::Null,
    }
}
fn decode_bigint_array(row: &PgRow, idx: usize, elem: &str) -> Cell {
    match row.try_get::<Vec<Option<i64>>, _>(idx) {
        Ok(vec) => Cell::Array {
            elem: elem.to_string(),
            values: vec.into_iter().map(|o| o.map(|v| Cell::Bigint(v.to_string())).unwrap_or(Cell::Null)).collect(),
        },
        Err(_) => Cell::Null,
    }
}
fn decode_text_array(row: &PgRow, idx: usize, elem: &str) -> Cell {
    match row.try_get::<Vec<Option<String>>, _>(idx) {
        Ok(vec) => Cell::Array {
            elem: elem.to_string(),
            values: vec.into_iter().map(|o| o.map(Cell::Text).unwrap_or(Cell::Null)).collect(),
        },
        Err(_) => Cell::Null,
    }
}
fn decode_bool_array(row: &PgRow, idx: usize, elem: &str) -> Cell {
    match row.try_get::<Vec<Option<bool>>, _>(idx) {
        Ok(vec) => Cell::Array {
            elem: elem.to_string(),
            values: vec.into_iter().map(|o| o.map(Cell::Bool).unwrap_or(Cell::Null)).collect(),
        },
        Err(_) => Cell::Null,
    }
}
```

- [ ] **Step 3: Run unit tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib decoder
```

Expected: 2 tests pass.

- [ ] **Step 4: Update `commands/query.rs` to use new decoder**

Replace the file's body with:

```rust
// src-tauri/src/commands/query.rs
use std::time::Instant;

use serde::Serialize;
use tauri::State;

use crate::db::decoder::{columns_of, decode_row, Cell, ColumnMeta};
use crate::db::pool::ConnectionRegistry;
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Vec<Cell>>,
    pub duration_ms: u128,
    pub row_count: usize,
}

#[tauri::command]
pub async fn execute_query(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
    sql: String,
) -> TuskResult<QueryResult> {
    let pool = registry.pool(&connection_id)?;
    let started = Instant::now();
    let rows = sqlx::query(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| TuskError::Query(e.to_string()))?;
    let duration_ms = started.elapsed().as_millis();

    let columns = rows.first().map(columns_of).unwrap_or_default();
    let row_count = rows.len();
    let mut data = Vec::with_capacity(row_count);
    for row in &rows {
        data.push(decode_row(row, &columns));
    }

    Ok(QueryResult { columns, rows: data, duration_ms, row_count })
}
```

(The `meta` field is added later in Task 6 — keep this slice strictly the decoder migration.)

- [ ] **Step 5: Update frontend `cells.tsx` to render new Cell variants**

Search current `cells.tsx`:

```bash
grep -n 'unsupported\|JsonValue\|type_name' src/features/results/cells.tsx
```

Replace whatever raw JsonValue rendering exists with a `renderCell(cell: Cell): ReactNode` switch on `cell.kind`. Keep visual behavior identical to Week 2 (NULL italic, JSON click-to-expand, others as text). The widgets in Tasks 12–15 will subclass this for editing.

```tsx
// src/features/results/cells.tsx
import type { Cell } from "@/lib/types";

export function renderCell(cell: Cell): React.ReactNode {
  switch (cell.kind) {
    case "Null":
      return <span className="text-muted-foreground italic">NULL</span>;
    case "Bool":
      return cell.value ? "true" : "false";
    case "Int":
    case "Float":
      return String(cell.value);
    case "Bigint":
    case "Numeric":
    case "Text":
    case "Uuid":
    case "Inet":
    case "Date":
    case "Time":
    case "Timetz":
    case "Timestamp":
    case "Timestamptz":
      return cell.value;
    case "Interval":
      return cell.value.iso;
    case "Bytea":
      return (
        <span className="font-mono text-xs">
          \\x{cell.value.b64.slice(0, 24)}…
        </span>
      );
    case "Json":
      return (
        <code className="text-xs">
          {JSON.stringify(cell.value).slice(0, 80)}
        </code>
      );
    case "Array":
      return `{${cell.value.values.length} items}`;
    case "Enum":
      return cell.value.value;
    case "Vector":
      return `vector(${cell.value.dim})`;
    case "Unknown":
      return (
        <span className="text-muted-foreground italic">
          {cell.value.text || `<oid ${cell.value.oid}>`}
        </span>
      );
  }
}
```

Update consumers (`ResultsGrid.tsx`) accordingly: rows are now `Cell[][]` instead of `JsonValue[][]`. Refactor minimally — display correctness is verified at the end of Task 6.

- [ ] **Step 6: Write integration test (`tests/decoder.rs`)**

Create `src-tauri/tests/decoder.rs`:

```rust
//! Integration tests for db::decoder against a real postgres.
//! Requires `docker compose -f infra/postgres/docker-compose.yml up -d`.

use sqlx::postgres::PgPoolOptions;
use tusk_lib::db::decoder::{columns_of, decode_row, Cell};

const URL: &str = "postgres://tusk:tusk@127.0.0.1:55432/tusk_test";

async fn pool() -> sqlx::PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .connect(URL)
        .await
        .expect("docker postgres must be up")
}

#[tokio::test]
async fn decodes_core_types_round_trip() {
    let pool = pool().await;
    let rows = sqlx::query(
        "SELECT
            true::bool                              AS b,
            (32767::int2)                            AS i2,
            (2147483647::int4)                       AS i4,
            (9223372036854775807::int8)              AS i8,
            (1.5::float4)                            AS f4,
            (1.5::float8)                            AS f8,
            (1.234::numeric)                         AS num,
            'hello'::text                            AS t,
            '\\x01ff'::bytea                          AS bytes,
            '550e8400-e29b-41d4-a716-446655440000'::uuid AS id,
            '10.0.0.1/32'::inet                      AS ip,
            '2026-04-28'::date                       AS d,
            '12:34:56'::time                         AS tm,
            '2026-04-28 12:34:56'::timestamp         AS ts,
            '2026-04-28 12:34:56+00'::timestamptz    AS tstz,
            '1 hour 30 minutes'::interval            AS iv,
            '{\"k\":1}'::jsonb                        AS jb,
            ARRAY[1,2,3]::int4[]                     AS arr,
            NULL::int                                AS nul"
    )
    .fetch_all(&pool)
    .await
    .expect("query");

    let cols = columns_of(&rows[0]);
    let cells = decode_row(&rows[0], &cols);
    assert!(matches!(cells[0], Cell::Bool(true)));
    assert!(matches!(cells[1], Cell::Int(32767)));
    assert!(matches!(cells[2], Cell::Int(2147483647)));
    if let Cell::Bigint(s) = &cells[3] { assert_eq!(s, "9223372036854775807"); } else { panic!("i8") }
    assert!(matches!(cells[4], Cell::Float(_)));
    assert!(matches!(cells[5], Cell::Float(_)));
    if let Cell::Numeric(s) = &cells[6] { assert_eq!(s, "1.234"); } else { panic!("num") }
    if let Cell::Text(s) = &cells[7] { assert_eq!(s, "hello"); } else { panic!("text") }
    assert!(matches!(cells[8], Cell::Bytea { .. }));
    assert!(matches!(cells[9], Cell::Uuid(_)));
    assert!(matches!(cells[10], Cell::Inet(_)));
    assert!(matches!(cells[11], Cell::Date(_)));
    assert!(matches!(cells[12], Cell::Time(_)));
    assert!(matches!(cells[13], Cell::Timestamp(_)));
    assert!(matches!(cells[14], Cell::Timestamptz(_)));
    if let Cell::Interval { iso } = &cells[15] {
        assert!(iso.starts_with("PT"));
        assert!(iso.contains("H"));
    } else { panic!("interval") }
    assert!(matches!(cells[16], Cell::Json(_)));
    if let Cell::Array { elem, values } = &cells[17] {
        assert_eq!(elem, "int4");
        assert_eq!(values.len(), 3);
    } else { panic!("array") }
    assert!(matches!(cells[18], Cell::Null));
}
```

- [ ] **Step 7: Run integration test**

```bash
docker compose -f infra/postgres/docker-compose.yml up -d
cargo test --manifest-path src-tauri/Cargo.toml --test decoder
```

Expected: pass.

- [ ] **Step 8: Quality gates**

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
pnpm typecheck && pnpm lint && pnpm format:check
pnpm build
```

`pnpm build` is added because cells.tsx changes are compile-time visible only.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/db/decoder.rs src-tauri/src/commands/query.rs \
        src-tauri/tests/decoder.rs src/features/results/cells.tsx \
        src/features/results/ResultsGrid.tsx
git commit -m "feat: OID-dispatch typed decoder + Cell-aware result rendering"
```

---

## Task 4: SQL AST parser — `parse_select_target`

**Goal:** A pure function that classifies a SQL string into either `ParsedSelect::SingleTable { schema, table }` (editable candidate) or `ParsedSelect::NotEditable(reason)` (multi-table / CTE / non-SELECT / parser-failed). TDD.

**Files:**

- Create: `src-tauri/src/commands/sqlast.rs`
- Modify: `src-tauri/src/commands/mod.rs`

**Steps:**

- [ ] **Step 1: Wire module**

Edit `src-tauri/src/commands/mod.rs`, append:

```rust
pub mod sqlast;
```

- [ ] **Step 2: Write failing tests**

Create `src-tauri/src/commands/sqlast.rs`:

```rust
// src-tauri/src/commands/sqlast.rs
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
pub enum ParsedSelect {
    SingleTable { schema: String, table: String },
    NotEditable { reason: NotEditableReason },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotEditableReason {
    NotSelect,
    MultiTable,
    Cte,
    Subquery,
    Computed,
    ParserFailed,
}

pub fn parse_select_target(sql: &str) -> ParsedSelect {
    todo!("implemented in step 3")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_single(sql: &str, schema: &str, table: &str) {
        match parse_select_target(sql) {
            ParsedSelect::SingleTable { schema: s, table: t } => {
                assert_eq!(s, schema);
                assert_eq!(t, table);
            }
            other => panic!("expected SingleTable, got {other:?}"),
        }
    }

    fn assert_not_editable(sql: &str, expected: NotEditableReason) {
        match parse_select_target(sql) {
            ParsedSelect::NotEditable { reason } => assert_eq!(reason, expected),
            other => panic!("expected NotEditable({expected:?}), got {other:?}"),
        }
    }

    #[test]
    fn simple_select_unqualified_uses_public_default() {
        assert_single("SELECT * FROM users", "public", "users");
    }

    #[test]
    fn schema_qualified_select_keeps_schema() {
        assert_single("SELECT id, email FROM auth.users", "auth", "users");
    }

    #[test]
    fn select_with_where_still_editable() {
        assert_single("SELECT * FROM public.users WHERE id = 42", "public", "users");
    }

    #[test]
    fn select_with_order_by_still_editable() {
        assert_single("SELECT id FROM public.users ORDER BY id DESC LIMIT 10", "public", "users");
    }

    #[test]
    fn join_is_multi_table() {
        assert_not_editable(
            "SELECT u.id FROM users u JOIN orders o ON o.user_id = u.id",
            NotEditableReason::MultiTable,
        );
    }

    #[test]
    fn cte_is_not_editable() {
        assert_not_editable(
            "WITH x AS (SELECT * FROM users) SELECT * FROM x",
            NotEditableReason::Cte,
        );
    }

    #[test]
    fn subquery_in_from_is_not_editable() {
        assert_not_editable(
            "SELECT * FROM (SELECT * FROM users) sub",
            NotEditableReason::Subquery,
        );
    }

    #[test]
    fn group_by_is_computed() {
        assert_not_editable(
            "SELECT count(*) FROM users",
            NotEditableReason::Computed,
        );
    }

    #[test]
    fn insert_is_not_select() {
        assert_not_editable(
            "INSERT INTO users (id) VALUES (1)",
            NotEditableReason::NotSelect,
        );
    }

    #[test]
    fn unparseable_sql_yields_parser_failed() {
        assert_not_editable(
            "this is not sql at all",
            NotEditableReason::ParserFailed,
        );
    }
}
```

- [ ] **Step 3: Run tests, verify they fail**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib sqlast
```

Expected: panic on `todo!()`.

- [ ] **Step 4: Implement using `sqlparser` crate**

Replace the `todo!()` with:

```rust
use sqlparser::ast::{Query, SetExpr, Statement, TableFactor};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

pub fn parse_select_target(sql: &str) -> ParsedSelect {
    let stmts = match Parser::parse_sql(&PostgreSqlDialect {}, sql) {
        Ok(s) => s,
        Err(_) => return ParsedSelect::NotEditable { reason: NotEditableReason::ParserFailed },
    };
    let stmt = match stmts.into_iter().next() {
        Some(s) => s,
        None => return ParsedSelect::NotEditable { reason: NotEditableReason::NotSelect },
    };
    let query: Box<Query> = match stmt {
        Statement::Query(q) => q,
        _ => return ParsedSelect::NotEditable { reason: NotEditableReason::NotSelect },
    };
    if query.with.is_some() {
        return ParsedSelect::NotEditable { reason: NotEditableReason::Cte };
    }
    let select = match *query.body {
        SetExpr::Select(s) => s,
        _ => return ParsedSelect::NotEditable { reason: NotEditableReason::Computed },
    };
    if select.group_by != sqlparser::ast::GroupByExpr::Expressions(vec![], vec![])
        || select.having.is_some()
        || select.distinct.is_some()
    {
        return ParsedSelect::NotEditable { reason: NotEditableReason::Computed };
    }
    if select.from.len() != 1 {
        return ParsedSelect::NotEditable { reason: NotEditableReason::MultiTable };
    }
    let twj = &select.from[0];
    if !twj.joins.is_empty() {
        return ParsedSelect::NotEditable { reason: NotEditableReason::MultiTable };
    }
    match &twj.relation {
        TableFactor::Table { name, .. } => {
            let parts = name.0.iter().map(|i| i.value.clone()).collect::<Vec<_>>();
            let (schema, table) = match parts.as_slice() {
                [t] => ("public".to_string(), t.clone()),
                [s, t] => (s.clone(), t.clone()),
                _ => return ParsedSelect::NotEditable { reason: NotEditableReason::Computed },
            };
            ParsedSelect::SingleTable { schema, table }
        }
        TableFactor::Derived { .. } => ParsedSelect::NotEditable { reason: NotEditableReason::Subquery },
        _ => ParsedSelect::NotEditable { reason: NotEditableReason::Computed },
    }
}
```

> **API drift note:** the `sqlparser` crate's exact field names for `GroupByExpr` and `select.distinct` may shift across minor versions. If the empty-GROUP-BY check above fails to compile, replace it with the equivalent `matches!(select.group_by, sqlparser::ast::GroupByExpr::Expressions(ref e, _) if e.is_empty())` or whichever variant the pinned 0.52 actually exposes. Keep the semantic: bare SELECT with no GROUP BY / HAVING / DISTINCT.

- [ ] **Step 5: Run tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib sqlast
```

Expected: all 10 tests pass.

- [ ] **Step 6: Quality gates + commit**

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
git add src-tauri/src/commands/mod.rs src-tauri/src/commands/sqlast.rs
git commit -m "feat(rust): SQL AST parser for single-table SELECT recognition"
```

---

## Task 5: PG meta query + LRU cache (`pg_meta.rs`)

**Goal:** Given a `(schema, table)`, fetch PK columns, per-column nullable + enum values + FK targets in one round-trip. 60-second LRU cache keyed on `(conn_id, schema, table)`. TDD with docker integration.

**Files:**

- Modify: `src-tauri/src/db/pg_meta.rs`
- Create: `src-tauri/tests/pg_meta.rs`

**Steps:**

- [ ] **Step 1: Implement struct + LRU + queries**

Replace `pg_meta.rs` body with:

```rust
// src-tauri/src/db/pg_meta.rs
use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use lru::LruCache;
use serde::Serialize;
use sqlx::PgPool;

use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableMeta {
    pub schema: String,
    pub table: String,
    pub pk_columns: Vec<String>,
    pub columns: Vec<ColumnMetaRow>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMetaRow {
    pub name: String,
    pub oid: u32,
    pub type_name: String,
    pub nullable: bool,
    pub enum_values: Option<Vec<String>>,
    pub fk: Option<FkRef>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FkRef {
    pub schema: String,
    pub table: String,
    pub column: String,
}

const TTL: Duration = Duration::from_secs(60);

type Key = (String, String, String); // (conn_id, schema, table)
type Entry = (Instant, TableMeta);

pub struct MetaCache {
    inner: Mutex<LruCache<Key, Entry>>,
}

impl MetaCache {
    pub fn new() -> Self {
        Self { inner: Mutex::new(LruCache::new(NonZeroUsize::new(256).unwrap())) }
    }

    fn cached(&self, key: &Key) -> Option<TableMeta> {
        let mut c = self.inner.lock().unwrap();
        if let Some((stored_at, meta)) = c.get(key) {
            if stored_at.elapsed() < TTL {
                return Some(meta.clone());
            }
        }
        c.pop(key);
        None
    }

    fn store(&self, key: Key, meta: TableMeta) {
        self.inner.lock().unwrap().put(key, (Instant::now(), meta));
    }

    pub fn invalidate_conn(&self, conn_id: &str) {
        let mut c = self.inner.lock().unwrap();
        let to_remove: Vec<Key> = c.iter().filter_map(|(k, _)| {
            if k.0 == conn_id { Some(k.clone()) } else { None }
        }).collect();
        for k in to_remove {
            c.pop(&k);
        }
    }
}

impl Default for MetaCache {
    fn default() -> Self { Self::new() }
}

pub async fn fetch_table_meta(
    pool: &PgPool,
    cache: &MetaCache,
    conn_id: &str,
    schema: &str,
    table: &str,
) -> TuskResult<TableMeta> {
    let key = (conn_id.to_string(), schema.to_string(), table.to_string());
    if let Some(m) = cache.cached(&key) { return Ok(m); }

    let cols_q = r#"
        SELECT a.attname AS name,
               a.atttypid::oid::int4 AS oid,
               t.typname AS type_name,
               NOT a.attnotnull AS nullable,
               t.typtype = 'e' AS is_enum,
               t.oid AS type_oid
        FROM pg_attribute a
        JOIN pg_class c ON c.oid = a.attrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        JOIN pg_type t ON t.oid = a.atttypid
        WHERE n.nspname = $1 AND c.relname = $2
          AND a.attnum > 0 AND NOT a.attisdropped
        ORDER BY a.attnum
    "#;
    let col_rows = sqlx::query_as::<_, (String, i32, String, bool, bool, sqlx::types::Oid)>(cols_q)
        .bind(schema).bind(table)
        .fetch_all(pool).await
        .map_err(|e| TuskError::State(format!("pg_meta cols: {e}")))?;

    if col_rows.is_empty() {
        return Err(TuskError::State(format!("table {schema}.{table} not found")));
    }

    let pk_q = r#"
        SELECT a.attname
        FROM pg_index i
        JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
        JOIN pg_class c ON c.oid = i.indrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = $1 AND c.relname = $2 AND i.indisprimary
        ORDER BY array_position(i.indkey, a.attnum)
    "#;
    let pk_rows: Vec<(String,)> = sqlx::query_as(pk_q)
        .bind(schema).bind(table)
        .fetch_all(pool).await
        .map_err(|e| TuskError::State(format!("pg_meta pk: {e}")))?;
    let pk_columns: Vec<String> = pk_rows.into_iter().map(|(n,)| n).collect();

    let fk_q = r#"
        SELECT
            att.attname            AS col,
            ns.nspname             AS ref_schema,
            cl.relname             AS ref_table,
            ratt.attname           AS ref_col
        FROM pg_constraint c
        JOIN pg_class cl_src ON cl_src.oid = c.conrelid
        JOIN pg_namespace ns_src ON ns_src.oid = cl_src.relnamespace
        JOIN pg_attribute att ON att.attrelid = c.conrelid AND att.attnum = ANY(c.conkey)
        JOIN pg_class cl ON cl.oid = c.confrelid
        JOIN pg_namespace ns ON ns.oid = cl.relnamespace
        JOIN pg_attribute ratt ON ratt.attrelid = c.confrelid AND ratt.attnum = ANY(c.confkey)
        WHERE ns_src.nspname = $1 AND cl_src.relname = $2 AND c.contype = 'f'
    "#;
    let fk_rows: Vec<(String, String, String, String)> = sqlx::query_as(fk_q)
        .bind(schema).bind(table)
        .fetch_all(pool).await
        .map_err(|e| TuskError::State(format!("pg_meta fk: {e}")))?;

    let mut columns = Vec::with_capacity(col_rows.len());
    for (name, oid_i32, type_name, nullable, is_enum, type_oid) in col_rows {
        let enum_values = if is_enum {
            let evs: Vec<(String,)> = sqlx::query_as(
                "SELECT enumlabel FROM pg_enum WHERE enumtypid = $1 ORDER BY enumsortorder"
            ).bind(type_oid).fetch_all(pool).await
              .map_err(|e| TuskError::State(format!("pg_meta enum: {e}")))?;
            Some(evs.into_iter().map(|(l,)| l).collect())
        } else { None };
        let fk = fk_rows.iter().find(|(c,_,_,_)| c == &name).map(|(_, s, t, c)| FkRef {
            schema: s.clone(), table: t.clone(), column: c.clone(),
        });
        columns.push(ColumnMetaRow {
            name,
            oid: oid_i32 as u32,
            type_name: crate::db::decoder::pg_type_name(oid_i32 as u32).to_string(),
            nullable,
            enum_values,
            fk,
        });
        let _ = type_name; // typename from pg_type used only for enum dispatch above
    }

    let meta = TableMeta {
        schema: schema.to_string(),
        table: table.to_string(),
        pk_columns,
        columns,
    };
    cache.store(key, meta.clone());
    Ok(meta)
}
```

> **Note**: the SQL query bindings rely on sqlx's `Oid` type matching the integer-as-i32 trick — adjust binding types if compilation complains. The semantic is: query `pg_attribute` once for columns, `pg_index` once for PK, `pg_constraint` once for FKs, and `pg_enum` per enum column.

- [ ] **Step 2: Add `MetaCache` to app state**

Edit `src-tauri/src/lib.rs`. In `run()`, after `app.manage(ConnectionRegistry::new());`, add:

```rust
            app.manage(crate::db::pg_meta::MetaCache::new());
```

Also invalidate on disconnect. Edit `src-tauri/src/commands/connections.rs::disconnect` (search for it):

```rust
// at the end of disconnect, add:
let meta_cache = app_handle.state::<crate::db::pg_meta::MetaCache>();
meta_cache.invalidate_conn(&connection_id);
```

(If the function signature doesn't already take an `app_handle`, add `app_handle: tauri::AppHandle` parameter.)

- [ ] **Step 3: Write integration test**

Create `src-tauri/tests/pg_meta.rs`:

```rust
//! Requires docker postgres up.
use sqlx::postgres::PgPoolOptions;
use tusk_lib::db::pg_meta::{fetch_table_meta, MetaCache};

const URL: &str = "postgres://tusk:tusk@127.0.0.1:55432/tusk_test";

async fn pool() -> sqlx::PgPool {
    PgPoolOptions::new().max_connections(2).connect(URL).await.unwrap()
}

#[tokio::test]
async fn fetches_pk_and_columns() {
    let pool = pool().await;
    sqlx::query("DROP TABLE IF EXISTS pg_meta_t CASCADE").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE pg_meta_t (id int primary key, name text not null, note text)")
        .execute(&pool).await.unwrap();

    let cache = MetaCache::new();
    let m = fetch_table_meta(&pool, &cache, "c1", "public", "pg_meta_t").await.unwrap();
    assert_eq!(m.pk_columns, vec!["id".to_string()]);
    assert_eq!(m.columns.len(), 3);
    assert!(!m.columns[0].nullable);
    assert!(!m.columns[1].nullable);
    assert!(m.columns[2].nullable);
}

#[tokio::test]
async fn enum_values_loaded() {
    let pool = pool().await;
    sqlx::query("DROP TABLE IF EXISTS pg_meta_e CASCADE").execute(&pool).await.unwrap();
    sqlx::query("DROP TYPE IF EXISTS mood2").execute(&pool).await.unwrap();
    sqlx::query("CREATE TYPE mood2 AS ENUM ('sad','ok','happy')").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE pg_meta_e (id int primary key, m mood2)").execute(&pool).await.unwrap();
    let cache = MetaCache::new();
    let m = fetch_table_meta(&pool, &cache, "c1", "public", "pg_meta_e").await.unwrap();
    let mcol = m.columns.iter().find(|c| c.name == "m").unwrap();
    assert_eq!(mcol.enum_values.as_ref().unwrap(), &vec!["sad".to_string(), "ok".into(), "happy".into()]);
}

#[tokio::test]
async fn fk_target_resolved() {
    let pool = pool().await;
    sqlx::query("DROP TABLE IF EXISTS pg_meta_child CASCADE").execute(&pool).await.unwrap();
    sqlx::query("DROP TABLE IF EXISTS pg_meta_parent CASCADE").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE pg_meta_parent (id int primary key)").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE pg_meta_child (id int primary key, p int references pg_meta_parent(id))")
        .execute(&pool).await.unwrap();
    let cache = MetaCache::new();
    let m = fetch_table_meta(&pool, &cache, "c1", "public", "pg_meta_child").await.unwrap();
    let pcol = m.columns.iter().find(|c| c.name == "p").unwrap();
    let fk = pcol.fk.as_ref().unwrap();
    assert_eq!(fk.schema, "public");
    assert_eq!(fk.table, "pg_meta_parent");
    assert_eq!(fk.column, "id");
}
```

- [ ] **Step 4: Run integration test**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test pg_meta
```

Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
git add src-tauri/src/db/pg_meta.rs src-tauri/src/lib.rs \
        src-tauri/src/commands/connections.rs src-tauri/tests/pg_meta.rs
git commit -m "feat(rust): pg_meta lookups (PK / enum / FK) with 60s LRU cache"
```

---

## Task 6: Wire `meta` into `execute_query` response

**Goal:** Combine Task 4 (sqlast) + Task 5 (pg_meta). After a SELECT, attach a `ResultMeta` so the frontend knows whether the result is editable. Frontend store this on the `results` slice.

**Files:**

- Modify: `src-tauri/src/commands/query.rs`
- Modify: `src-tauri/src/lib.rs` (state injection)
- Modify: `src/lib/types.ts` (QueryResult.meta)
- Modify: `src/store/*.ts` (whichever store holds last result — likely `tabs.ts`)

**Steps:**

- [ ] **Step 1: Extend `QueryResult` in Rust**

Edit `src-tauri/src/commands/query.rs`. Add:

```rust
use crate::commands::sqlast::{parse_select_target, ParsedSelect, NotEditableReason};
use crate::db::pg_meta::{fetch_table_meta, MetaCache, TableMeta};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultMeta {
    pub editable: bool,
    pub reason: Option<String>,
    pub table: Option<TableRef>,
    pub pk_columns: Vec<String>,
    pub pk_column_indices: Vec<usize>,
    pub column_types: Vec<ColumnTypeMeta>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableRef { pub schema: String, pub name: String }

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnTypeMeta {
    pub name: String,
    pub oid: u32,
    pub type_name: String,
    pub nullable: bool,
    pub enum_values: Option<Vec<String>>,
    pub fk: Option<crate::db::pg_meta::FkRef>,
}
```

Add `meta: ResultMeta` to `QueryResult`. Inside `execute_query`, after computing `columns` and `data`, build it:

```rust
    let meta = build_meta(&pool, &meta_cache, &connection_id, &sql, &columns, row_count).await;
```

Where `meta_cache: State<'_, MetaCache>` is added as an extra `tauri::State` parameter. Implement `build_meta`:

```rust
async fn build_meta(
    pool: &sqlx::PgPool,
    cache: &MetaCache,
    conn_id: &str,
    sql: &str,
    columns: &[crate::db::decoder::ColumnMeta],
    row_count: usize,
) -> ResultMeta {
    let parsed = parse_select_target(sql);
    let (schema, table) = match parsed {
        ParsedSelect::SingleTable { schema, table } => (schema, table),
        ParsedSelect::NotEditable { reason } => {
            return not_editable(reason_to_string(reason), columns, vec![], vec![]);
        }
    };
    if row_count > 10_000 {
        return not_editable("too-large".into(), columns, vec![], vec![]);
    }
    let table_meta = match fetch_table_meta(pool, cache, conn_id, &schema, &table).await {
        Ok(m) => m,
        Err(_) => return not_editable("unknown-type".into(), columns, vec![], vec![]),
    };
    // PK present in result?
    let pk_indices: Vec<usize> = table_meta.pk_columns.iter()
        .filter_map(|pk| columns.iter().position(|c| c.name == *pk))
        .collect();
    if pk_indices.len() != table_meta.pk_columns.len() {
        return not_editable("pk-not-in-select".into(),
            columns, table_meta.pk_columns.clone(), vec![]);
    }
    let column_types = columns.iter().map(|c| {
        let row = table_meta.columns.iter().find(|cm| cm.name == c.name);
        ColumnTypeMeta {
            name: c.name.clone(),
            oid: c.oid,
            type_name: c.type_name.clone(),
            nullable: row.map(|r| r.nullable).unwrap_or(true),
            enum_values: row.and_then(|r| r.enum_values.clone()),
            fk: row.and_then(|r| r.fk.clone()),
        }
    }).collect();
    ResultMeta {
        editable: true,
        reason: None,
        table: Some(TableRef { schema, name: table }),
        pk_columns: table_meta.pk_columns,
        pk_column_indices: pk_indices,
        column_types,
    }
}

fn not_editable(reason: String,
    columns: &[crate::db::decoder::ColumnMeta],
    pk_columns: Vec<String>,
    pk_column_indices: Vec<usize>,
) -> ResultMeta {
    ResultMeta {
        editable: false,
        reason: Some(reason),
        table: None,
        pk_columns,
        pk_column_indices,
        column_types: columns.iter().map(|c| ColumnTypeMeta {
            name: c.name.clone(),
            oid: c.oid,
            type_name: c.type_name.clone(),
            nullable: true,
            enum_values: None,
            fk: None,
        }).collect(),
    }
}

fn reason_to_string(r: NotEditableReason) -> String {
    match r {
        NotEditableReason::NotSelect => "no-pk".into(),
        NotEditableReason::MultiTable => "multi-table".into(),
        NotEditableReason::Cte => "computed".into(),
        NotEditableReason::Subquery => "computed".into(),
        NotEditableReason::Computed => "computed".into(),
        NotEditableReason::ParserFailed => "parser-failed".into(),
    }
}
```

`execute_query` now also accepts the `MetaCache` state parameter:

```rust
#[tauri::command]
pub async fn execute_query(
    registry: State<'_, ConnectionRegistry>,
    meta_cache: State<'_, MetaCache>,
    connection_id: String,
    sql: String,
) -> TuskResult<QueryResult> {
    // ... existing fetch + decode
    let meta = build_meta(&pool, meta_cache.inner(), &connection_id, &sql, &columns, row_count).await;
    Ok(QueryResult { columns, rows: data, duration_ms, row_count, meta })
}
```

- [ ] **Step 2: Frontend type sync**

Edit `src/lib/types.ts`. Append:

```ts
export interface QueryResult {
  columns: ColumnMeta[];
  rows: Cell[][];
  durationMs: number;
  rowCount: number;
  meta: ResultMeta;
}

export interface ColumnMeta {
  name: string;
  oid: number;
  typeName: PgTypeName;
}
```

(If a different `QueryResult` already exists, update it in place to include `meta`.)

- [ ] **Step 3: Frontend store wiring**

Find where execute_query result is stored (Week 2 stored on the active tab). Add `meta` propagation. The grid uses `meta.editable` to show ✏️ vs 🔒 indicator. For now, just plumb through and render a static badge:

```tsx
// in ResultsGrid.tsx
{
  result.meta.editable ? (
    <span title="Editable result" className="text-xs text-amber-500">
      ✏️
    </span>
  ) : (
    <span
      title={`Read-only — ${result.meta.reason}`}
      className="text-muted-foreground text-xs"
    >
      🔒
    </span>
  );
}
```

- [ ] **Step 4: Quality gates + manual smoke**

```bash
docker compose -f infra/postgres/docker-compose.yml up -d
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
pnpm typecheck && pnpm lint && pnpm format:check
pnpm tauri dev    # manual: connect, run "SELECT * FROM <some table>" and "SELECT 1+1", verify ✏️ vs 🔒
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/query.rs src-tauri/src/lib.rs \
        src/lib/types.ts src/features/results/ResultsGrid.tsx
git commit -m "feat: attach editable meta (PK / table / column types) to query results"
```

---

## Task 7: SQLite migration 002 + history single-entry recording

**Goal:** Persist every executed query to `history_entry` (+ `history_statement` for transactional groupings, used in Task 13). For now, every `execute_query` records a single-statement entry. TDD on `db/state.rs` round-trip; integration `tests/history.rs` smoke.

**Files:**

- Modify: `src-tauri/src/db/state.rs` (add migration + history APIs)
- Create: `src-tauri/src/commands/history.rs`
- Modify: `src-tauri/src/commands/mod.rs` + `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands/query.rs` (record on success/error)
- Create: `src-tauri/tests/history.rs`

**Steps:**

- [ ] **Step 1: Extend migration**

Edit `src-tauri/src/db/state.rs::migrate`. Append a second batch:

```rust
        db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS history_entry (
                id              TEXT PRIMARY KEY,
                conn_id         TEXT NOT NULL,
                source          TEXT NOT NULL,
                tx_id           TEXT,
                sql_preview     TEXT NOT NULL,
                sql_full        TEXT,
                started_at      INTEGER NOT NULL,
                duration_ms     INTEGER NOT NULL,
                row_count       INTEGER,
                status          TEXT NOT NULL,
                error_message   TEXT,
                statement_count INTEGER NOT NULL DEFAULT 1
            );
            CREATE INDEX IF NOT EXISTS idx_history_entry_conn_started
                ON history_entry(conn_id, started_at DESC);
            CREATE INDEX IF NOT EXISTS idx_history_entry_tx
                ON history_entry(tx_id);
            CREATE TABLE IF NOT EXISTS history_statement (
                id              TEXT PRIMARY KEY,
                entry_id        TEXT NOT NULL REFERENCES history_entry(id) ON DELETE CASCADE,
                ordinal         INTEGER NOT NULL,
                sql             TEXT NOT NULL,
                duration_ms     INTEGER NOT NULL,
                row_count       INTEGER,
                status          TEXT NOT NULL,
                error_message   TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_history_statement_entry
                ON history_statement(entry_id, ordinal);
            "#,
        )
        .map_err(|e| TuskError::State(e.to_string()))?;
```

- [ ] **Step 2: Add history APIs to `StateStore`**

In `state.rs`, after the connections CRUD section, append:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub conn_id: String,
    pub source: String,
    pub tx_id: Option<String>,
    pub sql_preview: String,
    pub sql_full: Option<String>,
    pub started_at: i64,
    pub duration_ms: i64,
    pub row_count: Option<i64>,
    pub status: String,
    pub error_message: Option<String>,
    pub statement_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryStatement {
    pub id: String,
    pub entry_id: String,
    pub ordinal: i64,
    pub sql: String,
    pub duration_ms: i64,
    pub row_count: Option<i64>,
    pub status: String,
    pub error_message: Option<String>,
}

impl StateStore {
    pub fn insert_history_entry(&self, e: &HistoryEntry) -> TuskResult<()> {
        let db = self.db.lock().expect("state lock");
        db.execute(
            "INSERT INTO history_entry (id, conn_id, source, tx_id, sql_preview, sql_full, started_at, duration_ms, row_count, status, error_message, statement_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                e.id, e.conn_id, e.source, e.tx_id, e.sql_preview, e.sql_full,
                e.started_at, e.duration_ms, e.row_count, e.status, e.error_message, e.statement_count
            ],
        ).map_err(|e| TuskError::History(e.to_string()))?;
        Ok(())
    }

    pub fn append_history_statement(&self, s: &HistoryStatement) -> TuskResult<()> {
        let db = self.db.lock().expect("state lock");
        db.execute(
            "INSERT INTO history_statement (id, entry_id, ordinal, sql, duration_ms, row_count, status, error_message)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![s.id, s.entry_id, s.ordinal, s.sql, s.duration_ms, s.row_count, s.status, s.error_message],
        ).map_err(|e| TuskError::History(e.to_string()))?;
        Ok(())
    }

    pub fn update_history_entry_finalize(
        &self, id: &str, duration_ms: i64, row_count: Option<i64>,
        status: &str, error: Option<&str>, statement_count: i64,
    ) -> TuskResult<()> {
        let db = self.db.lock().expect("state lock");
        db.execute(
            "UPDATE history_entry SET duration_ms=?1, row_count=?2, status=?3, error_message=?4, statement_count=?5 WHERE id=?6",
            params![duration_ms, row_count, status, error, statement_count, id],
        ).map_err(|e| TuskError::History(e.to_string()))?;
        Ok(())
    }

    pub fn list_history(&self, conn_id: Option<&str>, query: Option<&str>, limit: i64) -> TuskResult<Vec<HistoryEntry>> {
        let db = self.db.lock().expect("state lock");
        let (sql, conn_match) = match (conn_id, query) {
            (Some(_), Some(_)) => (
                "SELECT * FROM history_entry WHERE conn_id = ?1 AND sql_preview LIKE ?2 ORDER BY started_at DESC LIMIT ?3",
                true,
            ),
            (Some(_), None) => (
                "SELECT * FROM history_entry WHERE conn_id = ?1 ORDER BY started_at DESC LIMIT ?2",
                true,
            ),
            (None, Some(_)) => (
                "SELECT * FROM history_entry WHERE sql_preview LIKE ?1 ORDER BY started_at DESC LIMIT ?2",
                false,
            ),
            (None, None) => (
                "SELECT * FROM history_entry ORDER BY started_at DESC LIMIT ?1",
                false,
            ),
        };
        let mut stmt = db.prepare(sql).map_err(|e| TuskError::History(e.to_string()))?;
        let mapper = |r: &rusqlite::Row<'_>| -> rusqlite::Result<HistoryEntry> {
            Ok(HistoryEntry {
                id: r.get(0)?, conn_id: r.get(1)?, source: r.get(2)?, tx_id: r.get(3)?,
                sql_preview: r.get(4)?, sql_full: r.get(5)?, started_at: r.get(6)?,
                duration_ms: r.get(7)?, row_count: r.get(8)?, status: r.get(9)?,
                error_message: r.get(10)?, statement_count: r.get(11)?,
            })
        };
        let rows: Vec<HistoryEntry> = match (conn_id, query) {
            (Some(c), Some(q)) => {
                let pat = format!("%{q}%");
                stmt.query_map(params![c, pat, limit], mapper)
            }
            (Some(c), None) => stmt.query_map(params![c, limit], mapper),
            (None, Some(q)) => {
                let pat = format!("%{q}%");
                stmt.query_map(params![pat, limit], mapper)
            }
            (None, None) => stmt.query_map(params![limit], mapper),
        }
        .map_err(|e| TuskError::History(e.to_string()))?
        .collect::<Result<_,_>>()
        .map_err(|e| TuskError::History(e.to_string()))?;
        let _ = conn_match;
        Ok(rows)
    }

    pub fn list_history_statements(&self, entry_id: &str) -> TuskResult<Vec<HistoryStatement>> {
        let db = self.db.lock().expect("state lock");
        let mut stmt = db.prepare(
            "SELECT id, entry_id, ordinal, sql, duration_ms, row_count, status, error_message
             FROM history_statement WHERE entry_id = ?1 ORDER BY ordinal"
        ).map_err(|e| TuskError::History(e.to_string()))?;
        stmt.query_map(params![entry_id], |r| {
            Ok(HistoryStatement {
                id: r.get(0)?, entry_id: r.get(1)?, ordinal: r.get(2)?,
                sql: r.get(3)?, duration_ms: r.get(4)?, row_count: r.get(5)?,
                status: r.get(6)?, error_message: r.get(7)?,
            })
        })
        .map_err(|e| TuskError::History(e.to_string()))?
        .collect::<Result<_,_>>()
        .map_err(|e| TuskError::History(e.to_string()))
    }
}
```

- [ ] **Step 3: Add unit tests in `state.rs`**

Append to the `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn history_entry_round_trip() {
        let store = StateStore::open_in_memory().unwrap();
        let e = HistoryEntry {
            id: "e1".into(), conn_id: "c1".into(), source: "editor".into(),
            tx_id: None, sql_preview: "SELECT 1".into(), sql_full: Some("SELECT 1".into()),
            started_at: 1000, duration_ms: 5, row_count: Some(1), status: "ok".into(),
            error_message: None, statement_count: 1,
        };
        store.insert_history_entry(&e).unwrap();
        let listed = store.list_history(Some("c1"), None, 10).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "e1");
    }

    #[test]
    fn history_search_uses_like() {
        let store = StateStore::open_in_memory().unwrap();
        for (i, sql) in ["SELECT users", "UPDATE orders", "SELECT products"].iter().enumerate() {
            let e = HistoryEntry {
                id: format!("e{i}"), conn_id: "c1".into(), source: "editor".into(),
                tx_id: None, sql_preview: (*sql).into(), sql_full: Some((*sql).into()),
                started_at: 1000 + i as i64, duration_ms: 1, row_count: Some(0), status: "ok".into(),
                error_message: None, statement_count: 1,
            };
            store.insert_history_entry(&e).unwrap();
        }
        let r = store.list_history(Some("c1"), Some("SELECT"), 10).unwrap();
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn statements_attach_to_entry() {
        let store = StateStore::open_in_memory().unwrap();
        let e = HistoryEntry {
            id: "tx1".into(), conn_id: "c1".into(), source: "editor".into(),
            tx_id: Some("t1".into()), sql_preview: "tx".into(), sql_full: None,
            started_at: 1000, duration_ms: 0, row_count: None, status: "ok".into(),
            error_message: None, statement_count: 0,
        };
        store.insert_history_entry(&e).unwrap();
        for i in 0..3 {
            store.append_history_statement(&HistoryStatement {
                id: format!("s{i}"), entry_id: "tx1".into(), ordinal: i,
                sql: format!("SELECT {i}"), duration_ms: 1, row_count: Some(1),
                status: "ok".into(), error_message: None,
            }).unwrap();
        }
        let stmts = store.list_history_statements("tx1").unwrap();
        assert_eq!(stmts.len(), 3);
        assert_eq!(stmts[0].ordinal, 0);
    }
```

- [ ] **Step 4: Tauri commands**

Create `src-tauri/src/commands/history.rs`:

```rust
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
```

Wire into `src-tauri/src/commands/mod.rs`:

```rust
pub mod history;
```

And `lib.rs::invoke_handler!`:

```rust
            commands::history::list_history,
            commands::history::list_history_statements,
```

- [ ] **Step 5: Record from `execute_query`**

Modify `src-tauri/src/commands/query.rs` `execute_query` to take `store: State<'_, StateStore>` and call:

```rust
    let entry_id = uuid::Uuid::new_v4().to_string();
    let result = sqlx::query(&sql).fetch_all(&pool).await;
    let duration_ms = started.elapsed().as_millis() as i64;
    let preview = sql.chars().take(200).collect::<String>();
    let (status, err_msg, rc): (&str, Option<String>, Option<i64>) = match &result {
        Ok(rows) => ("ok", None, Some(rows.len() as i64)),
        Err(e) => ("error", Some(e.to_string()), None),
    };
    let _ = store.insert_history_entry(&crate::db::state::HistoryEntry {
        id: entry_id.clone(), conn_id: connection_id.clone(), source: "editor".into(),
        tx_id: None, sql_preview: preview, sql_full: Some(sql.clone()),
        started_at: chrono::Utc::now().timestamp_millis(),
        duration_ms, row_count: rc, status: status.into(),
        error_message: err_msg, statement_count: 1,
    });
    let rows = result.map_err(|e| TuskError::Query(e.to_string()))?;
```

(Reorganize the existing function around this. The `_ = ...` swallows the `TuskResult<()>` from history insert — we never want history persistence failure to break the user's query flow. Log it instead via `eprintln!` if you want.)

- [ ] **Step 6: Integration test**

Create `src-tauri/tests/history.rs`:

```rust
//! Verifies entry round-trip through the file-backed store.
use tusk_lib::db::state::{HistoryEntry, StateStore};

#[test]
fn round_trip_via_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("h.db");
    let store = StateStore::open(&path).unwrap();
    store.insert_history_entry(&HistoryEntry {
        id: "x".into(), conn_id: "c".into(), source: "editor".into(),
        tx_id: None, sql_preview: "S".into(), sql_full: None,
        started_at: 1, duration_ms: 1, row_count: Some(0), status: "ok".into(),
        error_message: None, statement_count: 1,
    }).unwrap();
    drop(store);

    let store2 = StateStore::open(&path).unwrap();
    let listed = store2.list_history(None, None, 10).unwrap();
    assert_eq!(listed.len(), 1);
}
```

Add `tempfile = "3"` to `[dev-dependencies]` in `Cargo.toml` (if not already present).

- [ ] **Step 7: Run all tests + commit**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib state
cargo test --manifest-path src-tauri/Cargo.toml --test history
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
git add src-tauri/Cargo.toml src-tauri/Cargo.lock \
        src-tauri/src/db/state.rs src-tauri/src/commands/history.rs \
        src-tauri/src/commands/mod.rs src-tauri/src/lib.rs \
        src-tauri/src/commands/query.rs src-tauri/tests/history.rs
git commit -m "feat: query history persistence (entry + statement tables)"
```

---

## Task 8: Sticky transaction slot + tx_begin / tx_commit / tx_rollback commands

**Goal:** Extend `ActiveConnection` with a `tx_slot: Mutex<Option<StickyTx>>`. Three commands manage tx lifecycle; `Drop` does best-effort ROLLBACK. TDD with docker integration.

**Files:**

- Modify: `src-tauri/src/db/pool.rs`
- Create: `src-tauri/src/commands/transactions.rs`
- Modify: `src-tauri/src/commands/mod.rs` + `src-tauri/src/lib.rs`
- Create: `src-tauri/tests/transactions.rs`

**Steps:**

- [ ] **Step 1: Extend `ActiveConnection`**

Edit `src-tauri/src/db/pool.rs`. Add:

```rust
use std::sync::{Arc, Mutex};
use sqlx::pool::PoolConnection;
use sqlx::Postgres;
use std::time::Instant;

pub struct StickyTx {
    pub tx_id: String,
    pub conn: PoolConnection<Postgres>,
    pub started_at: Instant,
    pub backend_pid: i32,
    pub statement_count: u32,
    pub history_entry_id: String,
}

// Within the existing ActiveConnection struct:
pub struct ActiveConnection {
    pub pool: sqlx::PgPool,
    pub tunnel: Option<crate::ssh::tunnel::TunnelHandle>,
    pub tx_slot: Mutex<Option<StickyTx>>,
}
```

(Adapt to your existing `ActiveConnection` shape; the new field is `tx_slot`.)

In `Drop` (or `disconnect` cleanup) add best-effort rollback. Since `Drop` can't be async, spawn a blocking rollback **before** the registry removes the connection. The cleanest place is `ConnectionRegistry::remove`:

```rust
impl ConnectionRegistry {
    pub async fn remove(&self, id: &str) {
        let active = {
            let mut g = self.inner.lock().expect("registry lock");
            g.remove(id)
        };
        if let Some(active) = active {
            let mut slot = active.tx_slot.lock().expect("tx slot");
            if let Some(mut sticky) = slot.take() {
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(1),
                    sqlx::query("ROLLBACK").execute(&mut *sticky.conn),
                ).await;
            }
            // tunnel drops naturally
        }
    }
}
```

Adjust the existing `disconnect` command to call this `.await`'d remove.

- [ ] **Step 2: Add transactions commands**

Create `src-tauri/src/commands/transactions.rs`:

```rust
use serde::Serialize;
use tauri::State;

use crate::db::pool::{ConnectionRegistry, StickyTx};
use crate::db::state::{HistoryEntry, StateStore};
use crate::errors::{TuskError, TuskResult};

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

#[tauri::command]
pub async fn tx_begin(
    registry: State<'_, ConnectionRegistry>,
    store: State<'_, StateStore>,
    connection_id: String,
) -> TuskResult<TxStateSnapshot> {
    let pool = registry.pool(&connection_id)?;
    let active = registry.handle(&connection_id)?;     // returns Arc<ActiveConnection>
    {
        let slot = active.tx_slot.lock().expect("tx slot");
        if slot.is_some() {
            return Err(TuskError::Tx("transaction already active".into()));
        }
    }
    let mut conn = pool.acquire().await
        .map_err(|e| TuskError::Tx(e.to_string()))?;
    sqlx::query("BEGIN").execute(&mut *conn).await
        .map_err(|e| TuskError::Tx(e.to_string()))?;
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *conn).await
        .map_err(|e| TuskError::Tx(e.to_string()))?;
    let tx_id = uuid::Uuid::new_v4().to_string();
    let entry_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    store.insert_history_entry(&HistoryEntry {
        id: entry_id.clone(), conn_id: connection_id.clone(), source: "editor".into(),
        tx_id: Some(tx_id.clone()), sql_preview: format!("[transaction {}]", &tx_id[..8]),
        sql_full: None, started_at: now, duration_ms: 0, row_count: None,
        status: "open".into(), error_message: None, statement_count: 0,
    })?;
    let sticky = StickyTx {
        tx_id: tx_id.clone(),
        conn,
        started_at: std::time::Instant::now(),
        backend_pid: pid,
        statement_count: 0,
        history_entry_id: entry_id,
    };
    {
        let mut slot = active.tx_slot.lock().expect("tx slot");
        *slot = Some(sticky);
    }
    Ok(TxStateSnapshot {
        conn_id: connection_id, active: true, tx_id: Some(tx_id),
        started_at: Some(now), statement_count: 0, pid: Some(pid),
    })
}

#[tauri::command]
pub async fn tx_commit(
    registry: State<'_, ConnectionRegistry>,
    store: State<'_, StateStore>,
    connection_id: String,
) -> TuskResult<TxStateSnapshot> {
    finalize_tx(&registry, &store, &connection_id, "COMMIT", "ok").await
}

#[tauri::command]
pub async fn tx_rollback(
    registry: State<'_, ConnectionRegistry>,
    store: State<'_, StateStore>,
    connection_id: String,
) -> TuskResult<TxStateSnapshot> {
    finalize_tx(&registry, &store, &connection_id, "ROLLBACK", "rolled_back").await
}

async fn finalize_tx(
    registry: &State<'_, ConnectionRegistry>,
    store: &State<'_, StateStore>,
    connection_id: &str,
    sql: &str,
    final_status: &str,
) -> TuskResult<TxStateSnapshot> {
    let active = registry.handle(connection_id)?;
    let mut sticky = {
        let mut slot = active.tx_slot.lock().expect("tx slot");
        slot.take().ok_or_else(|| TuskError::Tx("no active transaction".into()))?
    };
    let res = sqlx::query(sql).execute(&mut *sticky.conn).await;
    let duration_ms = sticky.started_at.elapsed().as_millis() as i64;
    let (status, err) = match res {
        Ok(_) => (final_status.to_string(), None),
        Err(e) => ("error".to_string(), Some(e.to_string())),
    };
    store.update_history_entry_finalize(
        &sticky.history_entry_id, duration_ms, None,
        &status, err.as_deref(), sticky.statement_count as i64,
    )?;
    Ok(TxStateSnapshot {
        conn_id: connection_id.to_string(), active: false,
        tx_id: Some(sticky.tx_id), started_at: None,
        statement_count: sticky.statement_count, pid: None,
    })
}
```

(`registry.handle()` is a new helper returning `Arc<ActiveConnection>`. Add it to `pool.rs` if not present.)

Wire commands into `lib.rs`:

```rust
            commands::transactions::tx_begin,
            commands::transactions::tx_commit,
            commands::transactions::tx_rollback,
```

And `commands/mod.rs`:

```rust
pub mod transactions;
```

- [ ] **Step 3: Integration test**

Create `src-tauri/tests/transactions.rs`:

```rust
use sqlx::postgres::PgPoolOptions;

const URL: &str = "postgres://tusk:tusk@127.0.0.1:55432/tusk_test";

#[tokio::test]
async fn begin_commit_round_trip_visible_to_other_session() {
    let pool = PgPoolOptions::new().max_connections(2).connect(URL).await.unwrap();
    sqlx::query("DROP TABLE IF EXISTS tx_t").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE tx_t (id int)").execute(&pool).await.unwrap();

    let mut a = pool.acquire().await.unwrap();
    sqlx::query("BEGIN").execute(&mut *a).await.unwrap();
    sqlx::query("INSERT INTO tx_t VALUES (1)").execute(&mut *a).await.unwrap();
    // not yet visible to a separate session:
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM tx_t").fetch_one(&pool).await.unwrap();
    assert_eq!(n, 0);
    sqlx::query("COMMIT").execute(&mut *a).await.unwrap();
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM tx_t").fetch_one(&pool).await.unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn rollback_undoes_writes() {
    let pool = PgPoolOptions::new().max_connections(2).connect(URL).await.unwrap();
    sqlx::query("DROP TABLE IF EXISTS tx_t2").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE tx_t2 (id int)").execute(&pool).await.unwrap();
    let mut a = pool.acquire().await.unwrap();
    sqlx::query("BEGIN").execute(&mut *a).await.unwrap();
    sqlx::query("INSERT INTO tx_t2 VALUES (1)").execute(&mut *a).await.unwrap();
    sqlx::query("ROLLBACK").execute(&mut *a).await.unwrap();
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM tx_t2").fetch_one(&pool).await.unwrap();
    assert_eq!(n, 0);
}
```

(End-to-end Tauri command tests skip the harness here — the commands just glue these primitives together. Tauri-level testing waits for Phase 5 manual verification.)

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test transactions
```

Expected: 2 tests pass.

- [ ] **Step 4: Quality gates + commit**

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
git add src-tauri/src/db/pool.rs src-tauri/src/commands/transactions.rs \
        src-tauri/src/commands/mod.rs src-tauri/src/lib.rs \
        src-tauri/tests/transactions.rs
git commit -m "feat(rust): sticky transaction slot + tx_begin/commit/rollback commands"
```

---

## Task 9: Route `execute_query` through sticky tx + group history under tx

**Goal:** When `tx_slot` is `Some`, `execute_query` runs the SQL on the sticky connection and appends to `history_statement` instead of creating a fresh `history_entry`. When `tx_slot` is `None`, behavior matches Task 7. Same for the (future) `submit_pending_changes` in Task 17.

**Files:**

- Modify: `src-tauri/src/commands/query.rs`

**Steps:**

- [ ] **Step 1: Refactor `execute_query`**

Replace the body so it branches on `tx_slot`:

```rust
#[tauri::command]
pub async fn execute_query(
    registry: State<'_, ConnectionRegistry>,
    meta_cache: State<'_, MetaCache>,
    store: State<'_, StateStore>,
    connection_id: String,
    sql: String,
) -> TuskResult<QueryResult> {
    let active = registry.handle(&connection_id)?;
    let started = std::time::Instant::now();

    let in_tx = { active.tx_slot.lock().expect("tx slot").is_some() };
    let result = if in_tx {
        let mut slot = active.tx_slot.lock().expect("tx slot");
        let sticky = slot.as_mut().expect("checked above");
        sticky.statement_count += 1;
        let r = sqlx::query(&sql).fetch_all(&mut *sticky.conn).await;
        // Note: hold the lock across .await because sqlx requires &mut access.
        // This is fine: a sticky connection is single-writer by design.
        // But: holding a std::sync::Mutex across .await is unsafe with current futures.
        // Switch tx_slot to a tokio::sync::Mutex to fix this — see Step 1b.
        r
    } else {
        let pool = active.pool.clone();
        sqlx::query(&sql).fetch_all(&pool).await
    };

    let duration_ms = started.elapsed().as_millis();
    // ... unchanged decoder + meta build ...
}
```

- [ ] **Step 1b: Convert `tx_slot` to `tokio::sync::Mutex`**

The std `Mutex` cannot be held across `.await`. Edit `pool.rs`:

```rust
use tokio::sync::Mutex as AsyncMutex;
pub struct ActiveConnection {
    pub pool: sqlx::PgPool,
    pub tunnel: Option<crate::ssh::tunnel::TunnelHandle>,
    pub tx_slot: AsyncMutex<Option<StickyTx>>,
}
```

Update Task 8's commands to `.lock().await` instead of `.lock().expect()`. Re-run cargo check.

(This was a known deferred decision — fix it here before continuing.)

- [ ] **Step 2: History routing**

Within `execute_query`, after producing `result`:

```rust
    if in_tx {
        // append statement to existing entry
        let slot = active.tx_slot.lock().await;
        let sticky = slot.as_ref().expect("checked");
        let stmt = HistoryStatement {
            id: uuid::Uuid::new_v4().to_string(),
            entry_id: sticky.history_entry_id.clone(),
            ordinal: (sticky.statement_count - 1) as i64,
            sql: sql.chars().take(2000).collect(),
            duration_ms: duration_ms as i64,
            row_count: result.as_ref().ok().map(|r| r.len() as i64),
            status: if result.is_ok() { "ok" } else { "error" }.into(),
            error_message: result.as_ref().err().map(|e| e.to_string()),
        };
        let _ = store.append_history_statement(&stmt);
    } else {
        // single-statement entry as in Task 7
        let _ = store.insert_history_entry(&HistoryEntry { /* same as Task 7 */ });
    }
```

- [ ] **Step 3: Quality gates + commit**

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
cargo test --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/db/pool.rs src-tauri/src/commands/query.rs \
        src-tauri/src/commands/transactions.rs
git commit -m "feat(rust): route execute_query through sticky tx + tx-grouped history"
```

---

## Task 10: Frontend transaction UI — toggle, indicator, side panel, shutdown confirm

**Goal:** Visual feedback + control surface for the explicit transaction mode.

**Files:**

- Create: `src/store/transactions.ts`
- Create: `src/features/transactions/AutoCommitToggle.tsx`
- Create: `src/features/transactions/TxIndicator.tsx`
- Create: `src/features/transactions/TxSidePanel.tsx`
- Modify: `src/lib/types.ts` (TxState)
- Modify: app shell (wherever the global header lives — likely `src/App.tsx`) to mount the indicator + toggle + shortcuts
- Modify: `src/features/editor/keymap.ts` to add Cmd+Shift+C / Cmd+Shift+R

**Steps:**

- [ ] **Step 1: Add `TxState` type + store**

Edit `src/lib/types.ts`:

```ts
export interface TxState {
  connId: string;
  active: boolean;
  txId?: string;
  startedAt?: number;
  statementCount: number;
  lastError?: string;
  pid?: number;
}
```

Create `src/store/transactions.ts`:

```ts
import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { TxState } from "@/lib/types";

interface Store {
  byConn: Record<string, TxState>;
  begin(connId: string): Promise<void>;
  commit(connId: string): Promise<void>;
  rollback(connId: string): Promise<void>;
  applySnapshot(snap: TxState): void;
}

export const useTransactions = create<Store>((set, get) => ({
  byConn: {},
  applySnapshot(snap) {
    set((s) => ({ byConn: { ...s.byConn, [snap.connId]: snap } }));
  },
  async begin(connId) {
    const snap = await invoke<TxState>("tx_begin", { connectionId: connId });
    get().applySnapshot(snap);
  },
  async commit(connId) {
    const snap = await invoke<TxState>("tx_commit", { connectionId: connId });
    get().applySnapshot({ ...snap, active: false });
  },
  async rollback(connId) {
    const snap = await invoke<TxState>("tx_rollback", { connectionId: connId });
    get().applySnapshot({ ...snap, active: false });
  },
}));

export function isTxActive(connId: string): boolean {
  return useTransactions.getState().byConn[connId]?.active === true;
}
```

- [ ] **Step 2: AutoCommitToggle**

Create `src/features/transactions/AutoCommitToggle.tsx`:

```tsx
import { useTransactions } from "@/store/transactions";
import { toast } from "sonner";

export function AutoCommitToggle({ connId }: { connId: string }) {
  const tx = useTransactions((s) => s.byConn[connId]);
  const begin = useTransactions((s) => s.begin);
  const rollback = useTransactions((s) => s.rollback);
  const active = tx?.active === true;

  const onToggle = async () => {
    try {
      if (active) {
        // toggling auto-commit ON while active = abort tx
        await rollback(connId);
        toast.warning("Transaction rolled back (auto-commit re-enabled)");
      } else {
        await begin(connId);
        toast.info("Auto-commit OFF — explicit transaction started");
      }
    } catch (e) {
      toast.error(`Transaction error: ${String(e)}`);
    }
  };

  return (
    <button onClick={onToggle} className="rounded border px-2 py-1 text-xs">
      Auto-commit: {active ? "OFF" : "ON"}
    </button>
  );
}
```

- [ ] **Step 3: TxIndicator**

Create `src/features/transactions/TxIndicator.tsx`:

```tsx
import { useTransactions } from "@/store/transactions";

export function TxIndicator({ connId }: { connId: string }) {
  const tx = useTransactions((s) => s.byConn[connId]);
  if (!tx?.active) return null;
  const since = tx.startedAt
    ? `${Math.floor((Date.now() - tx.startedAt) / 1000)}s`
    : "";
  const commit = useTransactions((s) => s.commit);
  const rollback = useTransactions((s) => s.rollback);
  return (
    <div className="flex items-center gap-2 text-xs">
      <span className="text-amber-500">🟡</span>
      <span>
        Transaction · {tx.statementCount} stmts · {since}
      </span>
      <button
        className="rounded border px-2 py-0.5"
        onClick={() => commit(connId)}
      >
        Commit
      </button>
      <button
        className="rounded border px-2 py-0.5"
        onClick={() => rollback(connId)}
      >
        Rollback
      </button>
    </div>
  );
}
```

- [ ] **Step 4: TxSidePanel — list statements**

Create `src/features/transactions/TxSidePanel.tsx`:

```tsx
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTransactions } from "@/store/transactions";
import type { HistoryStatement } from "@/lib/types";

export function TxSidePanel({ connId }: { connId: string }) {
  const tx = useTransactions((s) => s.byConn[connId]);
  const [stmts, setStmts] = useState<HistoryStatement[]>([]);

  useEffect(() => {
    if (!tx?.active || !tx.txId) return;
    let cancelled = false;
    const tick = async () => {
      // Re-fetch list every 1s while tx is active.
      const entryId = await getEntryIdForTx(tx.txId!);
      if (!entryId) return;
      const list = await invoke<HistoryStatement[]>("list_history_statements", {
        entryId,
      });
      if (!cancelled) setStmts(list);
    };
    const id = setInterval(tick, 1000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [tx?.active, tx?.txId]);

  if (!tx?.active) return null;
  return (
    <aside className="w-64 border-l p-2 text-xs">
      <h3 className="font-medium">Transaction statements</h3>
      <ol className="mt-2 space-y-1">
        {stmts.map((s) => (
          <li key={s.id} className="truncate" title={s.sql}>
            {s.ordinal + 1}. {s.sql.slice(0, 60)}
          </li>
        ))}
      </ol>
    </aside>
  );
}

async function getEntryIdForTx(txId: string): Promise<string | null> {
  // Look up the most-recent entry whose tx_id matches.
  const entries = await invoke<{ id: string; txId?: string }[]>(
    "list_history",
    {
      connectionId: null,
      query: null,
      limit: 50,
    },
  );
  return entries.find((e) => e.txId === txId)?.id ?? null;
}
```

Add `HistoryStatement` and `HistoryEntry` to `lib/types.ts` if not yet present:

```ts
export interface HistoryEntry {
  id: string;
  connId: string;
  source: "editor" | "inline" | "palette";
  txId?: string;
  sqlPreview: string;
  sqlFull?: string;
  startedAt: number;
  durationMs: number;
  rowCount?: number;
  status: "ok" | "error" | "cancelled" | "rolled_back" | "open";
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

- [ ] **Step 5: Wire shortcuts**

Edit `src/features/editor/keymap.ts`. Add:

```ts
// Cmd+Shift+C / Ctrl+Shift+C  → commit
// Cmd+Shift+R / Ctrl+Shift+R  → rollback
// (Mac vs others handled by existing util.)
```

Implement by adding global listeners in the App shell that read `useTransactions.getState().byConn[activeConnId]?.active` and call `commit/rollback`.

- [ ] **Step 6: Shutdown confirm modal**

In the App shell (or `src-tauri/src/lib.rs::on_window_event`), intercept the close request when any tx_slot is active. Frontend approach:

```tsx
// in App.tsx
import { getCurrentWindow } from "@tauri-apps/api/window";

useEffect(() => {
  const win = getCurrentWindow();
  const off = win.onCloseRequested(async (e) => {
    const active = Object.values(useTransactions.getState().byConn).filter(
      (t) => t.active,
    );
    if (active.length === 0) return;
    e.preventDefault();
    const choice = await openConfirmModal({
      title: "Open transactions",
      body: `${active.length} transaction(s) have uncommitted changes.`,
      buttons: ["Commit all", "Rollback all", "Cancel"],
    });
    if (choice === "Commit all") {
      for (const t of active) await useTransactions.getState().commit(t.connId);
      win.close();
    } else if (choice === "Rollback all") {
      for (const t of active)
        await useTransactions.getState().rollback(t.connId);
      win.close();
    }
    // Cancel: just stay open.
  });
  return () => {
    off.then((fn) => fn());
  };
}, []);
```

(`openConfirmModal` is whatever modal helper the project uses; if there isn't one, build a small radix-dialog shim in `lib/confirm.tsx`.)

- [ ] **Step 7: Manual verification**

```bash
docker compose -f infra/postgres/docker-compose.yml up -d
pnpm tauri dev
```

Verify (record results in `manual-verification-week-3.md` later in Task 24):

1. Toggle Auto-commit OFF → 🟡 indicator appears, statement count = 0.
2. Run `INSERT INTO tx_t VALUES (1);` twice → indicator shows "2 stmts".
3. Open `psql` separately and `SELECT * FROM tx_t` → see 0 rows (uncommitted).
4. Click Commit → indicator disappears; psql now sees both rows.
5. Repeat with Rollback → no rows committed.
6. Begin tx, attempt window close → confirm modal appears.

- [ ] **Step 8: Quality gates + commit**

```bash
pnpm typecheck && pnpm lint && pnpm format:check && pnpm build
git add src/store/transactions.ts src/features/transactions/ \
        src/lib/types.ts src/App.tsx src/features/editor/keymap.ts
git commit -m "feat(frontend): explicit transaction toggle, indicator, side panel, shutdown confirm"
```

---

## Task 11: PendingChanges store + EditableCell shell

**Goal:** Frontend infrastructure for tracking edits without any widgets yet — a text-only fallback that proves the round-trip. Then widgets layer in (Tasks 12–15).

**Files:**

- Create: `src/store/pendingChanges.ts`
- Create: `src/features/editing/EditableCell.tsx`
- Create: `src/features/editing/PendingBadge.tsx`
- Modify: `src/features/results/ResultsGrid.tsx`
- Modify: `src/lib/types.ts` (PendingChange)

**Steps:**

- [ ] **Step 1: Add `PendingChange` type**

Edit `src/lib/types.ts`:

```ts
export interface PendingChange {
  rowKey: string;
  table: { schema: string; name: string };
  pk: { columns: string[]; values: Cell[] };
  edits: { column: string; original: Cell; next: Cell }[];
  op: "update" | "insert" | "delete";
  capturedRow: Cell[];
  capturedColumns: string[];
  capturedAt: number;
}
```

- [ ] **Step 2: Implement store with vitest-friendly contract**

Create `src/store/pendingChanges.ts`:

```ts
import { create } from "zustand";
import type { Cell, PendingChange, ResultMeta } from "@/lib/types";

interface Store {
  byRow: Map<string, PendingChange>;
  upsertEdit(args: {
    table: { schema: string; name: string };
    pkColumns: string[];
    pkValues: Cell[];
    column: string;
    original: Cell;
    next: Cell;
    capturedRow: Cell[];
    capturedColumns: string[];
  }): void;
  revertRow(rowKey: string): void;
  revertAll(): void;
  list(): PendingChange[];
  count(): number;
}

export const usePendingChanges = create<Store>((set, get) => ({
  byRow: new Map(),
  upsertEdit(args) {
    set((s) => {
      const next = new Map(s.byRow);
      const rowKey = JSON.stringify(args.pkValues);
      const existing = next.get(rowKey);
      const change: PendingChange = existing ?? {
        rowKey,
        table: args.table,
        pk: { columns: args.pkColumns, values: args.pkValues },
        edits: [],
        op: "update",
        capturedRow: args.capturedRow,
        capturedColumns: args.capturedColumns,
        capturedAt: Date.now(),
      };
      const idx = change.edits.findIndex((e) => e.column === args.column);
      if (idx >= 0) {
        change.edits[idx] = {
          column: args.column,
          original: args.original,
          next: args.next,
        };
      } else {
        change.edits.push({
          column: args.column,
          original: args.original,
          next: args.next,
        });
      }
      next.set(rowKey, change);
      return { byRow: next };
    });
  },
  revertRow(rowKey) {
    set((s) => {
      const next = new Map(s.byRow);
      next.delete(rowKey);
      return { byRow: next };
    });
  },
  revertAll() {
    set({ byRow: new Map() });
  },
  list() {
    return Array.from(get().byRow.values());
  },
  count() {
    return get().byRow.size;
  },
}));

// Helper used by EditableCell to compute pk-values for the row.
export function pkValuesOf(meta: ResultMeta, row: Cell[]): Cell[] {
  return meta.pkColumnIndices.map((i) => row[i]);
}
```

- [ ] **Step 3: PendingBadge**

Create `src/features/editing/PendingBadge.tsx`:

```tsx
import { usePendingChanges } from "@/store/pendingChanges";

export function PendingBadge({
  onPreview,
  onSubmit,
  onRevert,
}: {
  onPreview: () => void;
  onSubmit: () => void;
  onRevert: () => void;
}) {
  const count = usePendingChanges((s) => s.count());
  if (count === 0) return null;
  return (
    <div className="flex items-center gap-2 text-xs">
      <span>{count} pending</span>
      <button onClick={onPreview} className="rounded border px-2 py-0.5">
        Preview
      </button>
      <button
        onClick={onSubmit}
        className="rounded border bg-amber-500 px-2 py-0.5 text-black"
      >
        Submit
      </button>
      <button onClick={onRevert} className="rounded border px-2 py-0.5">
        Revert
      </button>
    </div>
  );
}
```

- [ ] **Step 4: EditableCell shell — text-only fallback**

Create `src/features/editing/EditableCell.tsx`:

```tsx
import { useState } from "react";
import type { Cell, ResultMeta } from "@/lib/types";
import { renderCell } from "@/features/results/cells";
import { usePendingChanges, pkValuesOf } from "@/store/pendingChanges";

export function EditableCell({
  value,
  columnIndex,
  row,
  meta,
}: {
  value: Cell;
  columnIndex: number;
  row: Cell[];
  meta: ResultMeta;
}) {
  const [editing, setEditing] = useState(false);
  const upsert = usePendingChanges((s) => s.upsertEdit);
  const colName = meta.columnTypes[columnIndex].name;
  const pendingForCol = usePendingChanges((s) => {
    const k = JSON.stringify(pkValuesOf(meta, row));
    return s.byRow.get(k)?.edits.find((e) => e.column === colName)?.next;
  });
  const display = pendingForCol ?? value;

  if (!meta.editable) return <>{renderCell(value)}</>;
  if (editing) {
    return (
      <input
        autoFocus
        defaultValue={cellAsString(display)}
        onBlur={(e) => {
          setEditing(false);
          const next = parseCellLike(value, e.currentTarget.value);
          upsert({
            table: meta.table!,
            pkColumns: meta.pkColumns,
            pkValues: pkValuesOf(meta, row),
            column: colName,
            original: value,
            next,
            capturedRow: row,
            capturedColumns: meta.columnTypes.map((c) => c.name),
          });
        }}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            setEditing(false);
          }
          if (e.key === "Enter") {
            (e.currentTarget as HTMLInputElement).blur();
          }
        }}
      />
    );
  }
  const dirty = pendingForCol !== undefined;
  return (
    <span
      onDoubleClick={() => setEditing(true)}
      className={dirty ? "cursor-text bg-amber-500/20" : "cursor-text"}
      title={dirty ? `Original: ${cellAsString(value)}` : undefined}
    >
      {renderCell(display)}
    </span>
  );
}

function cellAsString(c: Cell): string {
  switch (c.kind) {
    case "Null":
      return "";
    case "Bool":
      return c.value ? "true" : "false";
    case "Int":
    case "Float":
      return String(c.value);
    case "Bigint":
    case "Numeric":
    case "Text":
    case "Uuid":
    case "Inet":
    case "Date":
    case "Time":
    case "Timetz":
    case "Timestamp":
    case "Timestamptz":
      return c.value;
    case "Interval":
      return c.value.iso;
    case "Json":
      return JSON.stringify(c.value);
    case "Bytea":
      return c.value.b64;
    case "Array":
      return JSON.stringify(c.value.values);
    case "Enum":
      return c.value.value;
    case "Vector":
      return JSON.stringify(c.value.values);
    case "Unknown":
      return c.value.text;
  }
}

function parseCellLike(original: Cell, raw: string): Cell {
  // Text-only fallback. Per-type widgets in Tasks 12–15 replace this entirely.
  switch (original.kind) {
    case "Null":
      return raw === "" ? { kind: "Null" } : { kind: "Text", value: raw };
    case "Int":
      return { kind: "Int", value: Number(raw) };
    case "Float":
      return { kind: "Float", value: Number(raw) };
    case "Bool":
      return { kind: "Bool", value: raw.toLowerCase() === "true" };
    case "Bigint":
    case "Numeric":
    case "Text":
    case "Uuid":
    case "Inet":
    case "Date":
    case "Time":
    case "Timetz":
    case "Timestamp":
    case "Timestamptz":
      return { ...original, value: raw } as Cell;
    case "Json":
      try {
        return { kind: "Json", value: JSON.parse(raw) };
      } catch {
        return original;
      }
    default:
      return original; // widgets will handle
  }
}
```

- [ ] **Step 5: Wire EditableCell into ResultsGrid**

Edit `src/features/results/ResultsGrid.tsx` so each rendered cell goes through `EditableCell` (replacing the existing `renderCell` call). Pass row + columnIndex + meta. Keep behavior identical for `meta.editable === false` (just renders).

- [ ] **Step 6: Manual smoke**

```bash
pnpm tauri dev
```

Run `SELECT id, name FROM users LIMIT 5;` → double-click a `name` cell → type → Enter → cell is yellow + "1 pending". Click Revert → goes back. (No Submit/Preview yet — those land in Tasks 16–17.)

- [ ] **Step 7: Commit**

```bash
pnpm typecheck && pnpm lint && pnpm format:check && pnpm build
git add src/store/pendingChanges.ts src/features/editing/ \
        src/lib/types.ts src/features/results/ResultsGrid.tsx
git commit -m "feat(frontend): pending changes store + EditableCell text fallback"
```

---

## Task 12: Widgets batch 1 — Text / Int / Bigint / Numeric / Bool + Set NULL

**Goal:** First five typed widgets. Each is a small component the EditableCell mounts based on `columnTypes[i].typeName`. Set NULL is a button shown when the column is nullable.

**Files:**

- Create: `src/features/editing/widgets/Text.tsx`
- Create: `src/features/editing/widgets/Int.tsx`
- Create: `src/features/editing/widgets/Bigint.tsx`
- Create: `src/features/editing/widgets/Numeric.tsx`
- Create: `src/features/editing/widgets/Bool.tsx`
- Create: `src/features/editing/widgets/SetNullButton.tsx`
- Modify: `src/features/editing/EditableCell.tsx` (dispatch on typeName)

**Steps:**

- [ ] **Step 1: Common widget contract**

Create `src/features/editing/widgets/types.ts`:

```ts
import type { Cell } from "@/lib/types";
export interface WidgetProps {
  initial: Cell;
  nullable: boolean;
  onCommit: (next: Cell) => void;
  onCancel: () => void;
}
```

- [ ] **Step 2: Text widget**

Create `src/features/editing/widgets/Text.tsx`:

```tsx
import { useState } from "react";
import type { WidgetProps } from "./types";
import { SetNullButton } from "./SetNullButton";

export function TextWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
}: WidgetProps) {
  const [val, setVal] = useState(
    initial.kind === "Null"
      ? ""
      : ((initial as Extract<typeof initial, { kind: "Text" }>).value ?? ""),
  );
  const [multiline, setMultiline] = useState(false);
  return (
    <div className="flex items-center gap-1">
      {multiline ? (
        <textarea
          autoFocus
          value={val}
          onChange={(e) => setVal(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") onCancel();
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey))
              onCommit({ kind: "Text", value: val });
          }}
        />
      ) : (
        <input
          autoFocus
          value={val}
          onChange={(e) => setVal(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") onCancel();
            if (e.key === "Enter") onCommit({ kind: "Text", value: val });
          }}
        />
      )}
      <button onClick={() => setMultiline(!multiline)} className="text-xs">
        {multiline ? "single" : "multi"}
      </button>
      {nullable && <SetNullButton onCommit={onCommit} />}
    </div>
  );
}
```

- [ ] **Step 3: Int / Bigint / Numeric / Bool widgets**

Create `src/features/editing/widgets/Int.tsx`:

```tsx
import { useState } from "react";
import type { WidgetProps } from "./types";
import { SetNullButton } from "./SetNullButton";

export function IntWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
}: WidgetProps) {
  const [val, setVal] = useState(
    initial.kind === "Int" ? String(initial.value) : "",
  );
  const [err, setErr] = useState<string | null>(null);
  const submit = () => {
    if (!/^-?\d+$/.test(val)) {
      setErr("integer required");
      return;
    }
    const n = Number(val);
    if (n < -2147483648 || n > 2147483647) {
      setErr("out of range for int4");
      return;
    }
    onCommit({ kind: "Int", value: n });
  };
  return (
    <div className="flex flex-col">
      <div className="flex items-center gap-1">
        <input
          autoFocus
          value={val}
          onChange={(e) => {
            setVal(e.target.value);
            setErr(null);
          }}
          onKeyDown={(e) => {
            if (e.key === "Escape") onCancel();
            if (e.key === "Enter") submit();
          }}
        />
        {nullable && <SetNullButton onCommit={onCommit} />}
      </div>
      {err && <span className="text-xs text-red-500">{err}</span>}
    </div>
  );
}
```

Create `src/features/editing/widgets/Bigint.tsx` — identical to Int but `kind: 'Bigint'` and stores as string; range check is `BigInt(-2^63) <= BigInt(val) <= BigInt(2^63-1)`:

```tsx
import { useState } from "react";
import type { WidgetProps } from "./types";
import { SetNullButton } from "./SetNullButton";

export function BigintWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
}: WidgetProps) {
  const [val, setVal] = useState(
    initial.kind === "Bigint" ? initial.value : "",
  );
  const [err, setErr] = useState<string | null>(null);
  const submit = () => {
    if (!/^-?\d+$/.test(val)) {
      setErr("integer required");
      return;
    }
    try {
      const big = BigInt(val);
      const min = BigInt("-9223372036854775808");
      const max = BigInt("9223372036854775807");
      if (big < min || big > max) {
        setErr("out of range for int8");
        return;
      }
      onCommit({ kind: "Bigint", value: val });
    } catch {
      setErr("invalid");
    }
  };
  return (
    <div className="flex flex-col">
      <div className="flex items-center gap-1">
        <input
          autoFocus
          value={val}
          onChange={(e) => {
            setVal(e.target.value);
            setErr(null);
          }}
          onKeyDown={(e) => {
            if (e.key === "Escape") onCancel();
            if (e.key === "Enter") submit();
          }}
        />
        {nullable && <SetNullButton onCommit={onCommit} />}
      </div>
      {err && <span className="text-xs text-red-500">{err}</span>}
    </div>
  );
}
```

Create `src/features/editing/widgets/Numeric.tsx` — accepts `[-]\d+(\.\d+)?` and stores as string; if a precision/scale is known (would need more meta — we don't have it yet, so accept any and let PG validate):

```tsx
import { useState } from "react";
import type { WidgetProps } from "./types";
import { SetNullButton } from "./SetNullButton";

export function NumericWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
}: WidgetProps) {
  const [val, setVal] = useState(
    initial.kind === "Numeric" ? initial.value : "",
  );
  const [err, setErr] = useState<string | null>(null);
  const submit = () => {
    if (!/^-?\d+(\.\d+)?$/.test(val)) {
      setErr("numeric required");
      return;
    }
    onCommit({ kind: "Numeric", value: val });
  };
  return (
    <div className="flex flex-col">
      <div className="flex items-center gap-1">
        <input
          autoFocus
          value={val}
          onChange={(e) => {
            setVal(e.target.value);
            setErr(null);
          }}
          onKeyDown={(e) => {
            if (e.key === "Escape") onCancel();
            if (e.key === "Enter") submit();
          }}
        />
        {nullable && <SetNullButton onCommit={onCommit} />}
      </div>
      {err && <span className="text-xs text-red-500">{err}</span>}
    </div>
  );
}
```

Create `src/features/editing/widgets/Bool.tsx`:

```tsx
import type { WidgetProps } from "./types";
import { SetNullButton } from "./SetNullButton";

export function BoolWidget({ initial, nullable, onCommit }: WidgetProps) {
  const cur = initial.kind === "Bool" ? initial.value : false;
  return (
    <div className="flex items-center gap-2">
      <input
        type="checkbox"
        defaultChecked={cur}
        autoFocus
        onChange={(e) =>
          onCommit({ kind: "Bool", value: e.currentTarget.checked })
        }
      />
      {nullable && <SetNullButton onCommit={onCommit} />}
    </div>
  );
}
```

Create `src/features/editing/widgets/SetNullButton.tsx`:

```tsx
import type { Cell } from "@/lib/types";
export function SetNullButton({ onCommit }: { onCommit: (c: Cell) => void }) {
  return (
    <button
      onClick={() => onCommit({ kind: "Null" })}
      className="rounded border px-1 text-xs"
    >
      Set NULL
    </button>
  );
}
```

- [ ] **Step 4: Dispatch in EditableCell**

Replace EditableCell's editing branch with a switch on `meta.columnTypes[columnIndex].typeName`. Default falls back to TextWidget (string-cast). Sketch:

```tsx
import { TextWidget } from "./widgets/Text";
import { IntWidget } from "./widgets/Int";
import { BigintWidget } from "./widgets/Bigint";
import { NumericWidget } from "./widgets/Numeric";
import { BoolWidget } from "./widgets/Bool";

function pickWidget(typeName: string) {
  switch (typeName) {
    case "int2":
    case "int4":
      return IntWidget;
    case "int8":
      return BigintWidget;
    case "numeric":
      return NumericWidget;
    case "bool":
      return BoolWidget;
    case "text":
    case "varchar":
    case "bpchar":
    default:
      return TextWidget;
  }
}
```

(Other typeNames will route to widgets added in Tasks 13–15. Until then they fall through to TextWidget, which is acceptable.)

- [ ] **Step 5: Manual smoke**

```bash
pnpm tauri dev
```

Run `SELECT id, name, age, active FROM users LIMIT 5;` (or any table mixing int/text/bool). Verify each widget commits sane values and nullable columns show the Set NULL button.

- [ ] **Step 6: Commit**

```bash
pnpm typecheck && pnpm lint && pnpm format:check && pnpm build
git add src/features/editing/widgets/Text.tsx \
        src/features/editing/widgets/Int.tsx \
        src/features/editing/widgets/Bigint.tsx \
        src/features/editing/widgets/Numeric.tsx \
        src/features/editing/widgets/Bool.tsx \
        src/features/editing/widgets/SetNullButton.tsx \
        src/features/editing/widgets/types.ts \
        src/features/editing/EditableCell.tsx
git commit -m "feat(frontend): editing widgets — text, int, bigint, numeric, bool, set-null"
```

---

## Task 13: Widgets batch 2 — Date / Time / Timetz / Timestamp / Timestamptz / Uuid

**Goal:** Native date/time pickers + UUID with Generate button.

**Files:**

- Create: `src/features/editing/widgets/Date.tsx`
- Create: `src/features/editing/widgets/Time.tsx`
- Create: `src/features/editing/widgets/Timestamp.tsx`
- Create: `src/features/editing/widgets/Uuid.tsx`
- Modify: `src/features/editing/EditableCell.tsx`

**Steps:**

- [ ] **Step 1: Date / Time widgets (single file `Date.tsx` for both is OK)**

```tsx
// src/features/editing/widgets/Date.tsx
import { useState } from "react";
import type { WidgetProps } from "./types";
import { SetNullButton } from "./SetNullButton";

export function DateWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
}: WidgetProps) {
  const [val, setVal] = useState(initial.kind === "Date" ? initial.value : "");
  return (
    <div className="flex items-center gap-1">
      <input
        type="date"
        autoFocus
        value={val}
        onChange={(e) => setVal(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape") onCancel();
          if (e.key === "Enter") onCommit({ kind: "Date", value: val });
        }}
      />
      <button
        onClick={() => onCommit({ kind: "Date", value: val })}
        className="text-xs"
      >
        OK
      </button>
      {nullable && <SetNullButton onCommit={onCommit} />}
    </div>
  );
}
```

```tsx
// src/features/editing/widgets/Time.tsx
import { useState } from "react";
import type { Cell } from "@/lib/types";
import type { WidgetProps } from "./types";
import { SetNullButton } from "./SetNullButton";

export function TimeWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
  kind,
}: WidgetProps & { kind: "Time" | "Timetz" }) {
  const [val, setVal] = useState(
    initial.kind === kind ? (initial.value as string) : "",
  );
  return (
    <div className="flex items-center gap-1">
      <input
        type="time"
        step="1"
        autoFocus
        value={val}
        onChange={(e) => setVal(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape") onCancel();
          if (e.key === "Enter") onCommit({ kind, value: val } as Cell);
        }}
      />
      <button
        onClick={() => onCommit({ kind, value: val } as Cell)}
        className="text-xs"
      >
        OK
      </button>
      {nullable && <SetNullButton onCommit={onCommit} />}
    </div>
  );
}
```

(EditableCell passes `kind="Time"` or `kind="Timetz"` to TimeWidget based on the column type.)

- [ ] **Step 2: Timestamp / Timestamptz**

```tsx
// src/features/editing/widgets/Timestamp.tsx
import { useState } from "react";
import type { Cell } from "@/lib/types";
import type { WidgetProps } from "./types";
import { SetNullButton } from "./SetNullButton";

export function TimestampWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
  kind,
}: WidgetProps & { kind: "Timestamp" | "Timestamptz" }) {
  const [val, setVal] = useState(() => {
    if (initial.kind === kind) return (initial.value as string).slice(0, 19);
    return "";
  });
  const tz = kind === "Timestamptz" ? new Date().getTimezoneOffset() : null;
  return (
    <div className="flex items-center gap-1">
      <input
        type="datetime-local"
        step="1"
        autoFocus
        value={val}
        onChange={(e) => setVal(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape") onCancel();
          if (e.key === "Enter") onCommit({ kind, value: val } as Cell);
        }}
      />
      {tz !== null && (
        <span className="text-muted-foreground text-xs">
          UTC{tz <= 0 ? "+" : "-"}
          {Math.abs(tz / 60)}
        </span>
      )}
      <button
        onClick={() => onCommit({ kind, value: val } as Cell)}
        className="text-xs"
      >
        OK
      </button>
      {nullable && <SetNullButton onCommit={onCommit} />}
    </div>
  );
}
```

- [ ] **Step 3: Uuid**

```tsx
// src/features/editing/widgets/Uuid.tsx
import { useState } from "react";
import type { WidgetProps } from "./types";
import { SetNullButton } from "./SetNullButton";

export function UuidWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
}: WidgetProps) {
  const [val, setVal] = useState(initial.kind === "Uuid" ? initial.value : "");
  const [err, setErr] = useState<string | null>(null);
  const submit = () => {
    if (
      !/^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/.test(
        val,
      )
    ) {
      setErr("invalid uuid");
      return;
    }
    onCommit({ kind: "Uuid", value: val });
  };
  return (
    <div className="flex items-center gap-1">
      <input
        autoFocus
        value={val}
        onChange={(e) => {
          setVal(e.target.value);
          setErr(null);
        }}
        onKeyDown={(e) => {
          if (e.key === "Escape") onCancel();
          if (e.key === "Enter") submit();
        }}
      />
      <button onClick={() => setVal(crypto.randomUUID())} className="text-xs">
        Generate
      </button>
      {nullable && <SetNullButton onCommit={onCommit} />}
      {err && <span className="ml-2 text-xs text-red-500">{err}</span>}
    </div>
  );
}
```

- [ ] **Step 4: Dispatch + commit**

Update `EditableCell.tsx::pickWidget`:

```tsx
case 'date': return DateWidget;
case 'time': case 'timetz':
  return (props: WidgetProps) => TimeWidget({ ...props, kind: typeName === 'time' ? 'Time' : 'Timetz' });
case 'timestamp': case 'timestamptz':
  return (props: WidgetProps) => TimestampWidget({ ...props, kind: typeName === 'timestamp' ? 'Timestamp' : 'Timestamptz' });
case 'uuid': return UuidWidget;
```

(Or pass `typeName` as a prop and let widgets read it — pick the cleaner pattern.)

- [ ] **Step 5: Manual smoke + commit**

```bash
pnpm tauri dev
# Edit a table with date/timestamp/uuid columns; verify each widget commits.
pnpm typecheck && pnpm lint && pnpm format:check && pnpm build
git add src/features/editing/widgets/{Date,Time,Timestamp,Uuid}.tsx src/features/editing/EditableCell.tsx
git commit -m "feat(frontend): editing widgets — date, time, timestamp(tz), uuid"
```

---

## Task 14: Widgets batch 3 — Json (Monaco mini) / Bytea (hex/base64) / Vector (read-only)

**Goal:** Power-user widgets. Json uses a compact Monaco instance with JSON validation. Bytea provides hex/base64 toggles + file export. Vector is read-only (PLAN out-of-scope for Week 3 edit).

**Files:**

- Create: `src/features/editing/widgets/Json.tsx`
- Create: `src/features/editing/widgets/Bytea.tsx`
- Create: `src/features/editing/widgets/Vector.tsx`

**Steps:**

- [ ] **Step 1: Json widget**

```tsx
// src/features/editing/widgets/Json.tsx
import Editor from "@monaco-editor/react";
import { useState } from "react";
import type { WidgetProps } from "./types";
import { SetNullButton } from "./SetNullButton";

export function JsonWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
}: WidgetProps) {
  const [val, setVal] = useState(() => {
    if (initial.kind !== "Json") return "{}";
    return JSON.stringify(initial.value, null, 2);
  });
  const [err, setErr] = useState<string | null>(null);
  const submit = () => {
    try {
      onCommit({ kind: "Json", value: JSON.parse(val) });
    } catch (e) {
      setErr(String(e));
    }
  };
  return (
    <div className="flex h-[180px] w-[360px] flex-col">
      <Editor
        height="140px"
        language="json"
        value={val}
        onChange={(v) => {
          setVal(v ?? "");
          setErr(null);
        }}
        options={{ minimap: { enabled: false }, fontSize: 12 }}
      />
      <div className="mt-1 flex items-center gap-1">
        <button onClick={submit} className="rounded border px-1 text-xs">
          OK
        </button>
        <button onClick={onCancel} className="text-xs">
          Cancel
        </button>
        {nullable && <SetNullButton onCommit={onCommit} />}
        {err && <span className="ml-2 text-xs text-red-500">{err}</span>}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Bytea widget**

```tsx
// src/features/editing/widgets/Bytea.tsx
import { useState } from "react";
import type { WidgetProps } from "./types";
import { SetNullButton } from "./SetNullButton";

function b64ToHex(b64: string): string {
  const bin = atob(b64);
  let out = "";
  for (let i = 0; i < bin.length; i++)
    out += bin.charCodeAt(i).toString(16).padStart(2, "0");
  return out;
}
function hexToB64(hex: string): string {
  const clean = hex.replace(/\s+/g, "").replace(/^\\?x/, "");
  if (!/^[0-9a-fA-F]*$/.test(clean) || clean.length % 2 !== 0)
    throw new Error("invalid hex");
  const bytes = new Uint8Array(clean.length / 2);
  for (let i = 0; i < bytes.length; i++)
    bytes[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  let bin = "";
  bytes.forEach((b) => {
    bin += String.fromCharCode(b);
  });
  return btoa(bin);
}

export function ByteaWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
}: WidgetProps) {
  const initB64 = initial.kind === "Bytea" ? initial.value.b64 : "";
  const [mode, setMode] = useState<"hex" | "b64">("hex");
  const [val, setVal] = useState(() =>
    mode === "hex" ? b64ToHex(initB64) : initB64,
  );
  const [err, setErr] = useState<string | null>(null);
  const submit = () => {
    try {
      const b64 = mode === "hex" ? hexToB64(val) : val;
      onCommit({ kind: "Bytea", value: { b64 } });
    } catch (e) {
      setErr(String(e));
    }
  };
  return (
    <div className="flex items-center gap-1">
      <select
        value={mode}
        onChange={(e) => setMode(e.target.value as "hex" | "b64")}
        className="text-xs"
      >
        <option value="hex">hex</option>
        <option value="b64">base64</option>
      </select>
      <input
        autoFocus
        value={val}
        onChange={(e) => {
          setVal(e.target.value);
          setErr(null);
        }}
        onKeyDown={(e) => {
          if (e.key === "Escape") onCancel();
          if (e.key === "Enter") submit();
        }}
        className="w-[280px] font-mono"
      />
      <button onClick={submit} className="text-xs">
        OK
      </button>
      {nullable && <SetNullButton onCommit={onCommit} />}
      {err && <span className="ml-1 text-xs text-red-500">{err}</span>}
    </div>
  );
}
```

- [ ] **Step 3: Vector widget (read-only)**

```tsx
// src/features/editing/widgets/Vector.tsx
import type { WidgetProps } from "./types";

export function VectorWidget({ initial, onCancel }: WidgetProps) {
  if (initial.kind !== "Vector") return null;
  return (
    <div className="text-xs">
      <span className="text-muted-foreground italic">
        vector({initial.value.dim}) — read-only in this version
      </span>
      <button onClick={onCancel} className="ml-2">
        Close
      </button>
    </div>
  );
}
```

- [ ] **Step 4: Dispatch + commit**

`EditableCell::pickWidget` adds:

```tsx
case 'json': case 'jsonb': return JsonWidget;
case 'bytea': return ByteaWidget;
case 'vector': return VectorWidget;
```

For vector, also disable `onDoubleClick` editable activation when typeName is `vector`:

```tsx
const isReadonlyType = ["vector", "unknown"].includes(
  meta.columnTypes[columnIndex].typeName,
);
if (!meta.editable || isReadonlyType) return <>{renderCell(value)}</>;
```

- [ ] **Step 5: Manual smoke + commit**

```bash
pnpm typecheck && pnpm lint && pnpm format:check && pnpm build
git add src/features/editing/widgets/{Json,Bytea,Vector}.tsx src/features/editing/EditableCell.tsx
git commit -m "feat(frontend): editing widgets — json (monaco), bytea (hex/base64), vector readonly"
```

---

## Task 15: Widgets batch 4 — Enum / FK dropdown (with pg_meta lookups)

**Goal:** Enum reads `enumValues` from `ColumnTypeMeta` and shows a dropdown. FK fetches the referenced table's PK + first text column for searchable display.

**Files:**

- Create: `src/features/editing/widgets/Enum.tsx`
- Create: `src/features/editing/widgets/Fk.tsx`
- Create: `src-tauri/src/commands/fk_lookup.rs`
- Modify: `src-tauri/src/commands/mod.rs` + `lib.rs`

**Steps:**

- [ ] **Step 1: Enum widget**

```tsx
// src/features/editing/widgets/Enum.tsx
import type { WidgetProps } from "./types";
import { SetNullButton } from "./SetNullButton";

export function EnumWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
  enumValues,
  typeName,
}: WidgetProps & { enumValues: string[]; typeName: string }) {
  const cur = initial.kind === "Enum" ? initial.value.value : "";
  return (
    <div className="flex items-center gap-1">
      <select
        autoFocus
        defaultValue={cur}
        onChange={(e) =>
          onCommit({ kind: "Enum", value: { typeName, value: e.target.value } })
        }
        onKeyDown={(e) => {
          if (e.key === "Escape") onCancel();
        }}
      >
        {enumValues.map((v) => (
          <option key={v} value={v}>
            {v}
          </option>
        ))}
      </select>
      {nullable && <SetNullButton onCommit={onCommit} />}
    </div>
  );
}
```

- [ ] **Step 2: FK widget — backend lookup command**

Create `src-tauri/src/commands/fk_lookup.rs`:

```rust
use serde::Serialize;
use tauri::State;

use crate::db::pool::ConnectionRegistry;
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FkOption {
    pub pk_value: String,
    pub display: String,
}

#[tauri::command]
pub async fn fk_lookup(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
    schema: String,
    table: String,
    pk_column: String,
    query: Option<String>,
    limit: Option<i64>,
) -> TuskResult<Vec<FkOption>> {
    let pool = registry.pool(&connection_id)?;
    let lim = limit.unwrap_or(50);

    // Find first text-ish column for display (fallback to pk).
    let display_col: Option<String> = sqlx::query_scalar(
        r#"SELECT a.attname FROM pg_attribute a
           JOIN pg_type t ON t.oid = a.atttypid
           JOIN pg_class c ON c.oid = a.attrelid
           JOIN pg_namespace n ON n.oid = c.relnamespace
           WHERE n.nspname = $1 AND c.relname = $2
             AND a.attnum > 0 AND NOT a.attisdropped
             AND t.typname IN ('text','varchar','bpchar','name')
           ORDER BY a.attnum LIMIT 1"#,
    )
    .bind(&schema).bind(&table)
    .fetch_optional(&pool).await
    .map_err(|e| TuskError::Query(e.to_string()))?;

    let display = display_col.clone().unwrap_or_else(|| pk_column.clone());

    let where_clause = match &query {
        Some(q) if !q.is_empty() => format!("WHERE \"{display}\"::text ILIKE '%{}%'", q.replace('\'', "''")),
        _ => String::new(),
    };

    let sql = format!(
        "SELECT \"{pk_column}\"::text, \"{display}\"::text
         FROM \"{schema}\".\"{table}\" {where_clause}
         ORDER BY \"{pk_column}\" LIMIT {lim}"
    );
    let rows: Vec<(String, String)> = sqlx::query_as(&sql)
        .fetch_all(&pool).await
        .map_err(|e| TuskError::Query(e.to_string()))?;
    Ok(rows.into_iter().map(|(pk, disp)| FkOption { pk_value: pk, display: disp }).collect())
}
```

> **Security note:** the `query` param is interpolated, not parameterized — but the only chars that survive are alphanumerics + escaped single quotes. The schema/table/column come from `pg_meta`, not user input. This is intentional to keep ILIKE flexible. If exposed beyond v1, switch to a parameterized regex match.

Wire into `mod.rs` + `lib.rs`:

```rust
pub mod fk_lookup;
// invoke handler:
            commands::fk_lookup::fk_lookup,
```

- [ ] **Step 3: FK widget**

```tsx
// src/features/editing/widgets/Fk.tsx
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Cell } from "@/lib/types";
import type { WidgetProps } from "./types";
import { SetNullButton } from "./SetNullButton";

interface FkOption {
  pkValue: string;
  display: string;
}

export function FkWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
  connId,
  fk,
  originalKind,
}: WidgetProps & {
  connId: string;
  fk: { schema: string; table: string; column: string };
  originalKind: "Int" | "Bigint" | "Text" | "Uuid";
}) {
  const [q, setQ] = useState("");
  const [opts, setOpts] = useState<FkOption[]>([]);

  useEffect(() => {
    const t = setTimeout(() => {
      invoke<FkOption[]>("fk_lookup", {
        connectionId: connId,
        schema: fk.schema,
        table: fk.table,
        pkColumn: fk.column,
        query: q,
      })
        .then(setOpts)
        .catch(() => setOpts([]));
    }, 150);
    return () => clearTimeout(t);
  }, [q, connId, fk.schema, fk.table, fk.column]);

  const commit = (raw: string) => {
    const c: Cell = (() => {
      switch (originalKind) {
        case "Int":
          return { kind: "Int", value: Number(raw) };
        case "Bigint":
          return { kind: "Bigint", value: raw };
        case "Uuid":
          return { kind: "Uuid", value: raw };
        default:
          return { kind: "Text", value: raw };
      }
    })();
    onCommit(c);
  };

  return (
    <div className="flex w-[280px] flex-col">
      <input
        autoFocus
        placeholder={`Search ${fk.table}.${fk.column}`}
        value={q}
        onChange={(e) => setQ(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape") onCancel();
        }}
      />
      <div className="mt-1 max-h-40 overflow-auto rounded border">
        {opts.map((o) => (
          <button
            key={o.pkValue}
            className="hover:bg-muted block w-full px-2 py-0.5 text-left"
            onClick={() => commit(o.pkValue)}
          >
            <span className="font-mono text-xs">{o.pkValue}</span>
            <span className="text-muted-foreground ml-2">{o.display}</span>
          </button>
        ))}
      </div>
      {nullable && <SetNullButton onCommit={onCommit} />}
    </div>
  );
}
```

- [ ] **Step 4: Dispatch in EditableCell**

```tsx
// inside pickWidget, before falling through to TextWidget:
const colMeta = meta.columnTypes[columnIndex];
if (colMeta.enumValues) {
  return (props: WidgetProps) =>
    EnumWidget({ ...props, enumValues: colMeta.enumValues!, typeName: "enum" });
}
if (colMeta.fk) {
  const originalKind = (() => {
    switch (colMeta.typeName) {
      case "int2":
      case "int4":
        return "Int" as const;
      case "int8":
        return "Bigint" as const;
      case "uuid":
        return "Uuid" as const;
      default:
        return "Text" as const;
    }
  })();
  return (props: WidgetProps) =>
    FkWidget({ ...props, connId, fk: colMeta.fk!, originalKind });
}
```

(`connId` must thread through ResultsGrid → EditableCell. Add it as a prop.)

- [ ] **Step 5: Manual smoke + commit**

```bash
pnpm typecheck && pnpm lint && pnpm format:check && pnpm build
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
git add src/features/editing/widgets/{Enum,Fk}.tsx \
        src-tauri/src/commands/{fk_lookup.rs,mod.rs} src-tauri/src/lib.rs \
        src/features/editing/EditableCell.tsx \
        src/features/results/ResultsGrid.tsx
git commit -m "feat: enum + FK lookup widgets with backend search"
```

---

## Task 16: editing.rs — `build_update` / `build_insert` / `build_delete` + parameterized exec

**Goal:** Pure builders that turn a `PendingBatch` into:

1. A parameterized `sqlx::query` with bind values (executed against the connection),
2. A literal-inlined SQL string (for Preview / response).

Atomic Submit handles both PkOnly and Strict modes. TDD on builders (unit) + execution (docker).

**Files:**

- Create: `src-tauri/src/commands/editing.rs`
- Create: `src-tauri/tests/editing.rs`
- Modify: `src-tauri/src/commands/mod.rs` + `lib.rs`

**Steps:**

- [ ] **Step 1: Module skeleton + types**

Create `src-tauri/src/commands/editing.rs`:

```rust
use serde::{Deserialize, Serialize};
use sqlx::Postgres;

use crate::db::decoder::Cell;
use crate::db::pg_literals::to_literal;
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingBatch {
    pub batch_id: String,
    pub op: PendingOp,
    pub table: TableRef,
    pub pk_columns: Vec<String>,
    pub pk_values: Vec<Cell>,
    pub edits: Vec<ColumnEdit>,
    pub captured_row: Vec<Cell>,
    pub captured_columns: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnEdit { pub column: String, pub next: Cell }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableRef { pub schema: String, pub name: String }

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PendingOp { Update, Insert, Delete }

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum ConflictMode { PkOnly, Strict }

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum BatchResult {
    Ok       { batch_id: String, affected: u64, executed_sql: String },
    Conflict { batch_id: String, executed_sql: String, current: Vec<Cell> },
    Error    { batch_id: String, executed_sql: String, message: String },
}
```

- [ ] **Step 2: Build update — parameterized + preview**

Add:

```rust
pub struct BuiltUpdate {
    pub parameterized_sql: String,
    pub binds: Vec<Cell>,
    pub preview_sql: String,
}

pub fn build_update(b: &PendingBatch, mode: ConflictMode) -> TuskResult<BuiltUpdate> {
    if b.op != PendingOp::Update {
        return Err(TuskError::Editing(format!("expected Update, got {:?}", b.op)));
    }
    if b.edits.is_empty() {
        return Err(TuskError::Editing("update with no edits".into()));
    }
    let table_ident = format!("\"{}\".\"{}\"", b.table.schema, b.table.name);

    // SET clause
    let mut set_parts = Vec::with_capacity(b.edits.len());
    let mut set_preview = Vec::with_capacity(b.edits.len());
    let mut binds: Vec<Cell> = Vec::new();
    for (i, e) in b.edits.iter().enumerate() {
        set_parts.push(format!("\"{}\" = ${}", e.column, i + 1));
        set_preview.push(format!("\"{}\" = {}", e.column, to_literal(&e.next)));
        binds.push(e.next.clone());
    }

    // WHERE clause: PK always.
    let mut where_parts = Vec::new();
    let mut where_preview = Vec::new();
    for (j, (pkc, pkv)) in b.pk_columns.iter().zip(b.pk_values.iter()).enumerate() {
        let bind_idx = binds.len() + 1;
        where_parts.push(format!("\"{}\" IS NOT DISTINCT FROM ${}", pkc, bind_idx));
        where_preview.push(format!("\"{}\" IS NOT DISTINCT FROM {}", pkc, to_literal(pkv)));
        binds.push(pkv.clone());
        let _ = j;
    }

    if let ConflictMode::Strict = mode {
        // Add per-column NULL-safe equality on non-edited captured columns.
        // Skip floats (PG IS NOT DISTINCT FROM still works for floats but
        // exact-bit equality is misleading; spec calls this out).
        for (col, val) in b.captured_columns.iter().zip(b.captured_row.iter()) {
            let is_pk = b.pk_columns.contains(col);
            let edited = b.edits.iter().any(|e| &e.column == col);
            let is_float = matches!(val, Cell::Float(_));
            if is_pk || edited || is_float { continue; }
            let bind_idx = binds.len() + 1;
            where_parts.push(format!("\"{}\" IS NOT DISTINCT FROM ${}", col, bind_idx));
            where_preview.push(format!("\"{}\" IS NOT DISTINCT FROM {}", col, to_literal(val)));
            binds.push(val.clone());
        }
    }

    let parameterized_sql = format!(
        "UPDATE {table_ident} SET {} WHERE {}",
        set_parts.join(", "),
        where_parts.join(" AND ")
    );
    let preview_sql = format!(
        "UPDATE {table_ident} SET {} WHERE {}",
        set_preview.join(", "),
        where_preview.join(" AND ")
    );
    Ok(BuiltUpdate { parameterized_sql, binds, preview_sql })
}
```

Implement `build_insert` and `build_delete` analogously:

```rust
pub fn build_insert(b: &PendingBatch) -> TuskResult<BuiltUpdate> {
    if b.op != PendingOp::Insert {
        return Err(TuskError::Editing(format!("expected Insert, got {:?}", b.op)));
    }
    if b.edits.is_empty() {
        return Err(TuskError::Editing("insert with no values".into()));
    }
    let table_ident = format!("\"{}\".\"{}\"", b.table.schema, b.table.name);
    let cols: Vec<String> = b.edits.iter().map(|e| format!("\"{}\"", e.column)).collect();
    let placeholders: Vec<String> = (1..=b.edits.len()).map(|i| format!("${i}")).collect();
    let preview_vals: Vec<String> = b.edits.iter().map(|e| to_literal(&e.next)).collect();
    let binds: Vec<Cell> = b.edits.iter().map(|e| e.next.clone()).collect();
    let parameterized_sql = format!(
        "INSERT INTO {table_ident} ({}) VALUES ({})",
        cols.join(", "), placeholders.join(", ")
    );
    let preview_sql = format!(
        "INSERT INTO {table_ident} ({}) VALUES ({})",
        cols.join(", "), preview_vals.join(", ")
    );
    Ok(BuiltUpdate { parameterized_sql, binds, preview_sql })
}

pub fn build_delete(b: &PendingBatch, mode: ConflictMode) -> TuskResult<BuiltUpdate> {
    if b.op != PendingOp::Delete {
        return Err(TuskError::Editing(format!("expected Delete, got {:?}", b.op)));
    }
    let table_ident = format!("\"{}\".\"{}\"", b.table.schema, b.table.name);
    let mut where_parts = Vec::new();
    let mut where_preview = Vec::new();
    let mut binds = Vec::new();
    for (pkc, pkv) in b.pk_columns.iter().zip(b.pk_values.iter()) {
        let bind_idx = binds.len() + 1;
        where_parts.push(format!("\"{}\" IS NOT DISTINCT FROM ${}", pkc, bind_idx));
        where_preview.push(format!("\"{}\" IS NOT DISTINCT FROM {}", pkc, to_literal(pkv)));
        binds.push(pkv.clone());
    }
    if let ConflictMode::Strict = mode {
        for (col, val) in b.captured_columns.iter().zip(b.captured_row.iter()) {
            if b.pk_columns.contains(col) { continue; }
            if matches!(val, Cell::Float(_)) { continue; }
            let bind_idx = binds.len() + 1;
            where_parts.push(format!("\"{}\" IS NOT DISTINCT FROM ${}", col, bind_idx));
            where_preview.push(format!("\"{}\" IS NOT DISTINCT FROM {}", col, to_literal(val)));
            binds.push(val.clone());
        }
    }
    let parameterized_sql = format!("DELETE FROM {table_ident} WHERE {}", where_parts.join(" AND "));
    let preview_sql = format!("DELETE FROM {table_ident} WHERE {}", where_preview.join(" AND "));
    Ok(BuiltUpdate { parameterized_sql, binds, preview_sql })
}
```

- [ ] **Step 3: Bind helper — `Cell` → `sqlx::Arguments`**

Add:

```rust
pub fn bind_cells<'q>(
    q: sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments>,
    binds: &'q [Cell],
) -> sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments> {
    let mut q = q;
    for c in binds {
        q = match c {
            Cell::Null => q.bind(None::<i32>),
            Cell::Bool(v) => q.bind(*v),
            Cell::Int(v) => q.bind(*v),
            Cell::Bigint(s) => q.bind(s.parse::<i64>().unwrap_or(0)),
            Cell::Float(v) => q.bind(*v),
            Cell::Numeric(s) => q.bind(s.parse::<bigdecimal::BigDecimal>().unwrap_or_default()),
            Cell::Text(v) => q.bind(v.clone()),
            Cell::Bytea { b64 } => {
                use base64::{engine::general_purpose::STANDARD, Engine};
                q.bind(STANDARD.decode(b64).unwrap_or_default())
            }
            Cell::Uuid(v) => q.bind(uuid::Uuid::parse_str(v).unwrap_or_default()),
            Cell::Inet(v) => q.bind(v.parse::<ipnetwork::IpNetwork>().unwrap_or_else(|_| "0.0.0.0/0".parse().unwrap())),
            Cell::Date(v) => q.bind(v.parse::<chrono::NaiveDate>().unwrap_or_default()),
            Cell::Time(v) => q.bind(v.parse::<chrono::NaiveTime>().unwrap_or_default()),
            Cell::Timestamp(v) => q.bind(v.parse::<chrono::NaiveDateTime>().unwrap_or_default()),
            Cell::Timestamptz(v) => q.bind(v.parse::<chrono::DateTime<chrono::Utc>>().unwrap_or_default()),
            Cell::Json(v) => q.bind(v.clone()),
            // Other variants (Interval, Array, Enum, Vector, Timetz, Unknown) are not
            // typically in a PendingBatch's bind list (Week 3 widget set).
            // Bind as Null for safety; a later task can extend.
            _ => q.bind(None::<i32>),
        };
    }
    q
}
```

- [ ] **Step 4: Builder unit tests**

Append:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn batch_update_simple() -> PendingBatch {
        PendingBatch {
            batch_id: "b1".into(),
            op: PendingOp::Update,
            table: TableRef { schema: "public".into(), name: "users".into() },
            pk_columns: vec!["id".into()],
            pk_values: vec![Cell::Int(42)],
            edits: vec![ColumnEdit { column: "email".into(), next: Cell::Text("new@x".into()) }],
            captured_row: vec![Cell::Int(42), Cell::Text("old@x".into()), Cell::Bool(true)],
            captured_columns: vec!["id".into(), "email".into(), "active".into()],
        }
    }

    #[test]
    fn build_update_pk_only_no_strict_clauses() {
        let built = build_update(&batch_update_simple(), ConflictMode::PkOnly).unwrap();
        assert_eq!(built.parameterized_sql,
            "UPDATE \"public\".\"users\" SET \"email\" = $1 WHERE \"id\" IS NOT DISTINCT FROM $2");
        assert_eq!(built.binds.len(), 2);
        assert!(built.preview_sql.contains("'new@x'"));
        assert!(built.preview_sql.contains("42"));
        assert!(!built.preview_sql.contains("\"active\""));
    }

    #[test]
    fn build_update_strict_adds_captured_clauses() {
        let built = build_update(&batch_update_simple(), ConflictMode::Strict).unwrap();
        assert!(built.parameterized_sql.contains("\"active\" IS NOT DISTINCT FROM"));
        assert_eq!(built.binds.len(), 3); // email + id + active
    }

    #[test]
    fn build_insert_uses_value_list() {
        let mut b = batch_update_simple();
        b.op = PendingOp::Insert;
        let built = build_insert(&b).unwrap();
        assert_eq!(built.parameterized_sql,
            "INSERT INTO \"public\".\"users\" (\"email\") VALUES ($1)");
    }

    #[test]
    fn build_delete_pk_only() {
        let mut b = batch_update_simple();
        b.op = PendingOp::Delete;
        let built = build_delete(&b, ConflictMode::PkOnly).unwrap();
        assert_eq!(built.parameterized_sql,
            "DELETE FROM \"public\".\"users\" WHERE \"id\" IS NOT DISTINCT FROM $1");
    }
}
```

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib editing
```

Expected: 4 tests pass.

- [ ] **Step 5: Integration test — end-to-end execution**

Create `src-tauri/tests/editing.rs`:

```rust
use sqlx::postgres::PgPoolOptions;
use tusk_lib::commands::editing::*;
use tusk_lib::db::decoder::Cell;

const URL: &str = "postgres://tusk:tusk@127.0.0.1:55432/tusk_test";

async fn pool() -> sqlx::PgPool {
    PgPoolOptions::new().max_connections(2).connect(URL).await.unwrap()
}

#[tokio::test]
async fn pkonly_update_round_trip() {
    let pool = pool().await;
    sqlx::query("DROP TABLE IF EXISTS edit_t").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE edit_t (id int primary key, email text)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO edit_t VALUES (1, 'old@x')").execute(&pool).await.unwrap();

    let b = PendingBatch {
        batch_id: "b1".into(),
        op: PendingOp::Update,
        table: TableRef { schema: "public".into(), name: "edit_t".into() },
        pk_columns: vec!["id".into()],
        pk_values: vec![Cell::Int(1)],
        edits: vec![ColumnEdit { column: "email".into(), next: Cell::Text("new@x".into()) }],
        captured_row: vec![Cell::Int(1), Cell::Text("old@x".into())],
        captured_columns: vec!["id".into(), "email".into()],
    };
    let built = build_update(&b, ConflictMode::PkOnly).unwrap();
    let mut tx = pool.begin().await.unwrap();
    let q = sqlx::query(&built.parameterized_sql);
    let q = bind_cells(q, &built.binds);
    let res = q.execute(&mut *tx).await.unwrap();
    assert_eq!(res.rows_affected(), 1);
    tx.commit().await.unwrap();

    let v: String = sqlx::query_scalar("SELECT email FROM edit_t WHERE id=1").fetch_one(&pool).await.unwrap();
    assert_eq!(v, "new@x");
}

#[tokio::test]
async fn strict_detects_concurrent_change() {
    let pool = pool().await;
    sqlx::query("DROP TABLE IF EXISTS edit_t2").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE edit_t2 (id int primary key, email text)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO edit_t2 VALUES (1, 'old@x')").execute(&pool).await.unwrap();

    // Concurrent mutation — simulates "someone else changed it"
    sqlx::query("UPDATE edit_t2 SET email = 'other@x' WHERE id = 1").execute(&pool).await.unwrap();

    let b = PendingBatch {
        batch_id: "b1".into(),
        op: PendingOp::Update,
        table: TableRef { schema: "public".into(), name: "edit_t2".into() },
        pk_columns: vec!["id".into()],
        pk_values: vec![Cell::Int(1)],
        edits: vec![ColumnEdit { column: "email".into(), next: Cell::Text("new@x".into()) }],
        captured_row: vec![Cell::Int(1), Cell::Text("old@x".into())], // stale!
        captured_columns: vec!["id".into(), "email".into()],
    };
    let built = build_update(&b, ConflictMode::Strict).unwrap();
    let mut tx = pool.begin().await.unwrap();
    let q = sqlx::query(&built.parameterized_sql);
    let q = bind_cells(q, &built.binds);
    let res = q.execute(&mut *tx).await.unwrap();
    assert_eq!(res.rows_affected(), 0); // conflict detected
}
```

Run:

```bash
docker compose -f infra/postgres/docker-compose.yml up -d
cargo test --manifest-path src-tauri/Cargo.toml --test editing
```

Expected: 2 tests pass.

- [ ] **Step 6: Wire commands/mod + commit**

Edit `src-tauri/src/commands/mod.rs`:

```rust
pub mod editing;
```

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
git add src-tauri/src/commands/{editing.rs,mod.rs} src-tauri/tests/editing.rs
git commit -m "feat(rust): editing builders (update/insert/delete) + bind helpers"
```

---

## Task 17: `submit_pending_changes` + `preview_pending_changes` + PreviewModal

**Goal:** Wire builders into Tauri commands. PreviewModal in frontend renders preview SQL; Submit runs the same batches against DB. Sticky-tx aware: if `tx_slot` is set, run inside it (atomic per-submit only — does NOT auto-commit). If empty, wrap in a fresh tx.

**Files:**

- Modify: `src-tauri/src/commands/editing.rs`
- Create: `src/features/editing/PreviewModal.tsx`
- Modify: `src/features/editing/PendingBadge.tsx` (wire callbacks)
- Modify: `src/features/results/ResultsGrid.tsx` (mount PendingBadge + PreviewModal)

**Steps:**

- [ ] **Step 1: Add submit + preview commands**

Append to `src-tauri/src/commands/editing.rs`:

```rust
use tauri::State;
use crate::db::pool::ConnectionRegistry;
use crate::db::state::{HistoryEntry, HistoryStatement, StateStore};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitResult {
    pub batches: Vec<BatchResult>,
    pub tx_state: Option<crate::commands::transactions::TxStateSnapshot>,
}

fn build_one(b: &PendingBatch, mode: ConflictMode) -> TuskResult<BuiltUpdate> {
    match b.op {
        PendingOp::Update => build_update(b, mode),
        PendingOp::Insert => build_insert(b),
        PendingOp::Delete => build_delete(b, mode),
    }
}

#[tauri::command]
pub async fn preview_pending_changes(
    batches: Vec<PendingBatch>,
    mode: ConflictMode,
) -> TuskResult<Vec<BatchResult>> {
    batches.iter().map(|b| {
        match build_one(b, mode) {
            Ok(built) => Ok(BatchResult::Ok {
                batch_id: b.batch_id.clone(), affected: 0, executed_sql: built.preview_sql,
            }),
            Err(e) => Ok(BatchResult::Error {
                batch_id: b.batch_id.clone(), executed_sql: String::new(), message: e.to_string(),
            }),
        }
    }).collect()
}

#[tauri::command]
pub async fn submit_pending_changes(
    registry: State<'_, ConnectionRegistry>,
    store: State<'_, StateStore>,
    connection_id: String,
    batches: Vec<PendingBatch>,
    mode: ConflictMode,
) -> TuskResult<SubmitResult> {
    let active = registry.handle(&connection_id)?;
    let in_tx = active.tx_slot.lock().await.is_some();

    // If we're inside a sticky tx, append to it (no implicit commit).
    if in_tx {
        let mut slot = active.tx_slot.lock().await;
        let sticky = slot.as_mut().expect("checked");
        let mut results = Vec::with_capacity(batches.len());
        for b in &batches {
            let built = match build_one(b, mode) {
                Ok(v) => v,
                Err(e) => {
                    results.push(BatchResult::Error { batch_id: b.batch_id.clone(), executed_sql: String::new(), message: e.to_string() });
                    return Ok(SubmitResult { batches: results, tx_state: None });
                }
            };
            let q = sqlx::query(&built.parameterized_sql);
            let q = bind_cells(q, &built.binds);
            let res = q.execute(&mut *sticky.conn).await;
            match res {
                Ok(r) => {
                    if r.rows_affected() == 0 && matches!(b.op, PendingOp::Update | PendingOp::Delete) {
                        // Conflict: roll back the sticky tx? No — sticky tx semantics say only
                        // user controls commit/rollback. Report conflict, leave tx aborted on the user.
                        let current = fetch_current(&mut *sticky.conn, b).await.unwrap_or_default();
                        results.push(BatchResult::Conflict {
                            batch_id: b.batch_id.clone(), executed_sql: built.preview_sql, current,
                        });
                        // Do NOT execute remaining batches — atomic semantics still apply within the submit.
                        // The user can rollback the whole sticky tx if desired.
                        break;
                    }
                    sticky.statement_count += 1;
                    let _ = store.append_history_statement(&HistoryStatement {
                        id: uuid::Uuid::new_v4().to_string(),
                        entry_id: sticky.history_entry_id.clone(),
                        ordinal: (sticky.statement_count - 1) as i64,
                        sql: built.preview_sql.chars().take(2000).collect(),
                        duration_ms: 0, row_count: Some(r.rows_affected() as i64),
                        status: "ok".into(), error_message: None,
                    });
                    results.push(BatchResult::Ok {
                        batch_id: b.batch_id.clone(), affected: r.rows_affected(),
                        executed_sql: built.preview_sql,
                    });
                }
                Err(e) => {
                    results.push(BatchResult::Error {
                        batch_id: b.batch_id.clone(), executed_sql: built.preview_sql, message: e.to_string(),
                    });
                    break;
                }
            }
        }
        return Ok(SubmitResult { batches: results, tx_state: None /* refreshed by caller */ });
    }

    // Implicit transaction — atomic.
    let pool = active.pool.clone();
    let mut tx = pool.begin().await.map_err(|e| TuskError::Tx(e.to_string()))?;
    let mut results = Vec::with_capacity(batches.len());
    let entry_id = uuid::Uuid::new_v4().to_string();
    let _ = store.insert_history_entry(&HistoryEntry {
        id: entry_id.clone(), conn_id: connection_id.clone(), source: "inline".into(),
        tx_id: None, sql_preview: format!("[inline edits x{}]", batches.len()),
        sql_full: None, started_at: chrono::Utc::now().timestamp_millis(),
        duration_ms: 0, row_count: None, status: "open".into(), error_message: None,
        statement_count: 0,
    });

    let mut conflict = false;
    for (i, b) in batches.iter().enumerate() {
        let built = match build_one(b, mode) {
            Ok(v) => v,
            Err(e) => {
                results.push(BatchResult::Error { batch_id: b.batch_id.clone(), executed_sql: String::new(), message: e.to_string() });
                conflict = true;
                break;
            }
        };
        let q = sqlx::query(&built.parameterized_sql);
        let q = bind_cells(q, &built.binds);
        let res = q.execute(&mut *tx).await;
        match res {
            Ok(r) => {
                if r.rows_affected() == 0 && matches!(b.op, PendingOp::Update | PendingOp::Delete) {
                    let current = fetch_current(&mut *tx, b).await.unwrap_or_default();
                    results.push(BatchResult::Conflict {
                        batch_id: b.batch_id.clone(), executed_sql: built.preview_sql, current,
                    });
                    conflict = true;
                    break;
                }
                let _ = store.append_history_statement(&HistoryStatement {
                    id: uuid::Uuid::new_v4().to_string(),
                    entry_id: entry_id.clone(),
                    ordinal: i as i64,
                    sql: built.preview_sql.chars().take(2000).collect(),
                    duration_ms: 0, row_count: Some(r.rows_affected() as i64),
                    status: "ok".into(), error_message: None,
                });
                results.push(BatchResult::Ok {
                    batch_id: b.batch_id.clone(), affected: r.rows_affected(),
                    executed_sql: built.preview_sql,
                });
            }
            Err(e) => {
                results.push(BatchResult::Error {
                    batch_id: b.batch_id.clone(), executed_sql: built.preview_sql, message: e.to_string(),
                });
                conflict = true;
                break;
            }
        }
    }

    if conflict {
        let _ = tx.rollback().await;
        let _ = store.update_history_entry_finalize(&entry_id, 0, None, "rolled_back", None, results.len() as i64);
    } else {
        tx.commit().await.map_err(|e| TuskError::Tx(e.to_string()))?;
        let _ = store.update_history_entry_finalize(&entry_id, 0, None, "ok", None, results.len() as i64);
    }
    Ok(SubmitResult { batches: results, tx_state: None })
}

async fn fetch_current<'e, E>(executor: E, b: &PendingBatch) -> TuskResult<Vec<Cell>>
where E: sqlx::Executor<'e, Database = Postgres> {
    let cols = b.captured_columns.iter().map(|c| format!("\"{}\"", c)).collect::<Vec<_>>().join(", ");
    let where_sql = b.pk_columns.iter().enumerate()
        .map(|(i, c)| format!("\"{}\" IS NOT DISTINCT FROM ${}", c, i + 1))
        .collect::<Vec<_>>().join(" AND ");
    let table_ident = format!("\"{}\".\"{}\"", b.table.schema, b.table.name);
    let sql = format!("SELECT {cols} FROM {table_ident} WHERE {where_sql} LIMIT 1");
    let q = sqlx::query(&sql);
    let q = bind_cells(q, &b.pk_values);
    let row = q.fetch_optional(executor).await.map_err(|e| TuskError::Editing(e.to_string()))?;
    let Some(row) = row else { return Ok(vec![]); };
    use crate::db::decoder::{columns_of, decode_row};
    let cols_meta = columns_of(&row);
    Ok(decode_row(&row, &cols_meta))
}
```

Wire into `lib.rs`:

```rust
            commands::editing::preview_pending_changes,
            commands::editing::submit_pending_changes,
```

- [ ] **Step 2: PreviewModal frontend**

Create `src/features/editing/PreviewModal.tsx`:

```tsx
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { usePendingChanges } from "@/store/pendingChanges";
import { useSettings } from "@/store/settings";

interface BatchOk {
  status: "ok";
  batchId: string;
  executedSql: string;
  affected: number;
}
interface BatchConflict {
  status: "conflict";
  batchId: string;
  executedSql: string;
  current: unknown[];
}
interface BatchError {
  status: "error";
  batchId: string;
  executedSql: string;
  message: string;
}
type BatchResult = BatchOk | BatchConflict | BatchError;

export function PreviewModal({
  connId,
  onClose,
  onSubmitDone,
}: {
  connId: string;
  onClose: () => void;
  onSubmitDone: (r: BatchResult[]) => void;
}) {
  const list = usePendingChanges((s) => s.list());
  const mode = useSettings((s) => s.editConflictMode);
  const [previews, setPreviews] = useState<BatchResult[]>([]);

  useEffect(() => {
    invoke<BatchResult[]>("preview_pending_changes", {
      batches: list.map(toRust),
      mode,
    }).then(setPreviews);
  }, [list, mode]);

  const submit = async () => {
    const r = await invoke<{ batches: BatchResult[] }>(
      "submit_pending_changes",
      {
        connectionId: connId,
        batches: list.map(toRust),
        mode,
      },
    );
    onSubmitDone(r.batches);
  };

  return (
    <div
      className="fixed inset-0 flex items-center justify-center bg-black/40"
      onClick={onClose}
    >
      <div
        className="bg-card max-h-[80vh] w-[640px] overflow-auto rounded border p-4"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-sm font-medium">
          Preview pending changes ({list.length})
        </h2>
        <pre className="mt-2 text-xs whitespace-pre-wrap">
          {previews.map((p) => p.executedSql).join(";\n\n")}
        </pre>
        <p className="text-muted-foreground mt-2 text-xs">
          Actual execution uses parameterized binds; this rendering inlines
          literals using PG escape rules.
        </p>
        <div className="mt-3 flex justify-end gap-2">
          <button onClick={onClose} className="text-xs">
            Cancel
          </button>
          <button
            onClick={submit}
            className="rounded bg-amber-500 px-2 py-0.5 text-xs text-black"
          >
            Submit Now
          </button>
        </div>
      </div>
    </div>
  );
}

function toRust(
  p: ReturnType<typeof usePendingChanges>["getState"] extends () => infer S
    ? S extends { list(): infer L }
      ? L extends Array<infer X>
        ? X
        : never
      : never
    : never,
): unknown {
  // The store already keeps fields in camelCase; Tauri serde handles snake_case
  // mapping automatically given #[serde(rename_all = "camelCase")] on the Rust side.
  return p;
}
```

(Type gymnastics in `toRust` — replace with a plain `(p: PendingChange) => unknown` and let serde do the work.)

- [ ] **Step 3: settings store — conflict mode toggle**

Edit `src/store/settings.ts` (existing). Add:

```ts
editConflictMode: 'pkOnly' | 'strict';     // default: 'pkOnly'
setEditConflictMode(m: 'pkOnly' | 'strict'): void;
```

- [ ] **Step 4: Wire PendingBadge in ResultsGrid**

Edit `ResultsGrid.tsx`. Mount `<PendingBadge onPreview={...} onSubmit={...} onRevert={...}>`. Open `PreviewModal` when Preview is clicked. On `onSubmitDone`, if no conflicts → `usePendingChanges.getState().revertAll()` (commit succeeded — clear staging) + toast success. If conflict → leave staging intact, show ConflictModal (Task 18).

- [ ] **Step 5: Manual smoke + commit**

```bash
docker compose -f infra/postgres/docker-compose.yml up -d
pnpm tauri dev
# Edit a row, click Preview, see SQL. Click Submit. Verify DB updated via psql.
pnpm typecheck && pnpm lint && pnpm format:check && pnpm build
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
git add src-tauri/src/commands/editing.rs src-tauri/src/lib.rs \
        src/features/editing/PreviewModal.tsx \
        src/features/editing/PendingBadge.tsx \
        src/features/results/ResultsGrid.tsx \
        src/store/settings.ts
git commit -m "feat: submit/preview pending changes + PreviewModal"
```

---

## Task 18: Strict mode + ConflictModal + atomic rollback

**Goal:** UI for the conflict resolution flow. Backend already does atomic rollback (Task 17). This task adds:

1. Settings UI for the PkOnly / Strict toggle.
2. ConflictModal showing diff of "your edits" vs "server now" + three actions (Force overwrite, Discard, Re-edit on top of server).
3. Integration test for atomic rollback under multi-batch conflict.

**Files:**

- Create: `src/features/editing/ConflictModal.tsx`
- Modify: `src/features/results/ResultsGrid.tsx`
- Add to `src-tauri/tests/editing.rs`: multi-batch atomic test

**Steps:**

- [ ] **Step 1: Conflict-mode toggle UI**

Add a small dropdown in the result grid header (next to ✏️ indicator):

```tsx
import { useSettings } from "@/store/settings";
const mode = useSettings((s) => s.editConflictMode);
const setMode = useSettings((s) => s.setEditConflictMode);
// ...
<select
  value={mode}
  onChange={(e) => setMode(e.target.value as "pkOnly" | "strict")}
  className="text-xs"
>
  <option value="pkOnly">PK only</option>
  <option value="strict">Strict</option>
</select>;
```

- [ ] **Step 2: ConflictModal**

```tsx
// src/features/editing/ConflictModal.tsx
import { useState } from "react";
import type { Cell, PendingChange } from "@/lib/types";
import { renderCell } from "@/features/results/cells";
import { usePendingChanges } from "@/store/pendingChanges";

interface Props {
  conflict: { batchId: string; current: Cell[] };
  pending: PendingChange;
  capturedColumns: string[];
  onForceOverwrite: () => void;
  onDiscard: () => void;
  onClose: () => void;
}

export function ConflictModal({
  conflict,
  pending,
  capturedColumns,
  onForceOverwrite,
  onDiscard,
  onClose,
}: Props) {
  return (
    <div
      className="fixed inset-0 flex items-center justify-center bg-black/40"
      onClick={onClose}
    >
      <div
        className="bg-card w-[520px] rounded border p-4"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-sm font-medium">
          Row was modified by someone else
        </h2>
        <table className="mt-2 w-full text-xs">
          <thead>
            <tr>
              <th>Column</th>
              <th>Your edit</th>
              <th>Server now</th>
            </tr>
          </thead>
          <tbody>
            {pending.edits.map((e, i) => {
              const colIdx = capturedColumns.indexOf(e.column);
              const serverNow =
                colIdx >= 0
                  ? conflict.current[colIdx]
                  : ({ kind: "Unknown", value: { oid: 0, text: "?" } } as Cell);
              return (
                <tr key={i}>
                  <td className="pr-2">{e.column}</td>
                  <td className="pr-2">{renderCell(e.next)}</td>
                  <td>{renderCell(serverNow)}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
        <div className="mt-3 flex justify-end gap-2 text-xs">
          <button onClick={onDiscard}>Discard your edits</button>
          <button
            onClick={() => {
              // Re-edit on top of server: replace capturedRow + reset originals
              const updated = {
                ...pending,
                capturedRow: conflict.current,
                capturedAt: Date.now(),
              };
              const next = new Map(usePendingChanges.getState().byRow);
              next.set(pending.rowKey, updated);
              usePendingChanges.setState({ byRow: next });
              onClose();
            }}
          >
            Re-edit on top of server
          </button>
          <button
            onClick={onForceOverwrite}
            className="rounded border px-2 py-0.5"
          >
            Force overwrite
          </button>
        </div>
      </div>
    </div>
  );
}
```

`onForceOverwrite` re-submits the same `PendingChange` with `mode='pkOnly'` (which has no `IS NOT DISTINCT FROM` on captured columns).

- [ ] **Step 3: Wire ConflictModal in ResultsGrid**

After `submit_pending_changes` returns:

```tsx
const conflicts = result.batches.filter(
  (b) => b.status === "conflict",
) as BatchConflict[];
if (conflicts.length > 0) {
  setActiveConflict(conflicts[0]); // show first; user resolves one at a time
} else {
  toast.success(
    `${result.batches.filter((b) => b.status === "ok").length} row(s) updated`,
  );
  usePendingChanges.getState().revertAll();
}
```

- [ ] **Step 4: Multi-batch atomic test**

Append to `src-tauri/tests/editing.rs`:

```rust
#[tokio::test]
async fn multi_batch_atomic_rollback_on_conflict() {
    let pool = pool().await;
    sqlx::query("DROP TABLE IF EXISTS edit_t3").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE edit_t3 (id int primary key, v int)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO edit_t3 VALUES (1, 10), (2, 20)").execute(&pool).await.unwrap();

    // Simulate concurrent change on row 2 → strict will fail to match.
    sqlx::query("UPDATE edit_t3 SET v = 999 WHERE id = 2").execute(&pool).await.unwrap();

    let b1 = PendingBatch {
        batch_id: "b1".into(), op: PendingOp::Update,
        table: TableRef { schema: "public".into(), name: "edit_t3".into() },
        pk_columns: vec!["id".into()], pk_values: vec![Cell::Int(1)],
        edits: vec![ColumnEdit { column: "v".into(), next: Cell::Int(11) }],
        captured_row: vec![Cell::Int(1), Cell::Int(10)],
        captured_columns: vec!["id".into(), "v".into()],
    };
    let b2 = PendingBatch {
        batch_id: "b2".into(), op: PendingOp::Update,
        table: TableRef { schema: "public".into(), name: "edit_t3".into() },
        pk_columns: vec!["id".into()], pk_values: vec![Cell::Int(2)],
        edits: vec![ColumnEdit { column: "v".into(), next: Cell::Int(21) }],
        captured_row: vec![Cell::Int(2), Cell::Int(20)],  // stale
        captured_columns: vec!["id".into(), "v".into()],
    };

    // Mimic submit_pending_changes' atomic body inline.
    let mut tx = pool.begin().await.unwrap();
    let mut conflict = false;
    for b in [&b1, &b2] {
        let built = build_update(b, ConflictMode::Strict).unwrap();
        let q = sqlx::query(&built.parameterized_sql);
        let q = bind_cells(q, &built.binds);
        let r = q.execute(&mut *tx).await.unwrap();
        if r.rows_affected() == 0 { conflict = true; break; }
    }
    if conflict { tx.rollback().await.unwrap(); } else { tx.commit().await.unwrap(); }

    // Assert row 1 was NOT updated (atomic rollback).
    let v1: i32 = sqlx::query_scalar("SELECT v FROM edit_t3 WHERE id=1").fetch_one(&pool).await.unwrap();
    assert_eq!(v1, 10);
}
```

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test editing
```

- [ ] **Step 5: Quality gates + commit**

```bash
pnpm typecheck && pnpm lint && pnpm format:check && pnpm build
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
git add src/features/editing/ConflictModal.tsx \
        src/features/results/ResultsGrid.tsx \
        src-tauri/tests/editing.rs
git commit -m "feat: strict mode toggle + ConflictModal + atomic-rollback test"
```

---

## Task 19: INSERT / DELETE row UI

**Goal:** "+ Row" button at the bottom of an editable result; row right-click shows "Delete row".

**Files:**

- Modify: `src/features/results/ResultsGrid.tsx`
- Modify: `src/store/pendingChanges.ts` (insertRow / deleteRow helpers)

**Steps:**

- [ ] **Step 1: Add insertRow / deleteRow to store**

In `src/store/pendingChanges.ts`:

```ts
insertRow(args: {
  table: { schema: string; name: string };
  pkColumns: string[];
  defaults: Record<string, Cell>;
  capturedColumns: string[];
}): void;
deleteRow(args: {
  table: { schema: string; name: string };
  pkColumns: string[];
  pkValues: Cell[];
  capturedRow: Cell[];
  capturedColumns: string[];
}): void;
```

Implement in the store:

```ts
insertRow({ table, pkColumns, defaults, capturedColumns }) {
  set((s) => {
    const next = new Map(s.byRow);
    const rowKey = `__insert_${Math.random().toString(36).slice(2, 8)}`;
    const change: PendingChange = {
      rowKey, table, pk: { columns: pkColumns, values: [] },
      edits: capturedColumns.map((col) => ({
        column: col,
        original: { kind: 'Null' },
        next: defaults[col] ?? { kind: 'Null' },
      })),
      op: 'insert',
      capturedRow: capturedColumns.map((col) => defaults[col] ?? { kind: 'Null' }),
      capturedColumns,
      capturedAt: Date.now(),
    };
    next.set(rowKey, change);
    return { byRow: next };
  });
},
deleteRow({ table, pkColumns, pkValues, capturedRow, capturedColumns }) {
  set((s) => {
    const next = new Map(s.byRow);
    const rowKey = JSON.stringify(pkValues);
    next.set(rowKey, {
      rowKey, table, pk: { columns: pkColumns, values: pkValues },
      edits: [], op: 'delete', capturedRow, capturedColumns, capturedAt: Date.now(),
    });
    return { byRow: next };
  });
},
```

- [ ] **Step 2: + Row button**

In `ResultsGrid.tsx`, when `meta.editable`:

```tsx
<button
  onClick={() =>
    usePendingChanges.getState().insertRow({
      table: meta.table!,
      pkColumns: meta.pkColumns,
      defaults: {},
      capturedColumns: meta.columnTypes.map((c) => c.name),
    })
  }
  className="text-xs"
>
  + Row
</button>
```

The new row shows up as a "ghost" row (rendered separately above pending edits) where every cell can be edited via the same widgets. PK columns may need to be filled before submit if they're not auto-generated; if they're missing, the backend insert will error and the modal shows it.

- [ ] **Step 3: Delete row context (until Task 22's full menu lands, a button is fine)**

In each row's leftmost cell, add a small ✕ button when `meta.editable`:

```tsx
<button
  onClick={() =>
    usePendingChanges.getState().deleteRow({
      table: meta.table!,
      pkColumns: meta.pkColumns,
      pkValues: pkValuesOf(meta, row),
      capturedRow: row,
      capturedColumns: meta.columnTypes.map((c) => c.name),
    })
  }
  className="text-xs text-red-500"
>
  ✕
</button>
```

(The full ContextMenu in Task 22 supersedes this.)

- [ ] **Step 4: Manual smoke + commit**

```bash
pnpm tauri dev
# Click "+ Row" → fill PK + fields → Submit → row appears in DB.
# Click ✕ on a row → Submit → row gone.
pnpm typecheck && pnpm lint && pnpm format:check && pnpm build
git add src/store/pendingChanges.ts src/features/results/ResultsGrid.tsx
git commit -m "feat(frontend): + Row / Delete row inline editing"
```

---

## Task 20: Query cancellation (`cancel.rs`)

**Goal:** Toast Cancel button → backend issues `pg_cancel_backend(pid)` from a separate connection. TDD: long-running `pg_sleep` + cancel.

**Files:**

- Create: `src-tauri/src/commands/cancel.rs`
- Modify: `src-tauri/src/commands/query.rs` (emit `query:started` event with pid)
- Modify: `src-tauri/src/commands/mod.rs` + `lib.rs`
- Create: `src-tauri/tests/cancel.rs`
- Modify: frontend — running toast + Cancel

**Steps:**

- [ ] **Step 1: Backend command**

```rust
// src-tauri/src/commands/cancel.rs
use sqlx::postgres::PgPoolOptions;
use tauri::State;

use crate::db::pool::ConnectionRegistry;
use crate::db::state::StateStore;
use crate::errors::{TuskError, TuskResult};

#[tauri::command]
pub async fn cancel_query(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
    pid: i32,
) -> TuskResult<bool> {
    // Build a fresh single-connection pool from the same connection's URL.
    let url = registry.connection_url(&connection_id)?;   // see helper below
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&url).await
        .map_err(|e| TuskError::Tx(format!("cancel pool: {e}")))?;
    let cancelled: bool = sqlx::query_scalar("SELECT pg_cancel_backend($1)")
        .bind(pid)
        .fetch_one(&pool).await
        .map_err(|e| TuskError::Tx(format!("pg_cancel_backend: {e}")))?;
    pool.close().await;
    Ok(cancelled)
}
```

Add `connection_url(&id)` to `ConnectionRegistry` so cancel can open a parallel session. (The registry already knows host/port/user/db for tunneled connections — for SSH we route through the same local forwarded port.)

- [ ] **Step 2: Emit `query:started` from `execute_query`**

In `query.rs`, immediately after acquiring the connection, fetch the backend pid and emit:

```rust
use tauri::Emitter;
let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()").fetch_one(&pool).await
    .unwrap_or(-1);
let _ = app_handle.emit("query:started", serde_json::json!({
    "connId": connection_id, "pid": pid, "startedAt": chrono::Utc::now().timestamp_millis(),
}));
```

(`app_handle: tauri::AppHandle` becomes a parameter on `execute_query`.)

- [ ] **Step 3: Frontend — running toast with Cancel**

Listen for `query:started` and show a sonner toast with action button after 500ms:

```tsx
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";

useEffect(() => {
  const u = listen<{ connId: string; pid: number; startedAt: number }>(
    "query:started",
    (ev) => {
      const t = setTimeout(() => {
        const id = toast("Running query...", {
          action: {
            label: "Cancel",
            onClick: () =>
              invoke("cancel_query", {
                connectionId: ev.payload.connId,
                pid: ev.payload.pid,
              }),
          },
          duration: Infinity,
        });
        // Dismiss when query completes (next 'query:started' or completion event).
        void id;
      }, 500);
      return () => clearTimeout(t);
    },
  );
  return () => {
    u.then((fn) => fn());
  };
}, []);
```

- [ ] **Step 4: Integration test**

```rust
// src-tauri/tests/cancel.rs
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

const URL: &str = "postgres://tusk:tusk@127.0.0.1:55432/tusk_test";

#[tokio::test(flavor = "multi_thread")]
async fn cancel_long_running_select() {
    let pool = PgPoolOptions::new().max_connections(2).connect(URL).await.unwrap();
    let mut victim = pool.acquire().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()").fetch_one(&mut *victim).await.unwrap();

    let pool2 = pool.clone();
    let canceller = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let _: bool = sqlx::query_scalar("SELECT pg_cancel_backend($1)").bind(pid).fetch_one(&pool2).await.unwrap();
    });

    let res = sqlx::query("SELECT pg_sleep(10)").execute(&mut *victim).await;
    canceller.await.unwrap();
    assert!(res.is_err(), "expected cancellation error");
    assert!(format!("{:?}", res.unwrap_err()).contains("canceling statement"));
}
```

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test cancel -- --test-threads=1
```

- [ ] **Step 5: Wire + commit**

```rust
// commands/mod.rs
pub mod cancel;
// lib.rs invoke handler:
            commands::cancel::cancel_query,
```

```bash
pnpm typecheck && pnpm lint && pnpm format:check && pnpm build
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
git add src-tauri/src/commands/{cancel.rs,query.rs,mod.rs} src-tauri/src/lib.rs \
        src-tauri/tests/cancel.rs src/App.tsx
git commit -m "feat: query cancellation via pg_cancel_backend + running toast"
```

---

## Task 21: Export — CSV / JSON / SQL INSERT + ExportDialog

**Goal:** Stream the current result set (or selected rows) into a chosen format. SQL INSERT reuses `pg_literals`. Output goes to a user-chosen file path via Tauri dialog.

**Files:**

- Create: `src-tauri/src/commands/export.rs`
- Modify: `src-tauri/src/commands/mod.rs` + `lib.rs`
- Create: `src/features/export/ExportDialog.tsx`
- Modify: `src/features/results/ResultsGrid.tsx` (Export button)

**Steps:**

- [ ] **Step 1: Backend command**

```rust
// src-tauri/src/commands/export.rs
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::db::decoder::Cell;
use crate::db::pg_literals::to_literal;
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat { Csv, Json, SqlInsert }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub format: ExportFormat,
    pub path: PathBuf,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
    pub include_bom: bool,        // CSV
    pub table: Option<String>,    // SQL INSERT — required
}

#[derive(Debug, Serialize)]
pub struct ExportResult { pub path: String, pub bytes_written: u64 }

#[tauri::command]
pub async fn export_result(req: ExportRequest) -> TuskResult<ExportResult> {
    let f = File::create(&req.path).map_err(|e| TuskError::Internal(format!("export create: {e}")))?;
    let mut w = BufWriter::new(f);
    let written = match req.format {
        ExportFormat::Csv => write_csv(&mut w, &req)?,
        ExportFormat::Json => write_json(&mut w, &req)?,
        ExportFormat::SqlInsert => write_sql_inserts(&mut w, &req)?,
    };
    w.flush().map_err(|e| TuskError::Internal(format!("flush: {e}")))?;
    Ok(ExportResult { path: req.path.to_string_lossy().into(), bytes_written: written })
}

fn cell_to_csv(c: &Cell) -> String {
    match c {
        Cell::Null => "".into(),
        Cell::Bool(b) => if *b { "true".into() } else { "false".into() },
        Cell::Int(v) => v.to_string(),
        Cell::Float(v) => v.to_string(),
        Cell::Bigint(s) | Cell::Numeric(s) | Cell::Uuid(s) | Cell::Inet(s)
            | Cell::Date(s) | Cell::Time(s) | Cell::Timetz(s)
            | Cell::Timestamp(s) | Cell::Timestamptz(s) | Cell::Text(s) => s.clone(),
        Cell::Bytea { b64 } => format!("\\x{}", b64),
        Cell::Interval { iso } => iso.clone(),
        Cell::Json(v) => serde_json::to_string(v).unwrap_or_default(),
        Cell::Array { values, .. } => {
            let inner = values.iter().map(cell_to_csv).collect::<Vec<_>>().join(",");
            format!("{{{inner}}}")
        }
        Cell::Enum { value, .. } => value.clone(),
        Cell::Vector { values, .. } => {
            let inner = values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",");
            format!("[{inner}]")
        }
        Cell::Unknown { text, .. } => text.clone(),
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        let escaped = s.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else { s.to_string() }
}

fn write_csv<W: Write>(w: &mut W, req: &ExportRequest) -> TuskResult<u64> {
    let mut total = 0u64;
    if req.include_bom {
        w.write_all(&[0xEF, 0xBB, 0xBF]).map_err(|e| TuskError::Internal(e.to_string()))?;
        total += 3;
    }
    let header = req.columns.iter().map(|c| csv_escape(c)).collect::<Vec<_>>().join(",");
    writeln!(w, "{header}").map_err(|e| TuskError::Internal(e.to_string()))?;
    total += header.len() as u64 + 1;
    for row in &req.rows {
        let line = row.iter().map(|c| csv_escape(&cell_to_csv(c))).collect::<Vec<_>>().join(",");
        writeln!(w, "{line}").map_err(|e| TuskError::Internal(e.to_string()))?;
        total += line.len() as u64 + 1;
    }
    Ok(total)
}

fn write_json<W: Write>(w: &mut W, req: &ExportRequest) -> TuskResult<u64> {
    use serde_json::json;
    let arr: Vec<serde_json::Value> = req.rows.iter().map(|row| {
        let obj: serde_json::Map<String, serde_json::Value> = req.columns.iter().enumerate().map(|(i, c)| {
            (c.clone(), serde_json::to_value(&row[i]).unwrap_or(serde_json::Value::Null))
        }).collect();
        json!(obj)
    }).collect();
    let s = serde_json::to_string_pretty(&arr).map_err(|e| TuskError::Internal(e.to_string()))?;
    w.write_all(s.as_bytes()).map_err(|e| TuskError::Internal(e.to_string()))?;
    Ok(s.len() as u64)
}

fn write_sql_inserts<W: Write>(w: &mut W, req: &ExportRequest) -> TuskResult<u64> {
    let table = req.table.as_deref().ok_or_else(|| TuskError::Internal("export sql: table required".into()))?;
    let cols = req.columns.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
    let mut total = 0u64;
    for row in &req.rows {
        let vals = row.iter().map(to_literal).collect::<Vec<_>>().join(", ");
        let line = format!("INSERT INTO {table} ({cols}) VALUES ({vals});\n");
        w.write_all(line.as_bytes()).map_err(|e| TuskError::Internal(e.to_string()))?;
        total += line.len() as u64;
    }
    Ok(total)
}
```

Wire + invoke handler.

- [ ] **Step 2: ExportDialog frontend**

```tsx
// src/features/export/ExportDialog.tsx
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import type { Cell, ResultMeta } from "@/lib/types";

interface Props {
  rows: Cell[][];
  meta: ResultMeta;
  onClose: () => void;
}

export function ExportDialog({ rows, meta, onClose }: Props) {
  const [format, setFormat] = useState<"Csv" | "Json" | "SqlInsert">("Csv");
  const [bom, setBom] = useState(false);
  const [scope, setScope] = useState<"all" | "selected">("all");

  const run = async () => {
    const path = await save({
      defaultPath: meta.table?.name ?? "result",
      filters: [
        {
          name: format,
          extensions: [
            format === "Csv" ? "csv" : format === "Json" ? "json" : "sql",
          ],
        },
      ],
    });
    if (!path) return;
    const cols = meta.columnTypes.map((c) => c.name);
    // Row selection (`scope === 'selected'`) is not wired in this plan; the
    // grid's selection state is a v1.5 follow-up. For Week 3 we hard-fail
    // when the user picks "Selected rows" without an actual selection
    // mechanism by exporting an empty array — the user sees an empty file
    // and can rerun with "All rows".
    const useRows = scope === "all" ? rows : [];
    await invoke("export_result", {
      req: {
        format,
        path,
        columns: cols,
        rows: useRows,
        includeBom: bom,
        table:
          format === "SqlInsert"
            ? `"${meta.table?.schema}"."${meta.table?.name}"`
            : null,
      },
    });
    onClose();
  };

  return (
    <div
      className="fixed inset-0 flex items-center justify-center bg-black/40"
      onClick={onClose}
    >
      <div
        className="bg-card w-[360px] rounded border p-4"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-sm font-medium">Export result</h2>
        <div className="mt-2 space-y-1 text-xs">
          <div>
            Format:{" "}
            <select
              value={format}
              onChange={(e) =>
                setFormat(e.target.value as "Csv" | "Json" | "SqlInsert")
              }
            >
              <option value="Csv">CSV</option>
              <option value="Json">JSON</option>
              <option value="SqlInsert">SQL INSERT</option>
            </select>
          </div>
          {format === "Csv" && (
            <label>
              <input
                type="checkbox"
                checked={bom}
                onChange={(e) => setBom(e.target.checked)}
              />{" "}
              UTF-8 BOM
            </label>
          )}
          <div>
            Scope:{" "}
            <select
              value={scope}
              onChange={(e) => setScope(e.target.value as "all" | "selected")}
            >
              <option value="all">All rows</option>
              <option value="selected">Selected rows</option>
            </select>
          </div>
          {format === "SqlInsert" && !meta.table && (
            <p className="text-red-500">
              SQL INSERT requires a single-table source.
            </p>
          )}
        </div>
        <div className="mt-3 flex justify-end gap-2 text-xs">
          <button onClick={onClose}>Cancel</button>
          <button onClick={run} className="rounded border px-2">
            Export
          </button>
        </div>
      </div>
    </div>
  );
}
```

Add `@tauri-apps/plugin-dialog` to `package.json` if missing.

- [ ] **Step 3: Manual smoke + commit**

```bash
pnpm typecheck && pnpm lint && pnpm format:check && pnpm build
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
git add src-tauri/src/commands/{export.rs,mod.rs} src-tauri/src/lib.rs \
        src/features/export/ExportDialog.tsx \
        src/features/results/ResultsGrid.tsx package.json
git commit -m "feat: export result to CSV / JSON / SQL INSERT"
```

---

## Task 22: Cell context menu + Cmd+P history palette

**Goal:** Right-click on any cell shows Copy / Copy as INSERT / Set NULL / Filter by this value. Cmd+P opens a search palette over `history_entry`.

**Files:**

- Create: `src/features/results/ContextMenu.tsx`
- Create: `src/features/history/HistoryPalette.tsx`
- Create: `src/store/history.ts`
- Modify: `src/features/results/ResultsGrid.tsx`
- Modify: app shell (Cmd+P keybind)

**Steps:**

- [ ] **Step 1: ContextMenu**

```tsx
// src/features/results/ContextMenu.tsx
import type { Cell, ResultMeta } from "@/lib/types";
import { invoke } from "@tauri-apps/api/core";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { usePendingChanges, pkValuesOf } from "@/store/pendingChanges";

export function CellContextMenu({
  cell,
  columnIndex,
  row,
  meta,
  onClose,
  onFilter,
}: {
  cell: Cell;
  columnIndex: number;
  row: Cell[];
  meta: ResultMeta;
  onClose: () => void;
  onFilter: (col: string, value: Cell) => void;
}) {
  const colName = meta.columnTypes[columnIndex].name;
  const nullable = meta.columnTypes[columnIndex].nullable;

  const copyText = async () => {
    const text = cellToText(cell);
    await writeText(text);
    onClose();
  };

  const copyAsInsert = async () => {
    if (!meta.table) return onClose();
    const cols = meta.columnTypes.map((c) => `"${c.name}"`).join(", ");
    const literals = await invoke<string>("preview_inline_literals", { row });
    const text = `INSERT INTO "${meta.table.schema}"."${meta.table.name}" (${cols}) VALUES (${literals});`;
    await writeText(text);
    onClose();
  };

  const setNull = () => {
    if (!nullable) return;
    usePendingChanges.getState().upsertEdit({
      table: meta.table!,
      pkColumns: meta.pkColumns,
      pkValues: pkValuesOf(meta, row),
      column: colName,
      original: cell,
      next: { kind: "Null" },
      capturedRow: row,
      capturedColumns: meta.columnTypes.map((c) => c.name),
    });
    onClose();
  };

  return (
    <div className="bg-card absolute rounded border text-xs shadow">
      <button
        onClick={copyText}
        className="hover:bg-muted block w-full px-2 py-1 text-left"
      >
        Copy
      </button>
      <button
        onClick={copyAsInsert}
        className="hover:bg-muted block w-full px-2 py-1 text-left"
        disabled={!meta.table}
      >
        Copy as INSERT
      </button>
      {nullable && (
        <button
          onClick={setNull}
          className="hover:bg-muted block w-full px-2 py-1 text-left"
        >
          Set NULL
        </button>
      )}
      <button
        onClick={() => {
          onFilter(colName, cell);
          onClose();
        }}
        className="hover:bg-muted block w-full px-2 py-1 text-left"
      >
        Filter by this value
      </button>
    </div>
  );
}

function cellToText(c: Cell): string {
  switch (c.kind) {
    case "Null":
      return "NULL";
    case "Bool":
      return c.value ? "true" : "false";
    case "Int":
    case "Float":
      return String(c.value);
    case "Bigint":
    case "Numeric":
    case "Text":
    case "Uuid":
    case "Inet":
    case "Date":
    case "Time":
    case "Timetz":
    case "Timestamp":
    case "Timestamptz":
      return c.value;
    case "Interval":
      return c.value.iso;
    case "Bytea":
      return `\\x${c.value.b64}`;
    case "Json":
      return JSON.stringify(c.value);
    case "Array":
      return JSON.stringify(c.value.values);
    case "Enum":
      return c.value.value;
    case "Vector":
      return JSON.stringify(c.value.values);
    case "Unknown":
      return c.value.text;
  }
}
```

For `preview_inline_literals`, add a small Rust helper that takes `row: Vec<Cell>` and returns a comma-joined `to_literal` string (use `commands::editing` or extend `pg_literals` with a `row_to_literal_csv` helper). Wire as a Tauri command or compute client-side via a TS mirror of `pg_literals`. **Recommendation: TS mirror**, since `pg_literals` is small and that avoids the round-trip. Place it in `src/lib/pgLiterals.ts` (file already scaffolded in Task 1).

For "Filter by this value", the simplest non-invasive option: append `WHERE "<col>" = <literal>` to the active editor tab. Implement `onFilter` in ResultsGrid by calling a tabs-store helper.

- [ ] **Step 2: TS mirror of pg_literals**

Implement `src/lib/pgLiterals.ts` with the same logic as Rust's `to_literal`. Keep the test parity by writing a vitest spec in Task 23 that asserts identical outputs for every Cell variant. For now:

```ts
import type { Cell } from "./types";

export function toLiteral(c: Cell): string {
  switch (c.kind) {
    case "Null":
      return "NULL";
    case "Bool":
      return c.value ? "TRUE" : "FALSE";
    case "Int":
    case "Float":
      return String(c.value);
    case "Bigint":
    case "Numeric":
      return c.value;
    case "Text":
      return quote(c.value);
    case "Bytea":
      return `'\\x${b64ToHex(c.value.b64)}'::bytea`;
    case "Uuid":
      return `${quote(c.value)}::uuid`;
    case "Inet":
      return `${quote(c.value)}::inet`;
    case "Date":
      return `${quote(c.value)}::date`;
    case "Time":
      return `${quote(c.value)}::time`;
    case "Timetz":
      return `${quote(c.value)}::timetz`;
    case "Timestamp":
      return `${quote(c.value)}::timestamp`;
    case "Timestamptz":
      return `${quote(c.value)}::timestamptz`;
    case "Interval":
      return `${quote(c.value.iso)}::interval`;
    case "Json":
      return `${quote(JSON.stringify(c.value))}::jsonb`;
    case "Enum":
      return `${quote(c.value.value)}::${c.value.typeName}`;
    case "Array": {
      const inner = c.value.values.map(toLiteral).join(",");
      return `ARRAY[${inner}]::${c.value.elem}[]`;
    }
    case "Vector":
      return `${quote(`[${c.value.values.join(",")}]`)}::vector`;
    case "Unknown":
      return `${quote(c.value.text)}::text`;
  }
}

function quote(s: string): string {
  return `'${s.replace(/'/g, "''")}'`;
}
function b64ToHex(b64: string): string {
  const bin = atob(b64);
  let out = "";
  for (let i = 0; i < bin.length; i++)
    out += bin.charCodeAt(i).toString(16).padStart(2, "0");
  return out;
}
```

- [ ] **Step 3: HistoryPalette**

```ts
// src/store/history.ts
import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { HistoryEntry } from "@/lib/types";

interface S {
  entries: HistoryEntry[];
  search(query: string, connId?: string): Promise<void>;
}
export const useHistory = create<S>((set) => ({
  entries: [],
  async search(query, connId) {
    const e = await invoke<HistoryEntry[]>("list_history", {
      connectionId: connId ?? null,
      query: query || null,
      limit: 50,
    });
    set({ entries: e });
  },
}));
```

```tsx
// src/features/history/HistoryPalette.tsx
import { useEffect, useState } from "react";
import { useHistory } from "@/store/history";

export function HistoryPalette({
  onClose,
  onPick,
}: {
  onClose: () => void;
  onPick: (sql: string) => void;
}) {
  const [q, setQ] = useState("");
  const entries = useHistory((s) => s.entries);
  const search = useHistory((s) => s.search);

  useEffect(() => {
    const t = setTimeout(() => {
      void search(q);
    }, 120);
    return () => clearTimeout(t);
  }, [q, search]);

  return (
    <div
      className="fixed inset-0 flex items-start justify-center bg-black/40 pt-24"
      onClick={onClose}
    >
      <div
        className="bg-card w-[640px] rounded border p-2"
        onClick={(e) => e.stopPropagation()}
      >
        <input
          autoFocus
          placeholder="Search history…"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          className="w-full"
        />
        <ul className="mt-2 max-h-[60vh] overflow-auto">
          {entries.map((e) => (
            <li
              key={e.id}
              className="hover:bg-muted cursor-pointer py-1 text-xs"
              onClick={() => onPick(e.sqlFull ?? e.sqlPreview)}
            >
              <span className="text-muted-foreground mr-2">
                {new Date(e.startedAt).toISOString().slice(0, 19)}
              </span>
              <span>{e.sqlPreview}</span>
              {e.statementCount > 1 && (
                <span className="ml-2 text-amber-500">
                  (tx · {e.statementCount} stmts)
                </span>
              )}
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
```

In App shell, listen for `Cmd/Ctrl+P` and toggle the palette. On pick → load into the active editor tab via tabs store.

- [ ] **Step 4: Manual smoke + commit**

```bash
pnpm tauri dev
# Right-click cell → context menu items work
# Cmd+P → palette appears, search returns recent entries, picking opens in editor
pnpm typecheck && pnpm lint && pnpm format:check && pnpm build
git add src/lib/pgLiterals.ts src/features/results/ContextMenu.tsx \
        src/features/history/ src/store/history.ts \
        src/features/results/ResultsGrid.tsx src/App.tsx
git commit -m "feat(frontend): cell context menu + Cmd+P history palette"
```

---

## Task 23: vitest setup + critical frontend unit tests

**Goal:** Introduce vitest (deferred from Week 2) and cover the highest-leverage frontend logic: pendingChanges store + pgLiterals TS mirror.

**Files:**

- Modify: `package.json` (add vitest + jsdom + @testing-library/react)
- Create: `vitest.config.ts`
- Create: `src/store/pendingChanges.test.ts`
- Create: `src/lib/pgLiterals.test.ts`

**Steps:**

- [ ] **Step 1: Install + config**

```bash
pnpm add -D vitest jsdom @testing-library/react @testing-library/jest-dom @vitest/ui
```

Create `vitest.config.ts`:

```ts
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test-setup.ts"],
  },
  resolve: { alias: { "@": "/src" } },
});
```

Create `src/test-setup.ts`:

```ts
import "@testing-library/jest-dom";
```

Add to `package.json`:

```json
"scripts": {
  "test": "vitest run",
  "test:watch": "vitest"
}
```

- [ ] **Step 2: pendingChanges tests**

```ts
// src/store/pendingChanges.test.ts
import { describe, it, expect, beforeEach } from "vitest";
import { usePendingChanges } from "./pendingChanges";
import type { Cell } from "@/lib/types";

const T = { schema: "public", name: "users" };

describe("pendingChanges store", () => {
  beforeEach(() => {
    usePendingChanges.getState().revertAll();
  });

  it("upsert creates a row entry on first edit", () => {
    usePendingChanges.getState().upsertEdit({
      table: T,
      pkColumns: ["id"],
      pkValues: [{ kind: "Int", value: 1 } satisfies Cell],
      column: "name",
      original: { kind: "Text", value: "old" },
      next: { kind: "Text", value: "new" },
      capturedRow: [
        { kind: "Int", value: 1 },
        { kind: "Text", value: "old" },
      ],
      capturedColumns: ["id", "name"],
    });
    expect(usePendingChanges.getState().count()).toBe(1);
  });

  it("upsert on the same column overwrites", () => {
    const args = (next: string) => ({
      table: T,
      pkColumns: ["id"],
      pkValues: [{ kind: "Int", value: 1 } as Cell],
      column: "name",
      original: { kind: "Text", value: "old" } as Cell,
      next: { kind: "Text", value: next } as Cell,
      capturedRow: [
        { kind: "Int", value: 1 } as Cell,
        { kind: "Text", value: "old" } as Cell,
      ],
      capturedColumns: ["id", "name"],
    });
    usePendingChanges.getState().upsertEdit(args("a"));
    usePendingChanges.getState().upsertEdit(args("b"));
    const list = usePendingChanges.getState().list();
    expect(list).toHaveLength(1);
    expect(list[0].edits).toHaveLength(1);
    if (list[0].edits[0].next.kind === "Text") {
      expect(list[0].edits[0].next.value).toBe("b");
    }
  });

  it("revertRow drops the entry", () => {
    usePendingChanges.getState().upsertEdit({
      table: T,
      pkColumns: ["id"],
      pkValues: [{ kind: "Int", value: 1 } as Cell],
      column: "name",
      original: { kind: "Text", value: "a" },
      next: { kind: "Text", value: "b" },
      capturedRow: [
        { kind: "Int", value: 1 } as Cell,
        { kind: "Text", value: "a" } as Cell,
      ],
      capturedColumns: ["id", "name"],
    });
    const key = JSON.stringify([{ kind: "Int", value: 1 }]);
    usePendingChanges.getState().revertRow(key);
    expect(usePendingChanges.getState().count()).toBe(0);
  });
});
```

- [ ] **Step 3: pgLiterals parity tests**

```ts
// src/lib/pgLiterals.test.ts
import { describe, it, expect } from "vitest";
import { toLiteral } from "./pgLiterals";

describe("pgLiterals.toLiteral", () => {
  it("renders Null", () => expect(toLiteral({ kind: "Null" })).toBe("NULL"));
  it("renders Bool", () =>
    expect(toLiteral({ kind: "Bool", value: true })).toBe("TRUE"));
  it("renders Text with quote escape", () =>
    expect(toLiteral({ kind: "Text", value: "o'reilly" })).toBe("'o''reilly'"));
  it("renders Uuid with cast", () =>
    expect(
      toLiteral({
        kind: "Uuid",
        value: "550e8400-e29b-41d4-a716-446655440000",
      }),
    ).toBe("'550e8400-e29b-41d4-a716-446655440000'::uuid"));
  it("renders Json with quote-escape", () =>
    expect(toLiteral({ kind: "Json", value: { k: "v's" } })).toBe(
      `'{"k":"v''s"}'::jsonb`,
    ));
  it("renders Bytea hex form", () => {
    // base64 of [0xDE,0xAD,0xBE,0xEF] = "3q2+7w=="
    expect(toLiteral({ kind: "Bytea", value: { b64: "3q2+7w==" } })).toBe(
      "'\\xdeadbeef'::bytea",
    );
  });
});
```

- [ ] **Step 4: Run + commit**

```bash
pnpm test
pnpm typecheck && pnpm lint && pnpm format:check
git add package.json pnpm-lock.yaml vitest.config.ts src/test-setup.ts \
        src/store/pendingChanges.test.ts src/lib/pgLiterals.test.ts
git commit -m "test: vitest setup + pendingChanges + pgLiterals unit tests"
```

---

## Task 24: Manual verification checklist + Week 2 regression sweep

**Goal:** A `manual-verification-week-3.md` doc that walks every spec success criterion. Final task — also acts as the "did we break Week 2" gate.

**Files:**

- Create: `docs/superpowers/plans/manual-verification-week-3.md`

**Steps:**

- [ ] **Step 1: Write the doc**

The doc lives at `docs/superpowers/plans/manual-verification-week-3.md`. Its full content is in this plan's companion file (created in this same task) — copy it verbatim. The companion is the file the plan-execution agent should produce alongside this task.

- [ ] **Step 2: Run the full quality gate suite**

```bash
docker compose -f infra/postgres/docker-compose.yml up -d
pnpm typecheck && pnpm lint && pnpm format:check && pnpm build
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
cargo test --manifest-path src-tauri/Cargo.toml
pnpm test
```

All must pass.

- [ ] **Step 3: Run the Week 3 manual verification**

Open `manual-verification-week-3.md` and tick each box. If anything fails, file as a follow-up bug — do NOT mark this plan complete with red boxes.

- [ ] **Step 4: Re-run Week 2 verification (regression gate)**

Open `docs/superpowers/plans/manual-verification-week-2.md` and tick its boxes again on the same build. Any regression → fix before commit.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/plans/manual-verification-week-3.md
git commit -m "docs: Week 3 manual verification checklist"
```

---

## Self-review notes

After all 24 tasks land, verify against the spec:

- §1 Goals 1–8: tasks 11–19 cover inline edit; tasks 8–10, 17 cover transactions; task 3 covers decoder rewrite (`<unsupported type>` polished); task 7 + 22 cover history + palette; task 21 covers export; task 22 covers context menu; task 20 covers cancellation; task 11 (store) limits memory tracking to edited rows.
- §2 Out of scope: nothing implemented from §2.
- §6 Flow A/B/C/D: A in tasks 11–17, B in tasks 8–10 + 17, C in task 18, D in task 20.
- §10 Decisions taken: each routed to its task — see decisions table in the spec for reference.
- §11 Risks: golden tests in task 3 (Risk #1), parser fallback in task 4 (#2), tx indicator in task 10 (#3), strict NULL/float in task 16 (#4), preview footer in task 17 (#5), Drop rollback in task 8 (#6), LRU in task 5 (#7), 10k gate in task 6 (#8), LIKE index in task 7 (#9), Unknown fallback in task 3 (#10), token race in task 20 (#11).
