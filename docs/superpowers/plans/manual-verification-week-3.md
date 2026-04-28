# Week 3 — Manual Verification Checklist

Run after Task 24. Postgres docker must be up:

```bash
docker compose -f infra/postgres/docker-compose.yml up -d
pnpm tauri dev
```

Connection from Week 2: `postgres://tusk:tusk@127.0.0.1:55432/tusk_test` via the Direct TCP connection named `local`.

Seed a small workspace before starting:

```sql
DROP TABLE IF EXISTS w3_users CASCADE;
DROP TABLE IF EXISTS w3_orders CASCADE;
DROP TYPE  IF EXISTS w3_mood;
CREATE TYPE w3_mood AS ENUM ('sad','ok','happy');
CREATE TABLE w3_users (
    id        int primary key,
    email     text NOT NULL,
    age       int,
    mood      w3_mood,
    payload   jsonb,
    created   timestamptz DEFAULT now(),
    avatar    bytea,
    score     numeric(10,2)
);
CREATE TABLE w3_orders (
    id       int primary key,
    user_id  int REFERENCES w3_users(id),
    total    numeric(12,2),
    placed   timestamp
);
INSERT INTO w3_users (id, email, age, mood, payload, avatar, score) VALUES
    (1, 'a@x', 30, 'happy', '{"k":1}', '\\xDEADBEEF', 12.34),
    (2, 'b@x', 25, 'ok',    '{"k":2}', '\\x01',       55.00),
    (3, 'c@x', NULL, NULL,  NULL,      NULL,           NULL);
INSERT INTO w3_orders VALUES (10, 1, 99.99, '2026-04-01 12:00:00'),
                             (11, 2, 49.50, '2026-04-15 09:30:00');
```

---

## A. Decoder + result rendering (Task 3 + Task 6)

- [ ] `SELECT * FROM w3_users` shows every column without `<unsupported type>`.
      Specifically: `id` int, `email` text, `mood` enum, `payload` jsonb, `created` timestamptz,
      `avatar` bytea (`\\x` prefix), `score` numeric.
- [ ] NULL cells render in italic gray.
- [ ] `SELECT * FROM (SELECT 1) sub` shows result with `🔒` indicator
      (read-only, reason `multi-table` or `parser-failed`).
- [ ] `SELECT u.id FROM w3_users u JOIN w3_orders o ON o.user_id = u.id` → `🔒`.
- [ ] `SELECT count(*) FROM w3_users` → `🔒` (computed).
- [ ] Table without PK (create one ad hoc: `CREATE TABLE w3_nopk (a int)`) → `🔒` (no-pk).

## B. Inline editing — widgets (Tasks 11–15)

For each widget, double-click a cell, change value, hit Enter, Submit, then verify via `psql`.

- [ ] **Text** (email): change to `'aa@x'` → COMMIT → reflects in DB. Multiline toggle works.
- [ ] **Int** (age): change to `99` → COMMIT. Out-of-range like `5e9` rejected.
- [ ] **Numeric** (score): `12.345` → COMMIT. Non-numeric `"abc"` rejected.
- [ ] **Bool** (no column yet — add `online bool` to `w3_users` and re-test): toggle → COMMIT.
- [ ] **Date** (`SELECT id, created::date AS d FROM w3_users` — note: this is read-only because of cast,
      so add a real date column for a real round-trip): widget shows native picker, commit works.
- [ ] **Time** / **Timetz**: native picker, commit works.
- [ ] **Timestamp** / **Timestamptz** (`created`): datetime-local picker shows local TZ offset; commit
      preserves moment (verify in psql `created AT TIME ZONE 'UTC'`).
- [ ] **Uuid**: Generate button fills a fresh v4 UUID. Invalid hex rejected.
- [ ] **Json** (`payload`): Monaco mini editor opens with formatted JSON; invalid JSON rejected.
- [ ] **Bytea** (`avatar`): hex / base64 toggle round-trips correctly. Saving `\\x01ff` shows as
      base64 `Af8=` after refetch.
- [ ] **Vector**: read-only — double-click shows "vector(N) — read-only in this version".
- [ ] **Enum** (`mood`): dropdown shows `sad / ok / happy`. Commit lands valid value.
- [ ] **FK** (`w3_orders.user_id`): dropdown lists users with display column (email). Search filters.
- [ ] **Set NULL**: every nullable widget shows the button; clicking it commits `Null`.

## C. Submit + Preview + atomic semantics (Tasks 16–18)

- [ ] Edit two cells across two rows → "2 pending" badge appears.
- [ ] **Preview** modal shows two SQL statements with literal values inlined.
      The footer note about parameterized binds is visible.
- [ ] **Submit** runs both → toast "2 row(s) updated". Pending badge clears.
- [ ] **Revert** restores all pending edits visually (DB unchanged).
- [ ] Conflict detection (Strict mode): 1. Toggle mode dropdown to `Strict`. 2. In a separate `psql` session, change `email` of row 1 to `'remote@x'`. 3. In Tusk, edit row 1 `email` to `'tusk@x'` → Submit. 4. ConflictModal appears: shows `your edit: tusk@x` vs `server now: remote@x`. 5. Verify atomic rollback: any other batch in the same Submit must NOT have applied. 6. Try **Force overwrite** → succeeds (since pkOnly bypasses captured-row check). 7. Re-run with **Re-edit on top of server** → captured row updated; subsequent Submit succeeds without conflict.
- [ ] PkOnly mode: same scenario succeeds without the modal (last-writer-wins).

## D. INSERT / DELETE row (Task 19)

- [ ] Click `+ Row` → ghost row appears at the bottom. Fill PK + email → Submit → row exists in DB.
- [ ] Click ✕ on existing row 3 → marked for deletion (visual hint) → Submit → row gone.

## E. Explicit transaction mode (Tasks 8–10, 13)

- [ ] Toggle Auto-commit OFF → 🟡 Transaction (0 stmts) appears in app bar.
- [ ] Run `INSERT INTO w3_users (id,email) VALUES (100,'tx@x');` (the count goes to 1).
- [ ] In a separate psql session: `SELECT count(*) FROM w3_users WHERE id=100` returns 0 (not committed).
- [ ] Run `UPDATE w3_users SET age = 1 WHERE id = 1;` (count → 2).
- [ ] Inline-edit row 2 email → Submit (count → 3).
- [ ] Click **Commit** → indicator gone. psql now sees the changes.
- [ ] Repeat with **Rollback** → no changes persisted.
- [ ] Tx side panel (when active): shows `1. INSERT ...`, `2. UPDATE ...`, `3. UPDATE ... [inline]` with truncated SQL.
- [ ] Tx aborted state: in tx, run `INSERT INTO w3_users VALUES (1,'dup');` → fails with PK conflict;
      next statement gets "Transaction aborted — only ROLLBACK is allowed". Click Rollback → tx ends, error cleared.
- [ ] Shutdown confirm: with active tx, click app close → modal appears with Commit / Rollback / Cancel.
      Cancel keeps app open. Rollback commits nothing and exits.

## F. Query cancel (Task 20)

- [ ] Run `SELECT pg_sleep(20);` — after ~500ms, "Running query..." toast appears with [Cancel].
- [ ] Click Cancel → query errors with "canceling statement due to user request" (TuskError::QueryCancelled).
- [ ] history_entry shows status `cancelled`.
- [ ] Cancel inside a tx: same flow, but afterwards the tx is in aborted state — UI banner says so. Rollback to recover.

## G. Export (Task 21)

- [ ] Run `SELECT * FROM w3_users` → click Export → CSV → file written, opens in spreadsheet correctly with NULL as empty.
- [ ] Same → JSON → file is array of objects.
- [ ] Same → SQL INSERT → file contains one INSERT per row, all literals correctly quoted (inspect bytea cell, jsonb, enum).
- [ ] CSV with BOM toggle → file starts with `EF BB BF` bytes (`xxd`).

## H. Cell context menu + Cmd+P palette (Task 22)

- [ ] Right-click a cell → menu: Copy / Copy as INSERT / Set NULL (if nullable) / Filter by this value.
- [ ] Copy → clipboard contains `cellToText` form.
- [ ] Copy as INSERT → clipboard contains `INSERT INTO "public"."w3_users" (...) VALUES (...);` with proper literals.
- [ ] Set NULL on nullable column → adds pending edit `Null`. On non-nullable column the option is missing.
- [ ] Filter by this value → editor tab updated with `WHERE "<col>" = <literal>`.
- [ ] Cmd+P (Ctrl+P on Linux) → palette opens. Type `INSERT` → list filters. Click an entry → editor tab loads its full SQL.
- [ ] Tx-grouped entries show `(tx · N stmts)` badge; clicking expands them in the palette / loads them joined by `;`.

## I. Settings (PkOnly / Strict toggle)

- [ ] Toggle is visible on editable result grids only.
- [ ] Setting persists across app restart.

## J. Result size gate (Risk #8)

- [ ] `SELECT generate_series(1, 12000) AS n;` → grid loads but ✏️ indicator is replaced by 🔒 with reason `too-large`.

## K. Week 2 regression gate

Re-run every box in `manual-verification-week-2.md` on the same build.

- [ ] Direct TCP connection still works.
- [ ] SSH alias / SSH manual still works.
- [ ] Editor tabs / shortcuts unchanged.
- [ ] Result grid sorting / NULL rendering unchanged.
- [ ] Auto-LIMIT toggle unchanged.
- [ ] Connection-lost toast still fires when tunnel killed.
