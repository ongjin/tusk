# Week 4 — Manual Verification Checklist

Run after Task 24. Postgres docker must be up:

```bash
docker compose -f infra/postgres/docker-compose.yml up -d
pnpm tauri dev
```

Connection from Week 2/3: `postgres://tusk:tusk@127.0.0.1:55432/tusk_test` via the Direct TCP connection named `local`.

Seed a small workspace before starting:

```sql
DROP TABLE IF EXISTS audit_log  CASCADE;
DROP TABLE IF EXISTS payments   CASCADE;
DROP TABLE IF EXISTS orders     CASCADE;
DROP TABLE IF EXISTS products   CASCADE;
DROP TABLE IF EXISTS users      CASCADE;

CREATE TABLE users (
    id         serial PRIMARY KEY,
    email      text NOT NULL UNIQUE,
    name       text,
    created_at timestamptz DEFAULT now()
);
CREATE TABLE products (
    id    serial PRIMARY KEY,
    name  text NOT NULL,
    price numeric(10,2) NOT NULL
);
CREATE TABLE orders (
    id         serial PRIMARY KEY,
    user_id    int REFERENCES users(id),
    placed_at  timestamptz DEFAULT now(),
    status     text DEFAULT 'pending'
);
CREATE TABLE payments (
    id       serial PRIMARY KEY,
    order_id int REFERENCES orders(id),
    amount   numeric(12,2),
    paid_at  timestamptz
);
CREATE TABLE audit_log (
    id        serial PRIMARY KEY,
    action    text,
    ts        timestamptz DEFAULT now()
);

INSERT INTO users (email, name) VALUES
    ('alice@x.com', 'Alice'), ('bob@x.com', 'Bob'), ('carol@x.com', 'Carol');
INSERT INTO products (name, price) VALUES
    ('Widget', 9.99), ('Gadget', 49.99), ('Doohickey', 4.50);
INSERT INTO orders (user_id, status) VALUES
    (1, 'paid'), (1, 'pending'), (2, 'paid');
INSERT INTO payments (order_id, amount, paid_at) VALUES
    (1, 9.99, now()), (3, 49.99, now());
INSERT INTO audit_log (action) VALUES ('seed'), ('test');
```

---

## A. Setup

- [ ] `pnpm install` completes without errors.
- [ ] `pnpm tauri dev` builds and opens the app window.
- [ ] Docker Postgres is reachable: `SELECT 1` returns `1` in the Tusk editor.
- [ ] All five seeded tables appear in the schema tree under `public`.

→ Pass / Fail / Notes:

---

## B. BYOK round-trip

For each provider: **OpenAI**, **Anthropic**, **Gemini**, **Ollama** (needs local server running):

- [ ] Open Settings → Providers → enter a valid key → Save → "key set" badge appears next to the provider.
- [ ] Click **Test** → toast shows "pong" or a generic acknowledgement within 5 s.
- [ ] Click **Remove** → badge reverts to "no key".
- [ ] Open browser DevTools → Application → LocalStorage → inspect `tusk-ai` → confirm no `sk-...` / raw key substring; only `apiKeyPresent: true`.

Repeat for all four providers (Ollama: skip Test if no local server).

→ Pass / Fail / Notes:

---

## C. Schema index

- [ ] Settings → **Schema Index** → click **Sync** → progress indicator shows "running" then "done — N embedded".
- [ ] Click **Rebuild** → second run shows `skipped_unchanged` incrementing; `embedded ≈ 0`.
- [ ] In editor: `ALTER TABLE users ADD COLUMN nickname text;` → Cmd+Enter → trigger another Sync → only `users` is re-embedded.
- [ ] Click **Clear** → counter resets to "0 / 0", embeddings deleted.

→ Pass / Fail / Notes:

---

## D. Cmd+K — generation

- [ ] No selection: Cmd+K → type "list users from last week" → SQL streams into palette → **Apply** → editor tab populated with the generated SQL.
- [ ] With selection: select an existing SELECT statement in the editor → Cmd+K → type "filter to paid customers" → diff view shows old vs new side-by-side → **Apply** → selection replaced in editor.
- [ ] **Reject** → editor is unchanged (original text preserved).
- [ ] **Re-prompt** → previous stream cancels, input field cleared for fresh entry.
- [ ] Esc closes the palette without applying anything.

→ Pass / Fail / Notes:

---

## E. Cmd+K — tools

- [ ] Prompt: "show columns of the orders table" → palette shows a tool call to `get_table_schema`. Verify via:
  ```bash
  sqlite3 ~/Library/Application\ Support/dev.tusk.app/tusk.db \
    "SELECT tool_calls FROM ai_history ORDER BY rowid DESC LIMIT 1;"
  ```
  Result contains `get_table_schema`.
- [ ] Settings → enable **Sample rows** → prompt referencing data shape (e.g. "what does a typical payment look like?") → verify `sample_rows` tool call is recorded in the same SQLite query above.

→ Pass / Fail / Notes:

---

## F. Destructive guard — typed SQL (run gate)

- [ ] `DROP TABLE foo;` → Cmd+Enter → destructive modal pops → **Cancel** → nothing executed.
- [ ] `DROP TABLE foo;` → modal → **Run anyway** → attempts execute (error expected since table may not exist, but modal must have fired).
- [ ] `DELETE FROM users;` → modal triggers.
- [ ] `DELETE FROM users WHERE id = 1;` → **no modal** (targeted delete).
- [ ] `TRUNCATE audit_log;` → modal triggers.
- [ ] `VACUUM FULL users;` → modal triggers.
- [ ] `-- VACUUM FULL\nSELECT 1` → **no modal** (T7 comment-prefix fix).
- [ ] `SELECT 'VACUUM FULL is fast'` → **no modal** (string literal, not statement).
- [ ] Settings → enable **Strict mode** → `DROP TABLE foo;` → modal now shows a keyword confirmation input → **Run** button disabled until user types `DROP` exactly → button enables → Run executes.

→ Pass / Fail / Notes:

---

## G. Destructive guard — AI generated

- [ ] Cmd+K → prompt "remove all rows from audit_log" → AI generates `DELETE FROM audit_log` or `TRUNCATE audit_log` → **Apply** → Cmd+Enter → destructive modal fires as expected.

→ Pass / Fail / Notes:

---

## H. Error UX

- [ ] Wrong API key → Settings → Test → error toast shows provider-specific message (e.g. "401 Unauthorized").
- [ ] Disconnect network → Cmd+K → toast error within ~30 s timeout.
- [ ] Click **Reject** while generation is streaming → "Generation cancelled" toast.
- [ ] Embedding provider not configured (e.g. set to anthropic with no key) → Cmd+K → toast "Embedding model not set".
- [ ] No active connection (disconnect first) → Cmd+K → toast "No active connection".

→ Pass / Fail / Notes:

---

## I. History integration

- [ ] Cmd+P palette → AI-generated entries show `✦ AI: <prompt preview>` prefix.
- [ ] Click an AI entry → `generated_sql` loads into the editor tab.
- [ ] SQLite verification:
  ```bash
  sqlite3 ~/Library/Application\ Support/dev.tusk.app/tusk.db \
    "SELECT entry_id, provider, generation_model FROM ai_history;"
  ```
  Returns one row per generation performed during this session.

→ Pass / Fail / Notes:

---

## J. Privacy contract

- [ ] Save a fake OpenAI key (e.g. `sk-fake12345`) → inspect `localStorage["tusk-ai"]` → no `sk-fake12345` substring; only `apiKeyPresent: true`.
- [ ] Settings → Test → open DevTools Network tab → locate the outbound request → confirm `Authorization` header contains the actual key (in-flight only).
- [ ] Close app → reopen → "key set" badge persists for the saved provider; raw key not visible in any Zustand state dump (DevTools → `window.__zustand_state` or equivalent).

→ Pass / Fail / Notes:

---

## K. Regression — Week 2 / 3

- [ ] Connect / disconnect / reconnect via Direct TCP: works.
- [ ] Schema tree: lazy loads on expand.
- [ ] Cmd+Enter execute: works.
- [ ] Inline cell edit + Submit: works.
- [ ] Auto-commit OFF → INSERT → Commit: persists; Rollback: reverts.
- [ ] Cmd+P history palette: works.
- [ ] CSV export: file written, opens correctly.
- [ ] Cell context menu Copy / Filter by value: works.
- [ ] Conflict detection modal (Strict mode): fires on concurrent edit.

→ Pass / Fail / Notes:
