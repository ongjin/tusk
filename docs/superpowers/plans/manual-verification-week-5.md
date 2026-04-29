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
