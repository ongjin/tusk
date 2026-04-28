# Week 2 — Postgres MVP + SSH Tunnel — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire end-to-end Postgres connectivity (Direct TCP + SSH tunnel) with schema sidebar, Monaco SQL editor, and TanStack result grid — the "open app → first query in 90 seconds" path.

**Architecture:** sqlx pools managed in a Tauri-side `Mutex<HashMap<ConnectionId, ActiveConnection>>`. SSH tunnels are system `ssh` child processes spawned with `-N -L`; effective config resolved via `ssh -G <alias>`. Metadata in rusqlite, secrets in OS keychain. Frontend is React 19 + zustand + shadcn/ui + Monaco + TanStack.

**Tech Stack:** Tauri 2, sqlx 0.8, rusqlite 0.32, keyring 3, tokio, anyhow/thiserror, React 19, zustand, shadcn/ui, Monaco, TanStack Table + react-virtual.

**Reference spec:** `docs/superpowers/specs/2026-04-28-week-2-postgres-mvp-design.md`.

**Working dir:** `/Users/cyj/workspace/personal/tusk` on `main`.

**Quality gates between tasks:**

```
pnpm typecheck && pnpm lint && pnpm format:check
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
cargo test --manifest-path src-tauri/Cargo.toml
```

Run only the gates relevant to the task (Rust tasks → rust:\* + cargo test; Frontend tasks → typecheck/lint/format + `pnpm build`). Last task runs the full set.

---

## Task 1: Foundation — errors, secrets, app paths

**Goal:** Land cross-cutting building blocks every later task depends on.

**Files:**

- Create: `src-tauri/src/errors.rs`
- Create: `src-tauri/src/secrets.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`

**Steps:**

- [ ] **Step 1: Add Cargo dependencies**

Edit `src-tauri/Cargo.toml`, append to `[dependencies]`:

```toml
anyhow = "1"
thiserror = "2"
keyring = "3"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
tokio = { version = "1", features = ["full"] }
```

Verify: `cargo check --manifest-path src-tauri/Cargo.toml` succeeds.

- [ ] **Step 2: Write `errors.rs`**

```rust
// src-tauri/src/errors.rs
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug, Serialize)]
#[serde(tag = "kind", content = "message")]
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
}

impl From<anyhow::Error> for TuskError {
    fn from(e: anyhow::Error) -> Self {
        TuskError::Internal(format!("{e:#}"))
    }
}

pub type TuskResult<T> = Result<T, TuskError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_uses_tagged_repr() {
        let err = TuskError::Connection("nope".into());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, r#"{"kind":"Connection","message":"nope"}"#);
    }

    #[test]
    fn from_anyhow_becomes_internal() {
        let any = anyhow::anyhow!("boom");
        let err: TuskError = any.into();
        assert!(matches!(err, TuskError::Internal(_)));
    }
}
```

- [ ] **Step 3: Wire `errors` module into `lib.rs`**

Edit `src-tauri/src/lib.rs`, add at top of file (after existing `mod commands;`):

```rust
pub mod errors;
pub mod secrets;
```

- [ ] **Step 4: Write `secrets.rs`**

```rust
// src-tauri/src/secrets.rs
use crate::errors::{TuskError, TuskResult};

const SERVICE: &str = "tusk";

fn entry(connection_id: &str) -> TuskResult<keyring::Entry> {
    keyring::Entry::new(SERVICE, &format!("conn:{connection_id}"))
        .map_err(|e| TuskError::Secrets(e.to_string()))
}

pub fn set_password(connection_id: &str, password: &str) -> TuskResult<()> {
    entry(connection_id)?
        .set_password(password)
        .map_err(|e| TuskError::Secrets(e.to_string()))
}

pub fn get_password(connection_id: &str) -> TuskResult<Option<String>> {
    match entry(connection_id)?.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(TuskError::Secrets(e.to_string())),
    }
}

pub fn delete_password(connection_id: &str) -> TuskResult<()> {
    match entry(connection_id)?.delete_credential() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(TuskError::Secrets(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trips a password through the OS keychain. Skipped in CI when no
    /// keychain backend is available (the keyring crate falls back gracefully
    /// on Linux only when configured; on macOS the test runs against the
    /// real login keychain).
    #[test]
    fn set_get_delete_roundtrip() {
        let id = format!("test-{}", uuid::Uuid::new_v4());
        if set_password(&id, "hunter2").is_err() {
            // No usable backend — treat as skipped.
            return;
        }
        assert_eq!(get_password(&id).unwrap().as_deref(), Some("hunter2"));
        delete_password(&id).unwrap();
        assert_eq!(get_password(&id).unwrap(), None);
    }
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml errors
cargo test --manifest-path src-tauri/Cargo.toml secrets
```

Expected: 3 tests run, all pass (or `set_get_delete_roundtrip` is silently skipped on a backendless env).

- [ ] **Step 6: Quality gates**

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
```

All three must pass.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/errors.rs src-tauri/src/secrets.rs src-tauri/src/lib.rs
git commit -m "feat(rust): add TuskError + keyring-backed secrets

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: rusqlite state — connection metadata

**Goal:** Persist connection metadata (id, name, host, port, user, db, ssh\_\*) in a single SQLite file at `app_data_dir()/tusk.db`.

**Files:**

- Create: `src-tauri/src/db/mod.rs`
- Create: `src-tauri/src/db/state.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`

**Steps:**

- [ ] **Step 1: Add rusqlite to Cargo.toml**

Append to `[dependencies]`:

```toml
rusqlite = { version = "0.32", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

(`serde`/`serde_json` already present — skip duplicates.)

- [ ] **Step 2: Create `db/mod.rs`**

```rust
// src-tauri/src/db/mod.rs
pub mod state;
```

- [ ] **Step 3: Write `db/state.rs`**

```rust
// src-tauri/src/db/state.rs
use std::path::Path;
use std::sync::Mutex;

use chrono::Utc;
use rusqlite::{params, Connection as Sqlite};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SshKind {
    None,
    Alias,
    Manual,
}

impl SshKind {
    fn as_str(&self) -> &'static str {
        match self {
            SshKind::None => "none",
            SshKind::Alias => "alias",
            SshKind::Manual => "manual",
        }
    }

    fn parse(s: &str) -> TuskResult<Self> {
        match s {
            "none" => Ok(SshKind::None),
            "alias" => Ok(SshKind::Alias),
            "manual" => Ok(SshKind::Manual),
            other => Err(TuskError::State(format!("unknown ssh_kind '{other}'"))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRecord {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub db_user: String,
    pub database: String,
    pub ssl_mode: String,
    pub ssh_kind: SshKind,
    pub ssh_alias: Option<String>,
    pub ssh_host: Option<String>,
    pub ssh_port: Option<u16>,
    pub ssh_user: Option<String>,
    pub ssh_key_path: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewConnection {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub db_user: String,
    pub database: String,
    pub ssl_mode: String,
    pub ssh_kind: SshKind,
    pub ssh_alias: Option<String>,
    pub ssh_host: Option<String>,
    pub ssh_port: Option<u16>,
    pub ssh_user: Option<String>,
    pub ssh_key_path: Option<String>,
}

pub struct StateStore {
    db: Mutex<Sqlite>,
}

impl StateStore {
    pub fn open<P: AsRef<Path>>(path: P) -> TuskResult<Self> {
        let db = Sqlite::open(path).map_err(|e| TuskError::State(e.to_string()))?;
        let store = Self { db: Mutex::new(db) };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> TuskResult<Self> {
        let db = Sqlite::open_in_memory().map_err(|e| TuskError::State(e.to_string()))?;
        let store = Self { db: Mutex::new(db) };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> TuskResult<()> {
        let db = self.db.lock().expect("state lock poisoned");
        db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS connections (
                id           TEXT PRIMARY KEY,
                name         TEXT NOT NULL,
                host         TEXT NOT NULL,
                port         INTEGER NOT NULL,
                db_user      TEXT NOT NULL,
                database     TEXT NOT NULL,
                ssl_mode     TEXT NOT NULL,
                ssh_kind     TEXT NOT NULL,
                ssh_alias    TEXT,
                ssh_host     TEXT,
                ssh_port     INTEGER,
                ssh_user     TEXT,
                ssh_key_path TEXT,
                created_at   INTEGER NOT NULL,
                updated_at   INTEGER NOT NULL
            );
            "#,
        )
        .map_err(|e| TuskError::State(e.to_string()))?;
        Ok(())
    }

    pub fn insert(&self, new: NewConnection) -> TuskResult<ConnectionRecord> {
        let now = Utc::now().timestamp();
        let id = Uuid::new_v4().to_string();
        let record = ConnectionRecord {
            id: id.clone(),
            name: new.name,
            host: new.host,
            port: new.port,
            db_user: new.db_user,
            database: new.database,
            ssl_mode: new.ssl_mode,
            ssh_kind: new.ssh_kind,
            ssh_alias: new.ssh_alias,
            ssh_host: new.ssh_host,
            ssh_port: new.ssh_port,
            ssh_user: new.ssh_user,
            ssh_key_path: new.ssh_key_path,
            created_at: now,
            updated_at: now,
        };
        let db = self.db.lock().expect("state lock poisoned");
        db.execute(
            "INSERT INTO connections
             (id, name, host, port, db_user, database, ssl_mode, ssh_kind,
              ssh_alias, ssh_host, ssh_port, ssh_user, ssh_key_path,
              created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            params![
                record.id,
                record.name,
                record.host,
                record.port,
                record.db_user,
                record.database,
                record.ssl_mode,
                record.ssh_kind.as_str(),
                record.ssh_alias,
                record.ssh_host,
                record.ssh_port,
                record.ssh_user,
                record.ssh_key_path,
                record.created_at,
                record.updated_at,
            ],
        )
        .map_err(|e| TuskError::State(e.to_string()))?;
        Ok(record)
    }

    pub fn list(&self) -> TuskResult<Vec<ConnectionRecord>> {
        let db = self.db.lock().expect("state lock poisoned");
        let mut stmt = db
            .prepare(
                "SELECT id, name, host, port, db_user, database, ssl_mode, ssh_kind,
                        ssh_alias, ssh_host, ssh_port, ssh_user, ssh_key_path,
                        created_at, updated_at
                 FROM connections ORDER BY name",
            )
            .map_err(|e| TuskError::State(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, u16>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<u16>>(10)?,
                    row.get::<_, Option<String>>(11)?,
                    row.get::<_, Option<String>>(12)?,
                    row.get::<_, i64>(13)?,
                    row.get::<_, i64>(14)?,
                ))
            })
            .map_err(|e| TuskError::State(e.to_string()))?;

        let mut out = Vec::new();
        for row in rows {
            let r = row.map_err(|e| TuskError::State(e.to_string()))?;
            out.push(ConnectionRecord {
                id: r.0,
                name: r.1,
                host: r.2,
                port: r.3,
                db_user: r.4,
                database: r.5,
                ssl_mode: r.6,
                ssh_kind: SshKind::parse(&r.7)?,
                ssh_alias: r.8,
                ssh_host: r.9,
                ssh_port: r.10,
                ssh_user: r.11,
                ssh_key_path: r.12,
                created_at: r.13,
                updated_at: r.14,
            });
        }
        Ok(out)
    }

    pub fn get(&self, id: &str) -> TuskResult<Option<ConnectionRecord>> {
        let all = self.list()?;
        Ok(all.into_iter().find(|c| c.id == id))
    }

    pub fn delete(&self, id: &str) -> TuskResult<()> {
        let db = self.db.lock().expect("state lock poisoned");
        db.execute("DELETE FROM connections WHERE id = ?1", params![id])
            .map_err(|e| TuskError::State(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> NewConnection {
        NewConnection {
            name: name.into(),
            host: "127.0.0.1".into(),
            port: 5432,
            db_user: "postgres".into(),
            database: "postgres".into(),
            ssl_mode: "prefer".into(),
            ssh_kind: SshKind::None,
            ssh_alias: None,
            ssh_host: None,
            ssh_port: None,
            ssh_user: None,
            ssh_key_path: None,
        }
    }

    #[test]
    fn insert_then_list_returns_record() {
        let store = StateStore::open_in_memory().unwrap();
        let inserted = store.insert(fixture("local")).unwrap();
        let listed = store.list().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, inserted.id);
        assert_eq!(listed[0].name, "local");
    }

    #[test]
    fn delete_removes_record() {
        let store = StateStore::open_in_memory().unwrap();
        let r = store.insert(fixture("local")).unwrap();
        store.delete(&r.id).unwrap();
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn ssh_kind_alias_round_trips() {
        let store = StateStore::open_in_memory().unwrap();
        let mut new = fixture("oci-db");
        new.ssh_kind = SshKind::Alias;
        new.ssh_alias = Some("oci-db".into());
        let inserted = store.insert(new).unwrap();
        let fetched = store.get(&inserted.id).unwrap().unwrap();
        assert_eq!(fetched.ssh_kind, SshKind::Alias);
        assert_eq!(fetched.ssh_alias.as_deref(), Some("oci-db"));
    }
}
```

- [ ] **Step 4: Wire `db` module into `lib.rs`**

Edit `src-tauri/src/lib.rs`:

```rust
pub mod db;
pub mod errors;
pub mod secrets;
mod commands;
```

- [ ] **Step 5: Run tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml db::state
```

Expected: 3 tests pass.

- [ ] **Step 6: Quality gates**

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
```

- [ ] **Step 7: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/db src-tauri/src/lib.rs
git commit -m "feat(rust): add rusqlite state store for connection metadata

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: DB pool + Direct TCP commands

**Goal:** sqlx pool registry + Tauri commands for `connect_direct`, `disconnect`, `list_connections`, `add_connection`, `execute_query`.

**Files:**

- Create: `src-tauri/src/db/pool.rs`
- Create: `src-tauri/src/commands/connections.rs`
- Create: `src-tauri/src/commands/query.rs`
- Modify: `src-tauri/src/db/mod.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/tests/postgres_integration.rs`
- Create: `infra/postgres/docker-compose.yml`

**Steps:**

- [ ] **Step 1: Add sqlx**

Append to `[dependencies]`:

```toml
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono", "json", "macros"] }
```

- [ ] **Step 2: Add docker-compose for integration tests**

Create `infra/postgres/docker-compose.yml`:

```yaml
services:
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: tusk
      POSTGRES_PASSWORD: tusk
      POSTGRES_DB: tusk_test
    ports:
      - "55432:5432"
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U tusk -d tusk_test"]
      interval: 1s
      timeout: 3s
      retries: 30
```

Bring it up once to verify:

```bash
docker compose -f infra/postgres/docker-compose.yml up -d
docker compose -f infra/postgres/docker-compose.yml exec -T postgres psql -U tusk -d tusk_test -c 'SELECT 1'
```

Leave it running for the rest of this task.

- [ ] **Step 3: Write `db/pool.rs`**

```rust
// src-tauri/src/db/pool.rs
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use sqlx::PgPool;

use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Clone)]
pub struct DirectConnectSpec {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
    pub ssl_mode: String,
}

pub struct ActiveConnection {
    pub pool: PgPool,
    // Tunnel handle slot — populated in Task 6.
    pub tunnel: Option<TunnelSlot>,
}

/// Placeholder until Task 6 adds the real tunnel handle.
pub struct TunnelSlot;

#[derive(Default)]
pub struct ConnectionRegistry {
    inner: Mutex<HashMap<String, ActiveConnection>>,
}

impl ConnectionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn connect_direct(
        &self,
        connection_id: &str,
        spec: DirectConnectSpec,
    ) -> TuskResult<()> {
        let opts = PgConnectOptions::new()
            .host(&spec.host)
            .port(spec.port)
            .username(&spec.user)
            .password(&spec.password)
            .database(&spec.database)
            .ssl_mode(parse_ssl_mode(&spec.ssl_mode)?);

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(opts)
            .await
            .map_err(|e| TuskError::Connection(e.to_string()))?;

        let mut guard = self.inner.lock().expect("registry poisoned");
        guard.insert(
            connection_id.to_string(),
            ActiveConnection { pool, tunnel: None },
        );
        Ok(())
    }

    pub fn disconnect(&self, connection_id: &str) -> TuskResult<()> {
        let mut guard = self.inner.lock().expect("registry poisoned");
        guard.remove(connection_id);
        Ok(())
    }

    pub fn pool(&self, connection_id: &str) -> TuskResult<PgPool> {
        let guard = self.inner.lock().expect("registry poisoned");
        guard
            .get(connection_id)
            .map(|c| c.pool.clone())
            .ok_or_else(|| TuskError::Connection(format!("not connected: {connection_id}")))
    }

    pub fn is_connected(&self, connection_id: &str) -> bool {
        self.inner.lock().expect("registry poisoned").contains_key(connection_id)
    }
}

fn parse_ssl_mode(s: &str) -> TuskResult<PgSslMode> {
    Ok(match s {
        "disable" => PgSslMode::Disable,
        "allow" => PgSslMode::Allow,
        "prefer" => PgSslMode::Prefer,
        "require" => PgSslMode::Require,
        "verify-ca" => PgSslMode::VerifyCa,
        "verify-full" => PgSslMode::VerifyFull,
        other => {
            return Err(TuskError::Connection(format!(
                "unknown ssl_mode '{other}'"
            )))
        }
    })
}
```

Update `db/mod.rs`:

```rust
pub mod pool;
pub mod state;
```

- [ ] **Step 4: Write `commands/query.rs`**

```rust
// src-tauri/src/commands/query.rs
use std::time::Instant;

use serde::Serialize;
use sqlx::{Column, Row, TypeInfo};
use tauri::State;

use crate::db::pool::ConnectionRegistry;
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Serialize)]
pub struct ColumnMeta {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Vec<serde_json::Value>>,
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

    let columns = rows
        .first()
        .map(|r| {
            r.columns()
                .iter()
                .map(|c| ColumnMeta {
                    name: c.name().to_string(),
                    type_name: c.type_info().name().to_string(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut data = Vec::with_capacity(rows.len());
    for row in &rows {
        let mut cells = Vec::with_capacity(row.len());
        for i in 0..row.len() {
            cells.push(decode_cell(row, i));
        }
        data.push(cells);
    }

    Ok(QueryResult {
        columns,
        rows: data.clone(),
        duration_ms,
        row_count: data.len(),
    })
}

fn decode_cell(row: &sqlx::postgres::PgRow, idx: usize) -> serde_json::Value {
    use sqlx::postgres::PgValueRef;
    use sqlx::ValueRef;

    // Try the most common types in order; fall back to a string repr.
    if let Ok(v) = row.try_get::<Option<bool>, _>(idx) {
        return serde_json::to_value(v).unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<i64>, _>(idx) {
        return serde_json::to_value(v).unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<f64>, _>(idx) {
        return serde_json::to_value(v).unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<String>, _>(idx) {
        return serde_json::to_value(v).unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<serde_json::Value>, _>(idx) {
        return v.unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(idx) {
        return serde_json::to_value(v.map(|d| d.to_rfc3339()))
            .unwrap_or(serde_json::Value::Null);
    }
    // Last resort — encode raw bytes as null (caller can render "<binary>").
    let raw = row.try_get_raw(idx);
    match raw {
        Ok(value) if value.is_null() => serde_json::Value::Null,
        _ => serde_json::Value::String("<unsupported type>".into()),
    }
}
```

- [ ] **Step 5: Write `commands/connections.rs`**

```rust
// src-tauri/src/commands/connections.rs
use serde::Serialize;
use tauri::State;

use crate::db::pool::{ConnectionRegistry, DirectConnectSpec};
use crate::db::state::{ConnectionRecord, NewConnection, SshKind, StateStore};
use crate::errors::{TuskError, TuskResult};
use crate::secrets;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionListItem {
    #[serde(flatten)]
    pub record: ConnectionRecord,
    pub connected: bool,
}

#[tauri::command]
pub fn list_connections(
    store: State<'_, StateStore>,
    registry: State<'_, ConnectionRegistry>,
) -> TuskResult<Vec<ConnectionListItem>> {
    let records = store.list()?;
    Ok(records
        .into_iter()
        .map(|r| ConnectionListItem {
            connected: registry.is_connected(&r.id),
            record: r,
        })
        .collect())
}

#[tauri::command]
pub fn add_connection(
    store: State<'_, StateStore>,
    new: NewConnection,
    password: String,
) -> TuskResult<ConnectionRecord> {
    let record = store.insert(new)?;
    secrets::set_password(&record.id, &password)?;
    Ok(record)
}

#[tauri::command]
pub fn delete_connection(
    store: State<'_, StateStore>,
    registry: State<'_, ConnectionRegistry>,
    id: String,
) -> TuskResult<()> {
    registry.disconnect(&id)?;
    secrets::delete_password(&id)?;
    store.delete(&id)?;
    Ok(())
}

#[tauri::command]
pub async fn connect(
    store: State<'_, StateStore>,
    registry: State<'_, ConnectionRegistry>,
    id: String,
) -> TuskResult<()> {
    let record = store
        .get(&id)?
        .ok_or_else(|| TuskError::Connection(format!("unknown connection {id}")))?;
    let password = secrets::get_password(&record.id)?
        .ok_or_else(|| TuskError::Secrets("no password stored".into()))?;

    match record.ssh_kind {
        SshKind::None => {
            let spec = DirectConnectSpec {
                host: record.host,
                port: record.port,
                user: record.db_user,
                password,
                database: record.database,
                ssl_mode: record.ssl_mode,
            };
            registry.connect_direct(&id, spec).await?;
            Ok(())
        }
        SshKind::Alias | SshKind::Manual => {
            // Wired up in Task 6.
            Err(TuskError::Tunnel("SSH-backed connect not yet wired".into()))
        }
    }
}

#[tauri::command]
pub fn disconnect(
    registry: State<'_, ConnectionRegistry>,
    id: String,
) -> TuskResult<()> {
    registry.disconnect(&id)
}
```

- [ ] **Step 6: Update `commands/mod.rs`**

```rust
pub mod connections;
pub mod meta;
pub mod query;
```

- [ ] **Step 7: Wire State + handlers in `lib.rs`**

```rust
// src-tauri/src/lib.rs
pub mod db;
pub mod errors;
pub mod secrets;
mod commands;

use crate::db::pool::ConnectionRegistry;
use crate::db::state::StateStore;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data = app
                .path()
                .app_data_dir()
                .expect("app_data_dir unavailable");
            std::fs::create_dir_all(&app_data).ok();
            let store = StateStore::open(app_data.join("tusk.db"))
                .expect("failed to open state store");
            app.manage(store);
            app.manage(ConnectionRegistry::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::meta::greet,
            commands::connections::list_connections,
            commands::connections::add_connection,
            commands::connections::delete_connection,
            commands::connections::connect,
            commands::connections::disconnect,
            commands::query::execute_query,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Add `use tauri::Manager;` at the top (needed for `app.path()` and `app.manage`).

- [ ] **Step 8: Write integration test**

Create `src-tauri/tests/postgres_integration.rs`:

```rust
//! Requires `docker compose -f infra/postgres/docker-compose.yml up -d`.
//! Skipped automatically if the test DB is unreachable.

use std::env;

use tusk_lib::db::pool::{ConnectionRegistry, DirectConnectSpec};

fn skip_if_no_postgres() -> bool {
    use std::net::TcpStream;
    use std::time::Duration;
    let host = env::var("TUSK_TEST_PG_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = env::var("TUSK_TEST_PG_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(55432);
    TcpStream::connect_timeout(&format!("{host}:{port}").parse().unwrap(), Duration::from_secs(1))
        .is_err()
}

#[tokio::test]
async fn connect_and_select_one() {
    if skip_if_no_postgres() {
        eprintln!("Postgres not running on 127.0.0.1:55432 — test skipped");
        return;
    }
    let registry = ConnectionRegistry::new();
    let spec = DirectConnectSpec {
        host: "127.0.0.1".into(),
        port: 55432,
        user: "tusk".into(),
        password: "tusk".into(),
        database: "tusk_test".into(),
        ssl_mode: "disable".into(),
    };
    registry.connect_direct("test", spec).await.unwrap();
    let pool = registry.pool("test").unwrap();
    let row: (i32,) = sqlx::query_as("SELECT 1").fetch_one(&pool).await.unwrap();
    assert_eq!(row.0, 1);
    registry.disconnect("test").unwrap();
}
```

- [ ] **Step 9: Run integration test**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test postgres_integration
```

Expected: PASS (or "skipped" message if Postgres isn't running).

- [ ] **Step 10: Quality gates**

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
cargo test --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 11: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src src-tauri/tests infra/postgres
git commit -m "feat(rust): sqlx pool registry + connection/query commands

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Frontend connection UI (Direct TCP)

**Goal:** Modal to add a connection, list connections in a sidebar, click to connect, run an ad-hoc query, dump results to a `<pre>` so we can verify end-to-end before the editor lands.

**Files:**

- Modify: `src/lib/tauri.ts`
- Create: `src/lib/types.ts`
- Create: `src/store/connections.ts`
- Create: `src/features/connections/ConnectionForm.tsx`
- Create: `src/features/connections/ConnectionList.tsx`
- Modify: `src/App.tsx`
- Modify: `src/main.tsx`
- shadcn additions: `dialog`, `input`, `label`, `select`, `tabs`, `sonner`

**Steps:**

- [ ] **Step 1: Add shadcn primitives**

```bash
pnpm dlx shadcn@latest add dialog input label select sonner --yes
```

(`tabs` lands in Task 7.)

- [ ] **Step 2: Write `src/lib/types.ts`**

```ts
export type SshKind = "None" | "Alias" | "Manual";

export interface ConnectionRecord {
  id: string;
  name: string;
  host: string;
  port: number;
  dbUser: string;
  database: string;
  sslMode: string;
  sshKind: SshKind;
  sshAlias: string | null;
  sshHost: string | null;
  sshPort: number | null;
  sshUser: string | null;
  sshKeyPath: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface NewConnection {
  name: string;
  host: string;
  port: number;
  dbUser: string;
  database: string;
  sslMode: string;
  sshKind: SshKind;
  sshAlias: string | null;
  sshHost: string | null;
  sshPort: number | null;
  sshUser: string | null;
  sshKeyPath: string | null;
}

export interface ConnectionListItem extends ConnectionRecord {
  connected: boolean;
}

export interface ColumnMeta {
  name: string;
  type_name: string;
}

export interface QueryResult {
  columns: ColumnMeta[];
  rows: Array<Array<unknown>>;
  durationMs: number;
  rowCount: number;
}

export interface TuskErrorPayload {
  kind:
    | "Connection"
    | "Query"
    | "Tunnel"
    | "Ssh"
    | "State"
    | "Secrets"
    | "Internal";
  message: string;
}

export class TuskError extends Error {
  kind: TuskErrorPayload["kind"];
  constructor(payload: TuskErrorPayload) {
    super(payload.message);
    this.kind = payload.kind;
    this.name = `TuskError(${payload.kind})`;
  }
}
```

- [ ] **Step 3: Replace `src/lib/tauri.ts`**

```ts
import { invoke as rawInvoke } from "@tauri-apps/api/core";

import type {
  ConnectionListItem,
  ConnectionRecord,
  NewConnection,
  QueryResult,
  TuskErrorPayload,
} from "./types";
import { TuskError } from "./types";

async function invoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await rawInvoke<T>(cmd, args);
  } catch (e) {
    if (e && typeof e === "object" && "kind" in e && "message" in e) {
      throw new TuskError(e as TuskErrorPayload);
    }
    throw e;
  }
}

export async function greet(name: string): Promise<string> {
  return invoke<string>("greet", { name });
}

export async function listConnections(): Promise<ConnectionListItem[]> {
  return invoke<ConnectionListItem[]>("list_connections");
}

export async function addConnection(
  newConnection: NewConnection,
  password: string,
): Promise<ConnectionRecord> {
  return invoke<ConnectionRecord>("add_connection", {
    new: newConnection,
    password,
  });
}

export async function deleteConnection(id: string): Promise<void> {
  return invoke<void>("delete_connection", { id });
}

export async function connect(id: string): Promise<void> {
  return invoke<void>("connect", { id });
}

export async function disconnect(id: string): Promise<void> {
  return invoke<void>("disconnect", { id });
}

export async function executeQuery(
  connectionId: string,
  sql: string,
): Promise<QueryResult> {
  return invoke<QueryResult>("execute_query", { connectionId, sql });
}
```

- [ ] **Step 4: Write `src/store/connections.ts`**

```ts
import { create } from "zustand";

import {
  addConnection as addConnectionCmd,
  connect as connectCmd,
  deleteConnection as deleteConnectionCmd,
  disconnect as disconnectCmd,
  listConnections,
} from "@/lib/tauri";
import type { ConnectionListItem, NewConnection } from "@/lib/types";

interface ConnectionsState {
  items: ConnectionListItem[];
  activeId: string | null;
  loading: boolean;
  refresh: () => Promise<void>;
  add: (newConnection: NewConnection, password: string) => Promise<void>;
  remove: (id: string) => Promise<void>;
  connect: (id: string) => Promise<void>;
  disconnect: (id: string) => Promise<void>;
  setActive: (id: string | null) => void;
}

export const useConnections = create<ConnectionsState>((set, get) => ({
  items: [],
  activeId: null,
  loading: false,

  async refresh() {
    set({ loading: true });
    try {
      const items = await listConnections();
      set({ items });
    } finally {
      set({ loading: false });
    }
  },

  async add(newConnection, password) {
    await addConnectionCmd(newConnection, password);
    await get().refresh();
  },

  async remove(id) {
    await deleteConnectionCmd(id);
    if (get().activeId === id) set({ activeId: null });
    await get().refresh();
  },

  async connect(id) {
    await connectCmd(id);
    set({ activeId: id });
    await get().refresh();
  },

  async disconnect(id) {
    await disconnectCmd(id);
    if (get().activeId === id) set({ activeId: null });
    await get().refresh();
  },

  setActive(id) {
    set({ activeId: id });
  },
}));
```

- [ ] **Step 5: Write `ConnectionForm.tsx`**

```tsx
// src/features/connections/ConnectionForm.tsx
import { useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useConnections } from "@/store/connections";
import type { NewConnection } from "@/lib/types";

const EMPTY: NewConnection = {
  name: "",
  host: "127.0.0.1",
  port: 5432,
  dbUser: "postgres",
  database: "postgres",
  sslMode: "prefer",
  sshKind: "None",
  sshAlias: null,
  sshHost: null,
  sshPort: null,
  sshUser: null,
  sshKeyPath: null,
};

export function ConnectionForm() {
  const add = useConnections((s) => s.add);
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState<NewConnection>(EMPTY);
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);

  function reset() {
    setDraft(EMPTY);
    setPassword("");
  }

  async function onSave() {
    if (!draft.name.trim()) {
      toast.error("Name is required");
      return;
    }
    setBusy(true);
    try {
      await add(draft, password);
      toast.success(`Saved "${draft.name}"`);
      reset();
      setOpen(false);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Failed to save");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (!next) reset();
      }}
    >
      <DialogTrigger asChild>
        <Button size="sm">+ New connection</Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>New connection</DialogTitle>
          <DialogDescription>
            Direct TCP connection to a Postgres server. SSH options arrive in
            Task 7.
          </DialogDescription>
        </DialogHeader>

        <div className="grid grid-cols-2 gap-3">
          <Field label="Name">
            <Input
              value={draft.name}
              onChange={(e) => setDraft({ ...draft, name: e.target.value })}
              placeholder="oci-prod"
            />
          </Field>
          <Field label="Database">
            <Input
              value={draft.database}
              onChange={(e) => setDraft({ ...draft, database: e.target.value })}
            />
          </Field>
          <Field label="Host">
            <Input
              value={draft.host}
              onChange={(e) => setDraft({ ...draft, host: e.target.value })}
            />
          </Field>
          <Field label="Port">
            <Input
              type="number"
              value={draft.port}
              onChange={(e) =>
                setDraft({ ...draft, port: Number(e.target.value) || 0 })
              }
            />
          </Field>
          <Field label="User">
            <Input
              value={draft.dbUser}
              onChange={(e) => setDraft({ ...draft, dbUser: e.target.value })}
            />
          </Field>
          <Field label="Password">
            <Input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </Field>
          <Field label="SSL mode">
            <Select
              value={draft.sslMode}
              onValueChange={(v) => setDraft({ ...draft, sslMode: v })}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {[
                  "disable",
                  "allow",
                  "prefer",
                  "require",
                  "verify-ca",
                  "verify-full",
                ].map((m) => (
                  <SelectItem key={m} value={m}>
                    {m}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>
        </div>

        <DialogFooter>
          <Button
            variant="ghost"
            onClick={() => setOpen(false)}
            disabled={busy}
          >
            Cancel
          </Button>
          <Button onClick={onSave} disabled={busy}>
            {busy ? "Saving..." : "Save"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1">
      <Label className="text-muted-foreground text-xs">{label}</Label>
      {children}
    </div>
  );
}
```

- [ ] **Step 6: Write `ConnectionList.tsx`**

```tsx
// src/features/connections/ConnectionList.tsx
import { useEffect } from "react";
import { Plug, PlugZap, Trash2 } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { useConnections } from "@/store/connections";
import { cn } from "@/lib/utils";

export function ConnectionList() {
  const items = useConnections((s) => s.items);
  const activeId = useConnections((s) => s.activeId);
  const refresh = useConnections((s) => s.refresh);
  const connect = useConnections((s) => s.connect);
  const disconnect = useConnections((s) => s.disconnect);
  const remove = useConnections((s) => s.remove);
  const setActive = useConnections((s) => s.setActive);

  useEffect(() => {
    refresh().catch((e) =>
      toast.error(`Failed to load connections: ${e.message ?? e}`),
    );
  }, [refresh]);

  if (items.length === 0) {
    return (
      <p className="text-muted-foreground px-3 py-2 text-sm">
        No connections yet — click <kbd>+ New connection</kbd> to add one.
      </p>
    );
  }

  return (
    <ul className="flex flex-col gap-1">
      {items.map((c) => {
        const isActive = activeId === c.id;
        return (
          <li
            key={c.id}
            className={cn(
              "group flex items-center justify-between rounded-md border px-3 py-2",
              isActive && "border-primary",
            )}
            onClick={() => setActive(c.id)}
          >
            <div>
              <div className="flex items-center gap-2 text-sm font-medium">
                <span
                  className={cn(
                    "size-2 rounded-full",
                    c.connected ? "bg-emerald-500" : "bg-muted-foreground/40",
                  )}
                />
                {c.name}
              </div>
              <div className="text-muted-foreground text-xs">
                {c.dbUser}@{c.host}:{c.port}/{c.database}
              </div>
            </div>
            <div className="flex gap-1 opacity-0 group-hover:opacity-100">
              {c.connected ? (
                <Button
                  size="icon-sm"
                  variant="ghost"
                  onClick={(e) => {
                    e.stopPropagation();
                    disconnect(c.id).catch((err) => toast.error(err.message));
                  }}
                >
                  <PlugZap />
                </Button>
              ) : (
                <Button
                  size="icon-sm"
                  variant="ghost"
                  onClick={(e) => {
                    e.stopPropagation();
                    connect(c.id)
                      .then(() => toast.success(`Connected to ${c.name}`))
                      .catch((err) => toast.error(err.message));
                  }}
                >
                  <Plug />
                </Button>
              )}
              <Button
                size="icon-sm"
                variant="ghost"
                onClick={(e) => {
                  e.stopPropagation();
                  remove(c.id).catch((err) => toast.error(err.message));
                }}
              >
                <Trash2 />
              </Button>
            </div>
          </li>
        );
      })}
    </ul>
  );
}
```

- [ ] **Step 7: Update `App.tsx` to use a sidebar layout + ad-hoc query box**

Replace `src/App.tsx` with a 2-column layout:

```tsx
import { useState } from "react";
import { Moon, Sun, Play } from "lucide-react";
import { toast } from "sonner";

import { ConnectionForm } from "@/features/connections/ConnectionForm";
import { ConnectionList } from "@/features/connections/ConnectionList";
import { Button } from "@/components/ui/button";
import { useTheme } from "@/hooks/use-theme";
import { useConnections } from "@/store/connections";
import { executeQuery } from "@/lib/tauri";
import type { QueryResult } from "@/lib/types";

function App() {
  const { theme, toggle } = useTheme();
  const activeId = useConnections((s) => s.activeId);
  const [sql, setSql] = useState("SELECT 1");
  const [result, setResult] = useState<QueryResult | null>(null);
  const [busy, setBusy] = useState(false);

  async function run() {
    if (!activeId) {
      toast.error("Select a connected database first");
      return;
    }
    setBusy(true);
    try {
      const r = await executeQuery(activeId, sql);
      setResult(r);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Query failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="bg-background text-foreground grid h-full grid-cols-[280px_1fr]">
      <aside className="border-border flex flex-col gap-3 border-r p-3">
        <div className="flex items-center justify-between">
          <h1 className="text-lg font-semibold">Tusk</h1>
          <Button variant="ghost" size="icon-sm" onClick={toggle}>
            {theme === "light" ? <Moon /> : <Sun />}
          </Button>
        </div>
        <ConnectionForm />
        <ConnectionList />
      </aside>

      <main className="flex flex-col gap-3 p-4">
        <div className="flex items-center gap-2">
          <textarea
            className="border-input bg-background min-h-[120px] flex-1 rounded-md border px-3 py-2 font-mono text-sm"
            value={sql}
            onChange={(e) => setSql(e.target.value)}
            spellCheck={false}
          />
          <Button onClick={run} disabled={busy}>
            <Play />
            Run
          </Button>
        </div>
        {result && (
          <pre className="bg-muted/40 max-h-[60vh] overflow-auto rounded-md p-3 text-xs">
            {JSON.stringify(result, null, 2)}
          </pre>
        )}
      </main>
    </div>
  );
}

export default App;
```

- [ ] **Step 8: Mount `<Toaster />` in `main.tsx`**

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import { Toaster } from "sonner";

import App from "./App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
    <Toaster richColors position="bottom-right" />
  </React.StrictMode>,
);
```

- [ ] **Step 9: Quality gates**

```bash
pnpm typecheck && pnpm lint && pnpm format && pnpm build
```

Expected: all pass. `pnpm format` will reformat `App.tsx` etc. — that's fine.

- [ ] **Step 10: Manual smoke test (with Postgres docker still up)**

```bash
pnpm tauri dev
```

Verify:

- Sidebar shows "No connections yet."
- Click `+ New connection` → fill `name=local`, `host=127.0.0.1`, `port=55432`, `user=tusk`, `password=tusk`, `database=tusk_test`, `sslMode=disable` → Save.
- Hover the row → click the plug icon → green dot appears, "Connected to local" toast.
- `Run` `SELECT 1` → JSON result with `columns: [{name: "?column?", ...}]`, `rows: [[1]]`, `rowCount: 1`.
- Toggle theme — colors switch, dot still visible.

- [ ] **Step 11: Commit**

```bash
git add src package.json pnpm-lock.yaml components.json
git commit -m "feat(frontend): connection form + list + ad-hoc query runner

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: SSH config — alias enumeration + `ssh -G` resolution

**Goal:** Surface `~/.ssh/config` host aliases (with the resolved hostname/user/proxyjump) to the frontend.

**Files:**

- Create: `src-tauri/src/ssh/mod.rs`
- Create: `src-tauri/src/ssh/config.rs`
- Create: `src-tauri/src/commands/ssh.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands/mod.rs`

**Steps:**

- [ ] **Step 1: Scaffold modules**

`src-tauri/src/ssh/mod.rs`:

```rust
pub mod config;
```

Update `src-tauri/src/commands/mod.rs`:

```rust
pub mod connections;
pub mod meta;
pub mod query;
pub mod ssh;
```

Update `src-tauri/src/lib.rs` (add `pub mod ssh;` next to `pub mod db;`):

```rust
pub mod db;
pub mod errors;
pub mod secrets;
pub mod ssh;
mod commands;
```

- [ ] **Step 2: Write `ssh/config.rs`**

```rust
// src-tauri/src/ssh/config.rs
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde::Serialize;

use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SshHost {
    pub alias: String,
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub proxy_jump: Option<String>,
}

pub fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".ssh")
        .join("config")
}

/// Extracts non-wildcard `Host` entries from a config string.
pub fn extract_aliases(config_text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in config_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("host ").or_else(|| lower.strip_prefix("host\t")) {
            let original = trimmed["host".len()..].trim();
            for token in original.split_whitespace() {
                if token.contains(['*', '?', '!']) {
                    continue;
                }
                out.push(token.to_string());
            }
            // We did the work via the original casing; ignore `rest`.
            let _ = rest;
        }
    }
    out
}

/// Calls `ssh -G <alias>` and parses the `key value` lines into an SshHost.
/// Returns None if the binary fails or exits non-zero.
pub fn resolve_via_ssh_g(alias: &str) -> TuskResult<Option<SshHost>> {
    let output = Command::new("ssh")
        .args(["-G", alias])
        .output()
        .map_err(|e| TuskError::Ssh(format!("ssh -G failed to spawn: {e}")))?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut host = SshHost {
        alias: alias.to_string(),
        hostname: None,
        user: None,
        port: None,
        identity_file: None,
        proxy_jump: None,
    };

    for line in stdout.lines() {
        let mut parts = line.splitn(2, ' ');
        let key = parts.next().unwrap_or("").to_ascii_lowercase();
        let value = parts.next().unwrap_or("").trim();
        if value.is_empty() {
            continue;
        }
        match key.as_str() {
            "hostname" => host.hostname = Some(value.to_string()),
            "user" => host.user = Some(value.to_string()),
            "port" => host.port = value.parse().ok(),
            "identityfile" if host.identity_file.is_none() => {
                host.identity_file = Some(value.to_string())
            }
            "proxyjump" if value != "none" => {
                host.proxy_jump = Some(value.to_string())
            }
            _ => {}
        }
    }

    Ok(Some(host))
}

pub fn list_known_hosts() -> TuskResult<Vec<SshHost>> {
    let path = config_path();
    let aliases = match fs::read_to_string(&path) {
        Ok(text) => extract_aliases(&text),
        Err(_) => return Ok(Vec::new()),
    };

    let mut out = Vec::new();
    for alias in aliases {
        if let Ok(Some(host)) = resolve_via_ssh_g(&alias) {
            out.push(host);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_non_wildcard_hosts() {
        let cfg = r#"
# preamble
Host *
    User defaults

Host oci-db oci-util
    HostName 10.0.0.4
    ProxyJump app-cf

Host app-cf
    HostName cf.example.com
"#;
        let aliases = extract_aliases(cfg);
        assert_eq!(aliases, vec!["oci-db", "oci-util", "app-cf"]);
    }
}
```

- [ ] **Step 3: Add `dirs` to Cargo.toml**

Append:

```toml
dirs = "5"
```

- [ ] **Step 4: Write `commands/ssh.rs`**

```rust
// src-tauri/src/commands/ssh.rs
use crate::errors::TuskResult;
use crate::ssh::config::{list_known_hosts, SshHost};

#[tauri::command]
pub fn list_known_ssh_hosts() -> TuskResult<Vec<SshHost>> {
    list_known_hosts()
}
```

- [ ] **Step 5: Register handler in `lib.rs`**

Add to `invoke_handler!`:

```rust
commands::ssh::list_known_ssh_hosts,
```

- [ ] **Step 6: Run tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml ssh::config
```

Expected: 1 test (`extracts_non_wildcard_hosts`) PASS.

- [ ] **Step 7: Quality gates**

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
```

- [ ] **Step 8: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/ssh src-tauri/src/commands src-tauri/src/lib.rs
git commit -m "feat(rust): list ~/.ssh/config aliases via 'ssh -G'

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: SSH tunnel — spawn, readiness, integrate into connect

**Goal:** Open an SSH local-forward via system `ssh`, wait for the local port to become reachable, attach the resulting `Child` to the active connection so that disconnect/drop kills it. Wire `connect` to use it for `SshKind::{Alias, Manual}`.

**Files:**

- Create: `src-tauri/src/ssh/tunnel.rs`
- Modify: `src-tauri/src/ssh/mod.rs`
- Modify: `src-tauri/src/db/pool.rs`
- Modify: `src-tauri/src/commands/connections.rs`

**Steps:**

- [ ] **Step 1: Update `ssh/mod.rs`**

```rust
pub mod config;
pub mod tunnel;
```

- [ ] **Step 2: Replace the `TunnelSlot` placeholder in `db/pool.rs` with the real handle import**

In `db/pool.rs`, swap the `TunnelSlot` definition for an import:

```rust
use crate::ssh::tunnel::TunnelHandle;

pub struct ActiveConnection {
    pub pool: PgPool,
    pub tunnel: Option<TunnelHandle>,
}
```

(Delete the `pub struct TunnelSlot;` line.)

Add a connect helper that takes an already-allocated tunnel:

```rust
impl ConnectionRegistry {
    pub async fn connect_tunneled(
        &self,
        connection_id: &str,
        spec: DirectConnectSpec,
        tunnel: TunnelHandle,
    ) -> TuskResult<()> {
        // Tunnel is already up, so we point at 127.0.0.1:tunnel.local_port.
        let mut tcp_spec = spec;
        tcp_spec.host = "127.0.0.1".into();
        tcp_spec.port = tunnel.local_port;

        let opts = sqlx::postgres::PgConnectOptions::new()
            .host(&tcp_spec.host)
            .port(tcp_spec.port)
            .username(&tcp_spec.user)
            .password(&tcp_spec.password)
            .database(&tcp_spec.database)
            .ssl_mode(parse_ssl_mode(&tcp_spec.ssl_mode)?);

        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .connect_with(opts)
            .await
            .map_err(|e| TuskError::Connection(e.to_string()))?;

        let mut guard = self.inner.lock().expect("registry poisoned");
        guard.insert(
            connection_id.to_string(),
            ActiveConnection { pool, tunnel: Some(tunnel) },
        );
        Ok(())
    }
}
```

(Hoist `parse_ssl_mode` to be reachable from both `connect_direct` and `connect_tunneled` — it's already a module-level fn.)

- [ ] **Step 3: Write `ssh/tunnel.rs`**

```rust
// src-tauri/src/ssh/tunnel.rs
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::time::sleep;

use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Clone)]
pub enum SshTarget {
    Alias(String),
    Manual {
        host: String,
        port: u16,
        user: String,
        key_path: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct TunnelSpec {
    pub target: SshTarget,
    pub remote_host: String, // Postgres host as seen from the bastion
    pub remote_port: u16,
}

#[derive(Debug)]
pub struct TunnelHandle {
    pub child: Child,
    pub local_port: u16,
}

impl Drop for TunnelHandle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn pick_free_port() -> TuskResult<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| TuskError::Tunnel(format!("bind 127.0.0.1:0 failed: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| TuskError::Tunnel(e.to_string()))?
        .port();
    drop(listener);
    Ok(port)
}

pub async fn open_tunnel(spec: TunnelSpec) -> TuskResult<TunnelHandle> {
    let local_port = pick_free_port()?;

    let mut cmd = Command::new("ssh");
    cmd.args([
        "-N",
        "-o", "ServerAliveInterval=30",
        "-o", "ServerAliveCountMax=3",
        "-o", "ExitOnForwardFailure=yes",
        "-o", "BatchMode=no",
        "-L",
        &format!("127.0.0.1:{local_port}:{}:{}", spec.remote_host, spec.remote_port),
    ]);

    match &spec.target {
        SshTarget::Alias(alias) => {
            cmd.arg(alias);
        }
        SshTarget::Manual { host, port, user, key_path } => {
            cmd.args(["-p", &port.to_string()]);
            if let Some(path) = key_path {
                cmd.args(["-i", path]);
            }
            cmd.arg(format!("{user}@{host}"));
        }
    }

    let child = cmd
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| TuskError::Tunnel(format!("ssh spawn failed: {e}")))?;

    let handle = TunnelHandle { child, local_port };

    // Poll until the forwarded port accepts TCP, or we time out.
    let started = Instant::now();
    let timeout = Duration::from_secs(5);
    loop {
        if TcpStream::connect(("127.0.0.1", local_port)).await.is_ok() {
            return Ok(handle);
        }
        if started.elapsed() >= timeout {
            return Err(TuskError::Tunnel(format!(
                "tunnel readiness timed out after {}s",
                timeout.as_secs()
            )));
        }
        sleep(Duration::from_millis(50)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unknown_alias_times_out() {
        let spec = TunnelSpec {
            target: SshTarget::Alias("definitely-not-a-real-host-tusk".into()),
            remote_host: "127.0.0.1".into(),
            remote_port: 5432,
        };
        let result = open_tunnel(spec).await;
        assert!(result.is_err(), "expected tunnel error, got {result:?}");
    }
}
```

- [ ] **Step 4: Update `commands/connections.rs` SSH branch**

Replace the `Err(TuskError::Tunnel("SSH-backed connect not yet wired"))` with real wiring:

```rust
use crate::ssh::tunnel::{open_tunnel, SshTarget, TunnelSpec};

// ... inside `connect`:
SshKind::Alias => {
    let alias = record
        .ssh_alias
        .clone()
        .ok_or_else(|| TuskError::Tunnel("ssh_alias missing".into()))?;
    let tunnel = open_tunnel(TunnelSpec {
        target: SshTarget::Alias(alias),
        remote_host: record.host.clone(),
        remote_port: record.port,
    })
    .await?;
    let spec = DirectConnectSpec {
        host: record.host,
        port: record.port,
        user: record.db_user,
        password,
        database: record.database,
        ssl_mode: record.ssl_mode,
    };
    registry.connect_tunneled(&id, spec, tunnel).await?;
    Ok(())
}
SshKind::Manual => {
    let host = record.ssh_host.clone()
        .ok_or_else(|| TuskError::Tunnel("ssh_host missing".into()))?;
    let port = record.ssh_port.unwrap_or(22);
    let user = record.ssh_user.clone()
        .ok_or_else(|| TuskError::Tunnel("ssh_user missing".into()))?;
    let key_path = record.ssh_key_path.clone();
    let tunnel = open_tunnel(TunnelSpec {
        target: SshTarget::Manual { host, port, user, key_path },
        remote_host: record.host.clone(),
        remote_port: record.port,
    })
    .await?;
    let spec = DirectConnectSpec {
        host: record.host,
        port: record.port,
        user: record.db_user,
        password,
        database: record.database,
        ssl_mode: record.ssl_mode,
    };
    registry.connect_tunneled(&id, spec, tunnel).await?;
    Ok(())
}
```

- [ ] **Step 5: Run tunnel test**

```bash
cargo test --manifest-path src-tauri/Cargo.toml ssh::tunnel
```

Expected: PASS in ≤6s. (Test waits for the 5s readiness timeout.)

- [ ] **Step 6: Quality gates**

```bash
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
cargo test --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/ssh src-tauri/src/db/pool.rs src-tauri/src/commands/connections.rs
git commit -m "feat(rust): SSH tunnel via system ssh + integrate into connect

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Frontend SSH UI — alias picker + manual fields

**Goal:** Three-tab connection form (`Direct TCP` | `SSH alias` | `SSH manual`) with the ssh-config alias dropdown.

**Files:**

- Modify: `src/lib/types.ts` (add `SshHost`, `listKnownSshHosts` types)
- Modify: `src/lib/tauri.ts`
- Create: `src/features/connections/SshHostPicker.tsx`
- Modify: `src/features/connections/ConnectionForm.tsx`
- shadcn: `tabs`

**Steps:**

- [ ] **Step 1: Add `tabs` primitive**

```bash
pnpm dlx shadcn@latest add tabs --yes
```

- [ ] **Step 2: Append `SshHost` type**

In `src/lib/types.ts`:

```ts
export interface SshHost {
  alias: string;
  hostname: string | null;
  user: string | null;
  port: number | null;
  identityFile: string | null;
  proxyJump: string | null;
}
```

- [ ] **Step 3: Add tauri wrapper**

In `src/lib/tauri.ts`:

```ts
import type { SshHost } from "./types";

export async function listKnownSshHosts(): Promise<SshHost[]> {
  return invoke<SshHost[]>("list_known_ssh_hosts");
}
```

(Add to the `import type` block: `, SshHost`.)

- [ ] **Step 4: Write `SshHostPicker.tsx`**

```tsx
// src/features/connections/SshHostPicker.tsx
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { listKnownSshHosts } from "@/lib/tauri";
import type { SshHost } from "@/lib/types";
import { cn } from "@/lib/utils";

interface Props {
  selected: SshHost | null;
  onSelect: (host: SshHost) => void;
}

export function SshHostPicker({ selected, onSelect }: Props) {
  const [hosts, setHosts] = useState<SshHost[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    listKnownSshHosts()
      .then(setHosts)
      .catch((e) => toast.error(`SSH config: ${e.message ?? e}`))
      .finally(() => setLoading(false));
  }, []);

  if (loading) return <p className="text-muted-foreground text-xs">Loading…</p>;

  if (hosts.length === 0) {
    return (
      <p className="text-muted-foreground text-xs">
        No usable hosts in <code>~/.ssh/config</code>.
      </p>
    );
  }

  return (
    <ul className="border-input max-h-48 overflow-auto rounded-md border">
      {hosts.map((h) => {
        const isActive = selected?.alias === h.alias;
        return (
          <li
            key={h.alias}
            className={cn(
              "hover:bg-accent cursor-pointer border-b px-3 py-2 text-sm last:border-b-0",
              isActive && "bg-accent",
            )}
            onClick={() => onSelect(h)}
          >
            <div className="font-medium">{h.alias}</div>
            <div className="text-muted-foreground text-xs">
              {h.user ?? "?"}@{h.hostname ?? "?"}
              {h.proxyJump ? ` · via ${h.proxyJump}` : ""}
            </div>
          </li>
        );
      })}
    </ul>
  );
}
```

- [ ] **Step 5: Refactor `ConnectionForm.tsx` to use Tabs**

Replace the JSX body of `<DialogContent>` to include three tabs. Keep the existing Direct TCP fields under the first tab; add the other two tabs:

```tsx
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

import { SshHostPicker } from "./SshHostPicker";
```

Replace the `<div className="grid grid-cols-2 gap-3">…</div>` with:

```tsx
<Tabs
  value={
    draft.sshKind === "None"
      ? "tcp"
      : draft.sshKind === "Alias"
        ? "alias"
        : "manual"
  }
  onValueChange={(v) => {
    if (v === "tcp") setDraft({ ...draft, sshKind: "None" });
    if (v === "alias") setDraft({ ...draft, sshKind: "Alias" });
    if (v === "manual") setDraft({ ...draft, sshKind: "Manual" });
  }}
>
  <TabsList>
    <TabsTrigger value="tcp">Direct TCP</TabsTrigger>
    <TabsTrigger value="alias">SSH alias</TabsTrigger>
    <TabsTrigger value="manual">SSH manual</TabsTrigger>
  </TabsList>

  <TabsContent value="tcp" className="grid grid-cols-2 gap-3 pt-3">
    {commonFields()}
  </TabsContent>

  <TabsContent value="alias" className="flex flex-col gap-3 pt-3">
    <SshHostPicker
      selected={
        draft.sshAlias
          ? {
              alias: draft.sshAlias,
              hostname: null,
              user: null,
              port: null,
              identityFile: null,
              proxyJump: null,
            }
          : null
      }
      onSelect={(host) =>
        setDraft({
          ...draft,
          sshAlias: host.alias,
          sshHost: host.hostname,
          sshUser: host.user,
          sshPort: host.port,
        })
      }
    />
    <div className="text-muted-foreground text-xs">
      Selected: <strong>{draft.sshAlias ?? "—"}</strong>
    </div>
    <div className="grid grid-cols-2 gap-3">{commonFields()}</div>
  </TabsContent>

  <TabsContent value="manual" className="flex flex-col gap-3 pt-3">
    <div className="grid grid-cols-2 gap-3">
      <Field label="SSH host">
        <Input
          value={draft.sshHost ?? ""}
          onChange={(e) =>
            setDraft({ ...draft, sshHost: e.target.value || null })
          }
        />
      </Field>
      <Field label="SSH port">
        <Input
          type="number"
          value={draft.sshPort ?? 22}
          onChange={(e) =>
            setDraft({ ...draft, sshPort: Number(e.target.value) || 22 })
          }
        />
      </Field>
      <Field label="SSH user">
        <Input
          value={draft.sshUser ?? ""}
          onChange={(e) =>
            setDraft({ ...draft, sshUser: e.target.value || null })
          }
        />
      </Field>
      <Field label="SSH key path">
        <Input
          value={draft.sshKeyPath ?? ""}
          placeholder="~/.ssh/id_ed25519"
          onChange={(e) =>
            setDraft({ ...draft, sshKeyPath: e.target.value || null })
          }
        />
      </Field>
    </div>
    <div className="grid grid-cols-2 gap-3">{commonFields()}</div>
  </TabsContent>
</Tabs>
```

Extract the existing 8 fields into a `commonFields()` helper inside the component:

```tsx
function commonFields() {
  return (
    <>
      <Field label="Name">{/* …Name input… */}</Field>
      <Field label="Database">{/* …Database input… */}</Field>
      <Field label="Postgres host">{/* draft.host */}</Field>
      <Field label="Postgres port">{/* draft.port */}</Field>
      <Field label="User">{/* draft.dbUser */}</Field>
      <Field label="Password">{/* password */}</Field>
      <Field label="SSL mode">{/* … */}</Field>
    </>
  );
}
```

(Move the existing JSX from Step 5 of Task 4 into `commonFields()` verbatim.)

- [ ] **Step 6: Quality gates**

```bash
pnpm typecheck && pnpm lint && pnpm format && pnpm build
```

- [ ] **Step 7: Manual smoke**

```bash
pnpm tauri dev
```

Verify:

- "+ New connection" → tabs visible.
- `SSH alias` tab lists `~/.ssh/config` aliases.
- Click an alias → fields under it auto-fill (host/user/port shown read-only inside the picker description).
- `SSH manual` tab accepts manual SSH entries.

- [ ] **Step 8: Commit**

```bash
git add src components.json package.json pnpm-lock.yaml
git commit -m "feat(frontend): SSH alias + manual tabs in connection form

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Schema tree

**Goal:** Lazy-loaded schema sidebar (DB → schema → table → column).

**Files:**

- Create: `src-tauri/src/commands/schema.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src/features/schema/SchemaTree.tsx`
- Create: `src/features/schema/SchemaNode.tsx`
- Create: `src/store/schema.ts`
- Modify: `src/lib/tauri.ts`, `src/lib/types.ts`
- Modify: `src/App.tsx`
- shadcn: `scroll-area`, `collapsible`

**Steps:**

- [ ] **Step 1: Add Rust commands**

`src-tauri/src/commands/schema.rs`:

```rust
use serde::Serialize;
use sqlx::Row;
use tauri::State;

use crate::db::pool::ConnectionRegistry;
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Serialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
}

async fn fetch_strings(
    registry: State<'_, ConnectionRegistry>,
    connection_id: &str,
    sql: &str,
) -> TuskResult<Vec<String>> {
    let pool = registry.pool(connection_id)?;
    let rows = sqlx::query(sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| TuskError::Query(e.to_string()))?;
    rows.iter()
        .map(|r| r.try_get::<String, _>(0).map_err(|e| TuskError::Query(e.to_string())))
        .collect()
}

#[tauri::command]
pub async fn list_databases(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
) -> TuskResult<Vec<String>> {
    fetch_strings(
        registry,
        &connection_id,
        "SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname",
    )
    .await
}

#[tauri::command]
pub async fn list_schemas(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
) -> TuskResult<Vec<String>> {
    fetch_strings(
        registry,
        &connection_id,
        "SELECT schema_name FROM information_schema.schemata
         WHERE schema_name NOT IN ('pg_catalog','information_schema','pg_toast')
         ORDER BY schema_name",
    )
    .await
}

#[tauri::command]
pub async fn list_tables(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
    schema: String,
) -> TuskResult<Vec<String>> {
    let pool = registry.pool(&connection_id)?;
    let rows = sqlx::query(
        "SELECT table_name FROM information_schema.tables
         WHERE table_schema = $1 ORDER BY table_name",
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .map_err(|e| TuskError::Query(e.to_string()))?;
    rows.iter()
        .map(|r| r.try_get::<String, _>(0).map_err(|e| TuskError::Query(e.to_string())))
        .collect()
}

#[tauri::command]
pub async fn list_columns(
    registry: State<'_, ConnectionRegistry>,
    connection_id: String,
    schema: String,
    table: String,
) -> TuskResult<Vec<ColumnInfo>> {
    let pool = registry.pool(&connection_id)?;
    let rows = sqlx::query(
        "SELECT column_name, data_type, is_nullable
         FROM information_schema.columns
         WHERE table_schema = $1 AND table_name = $2
         ORDER BY ordinal_position",
    )
    .bind(&schema)
    .bind(&table)
    .fetch_all(&pool)
    .await
    .map_err(|e| TuskError::Query(e.to_string()))?;

    rows.iter()
        .map(|r| {
            let name: String = r.try_get(0).map_err(|e| TuskError::Query(e.to_string()))?;
            let data_type: String = r.try_get(1).map_err(|e| TuskError::Query(e.to_string()))?;
            let nullable: String = r.try_get(2).map_err(|e| TuskError::Query(e.to_string()))?;
            Ok(ColumnInfo {
                name,
                data_type,
                is_nullable: nullable.eq_ignore_ascii_case("yes"),
            })
        })
        .collect()
}
```

Register in `commands/mod.rs`:

```rust
pub mod schema;
```

And in `lib.rs` `invoke_handler!`:

```rust
commands::schema::list_databases,
commands::schema::list_schemas,
commands::schema::list_tables,
commands::schema::list_columns,
```

- [ ] **Step 2: Frontend types + tauri wrappers**

`src/lib/types.ts`:

```ts
export interface ColumnInfo {
  name: string;
  data_type: string;
  is_nullable: boolean;
}
```

`src/lib/tauri.ts`:

```ts
export async function listDatabases(connectionId: string) {
  return invoke<string[]>("list_databases", { connectionId });
}
export async function listSchemas(connectionId: string) {
  return invoke<string[]>("list_schemas", { connectionId });
}
export async function listTables(connectionId: string, schema: string) {
  return invoke<string[]>("list_tables", { connectionId, schema });
}
export async function listColumns(
  connectionId: string,
  schema: string,
  table: string,
) {
  return invoke<ColumnInfo[]>("list_columns", { connectionId, schema, table });
}
```

(Import `ColumnInfo` in the type imports.)

- [ ] **Step 3: shadcn primitives**

```bash
pnpm dlx shadcn@latest add scroll-area collapsible --yes
```

- [ ] **Step 4: Schema store**

`src/store/schema.ts`:

```ts
import { create } from "zustand";

import {
  listColumns,
  listDatabases,
  listSchemas,
  listTables,
} from "@/lib/tauri";
import type { ColumnInfo } from "@/lib/types";

interface CacheEntry<T> {
  state: "idle" | "loading" | "ready" | "error";
  data?: T;
  error?: string;
}

interface SchemaState {
  databases: Record<string, CacheEntry<string[]>>;
  schemas: Record<string, CacheEntry<string[]>>;
  tables: Record<string, CacheEntry<string[]>>; // key = `${connId}:${schema}`
  columns: Record<string, CacheEntry<ColumnInfo[]>>; // key = `${connId}:${schema}:${table}`

  loadDatabases: (connId: string) => Promise<void>;
  loadSchemas: (connId: string) => Promise<void>;
  loadTables: (connId: string, schema: string) => Promise<void>;
  loadColumns: (connId: string, schema: string, table: string) => Promise<void>;
  clear: (connId: string) => void;
}

export const useSchema = create<SchemaState>((set, get) => ({
  databases: {},
  schemas: {},
  tables: {},
  columns: {},

  async loadDatabases(connId) {
    if (get().databases[connId]?.state === "ready") return;
    set((s) => ({
      databases: { ...s.databases, [connId]: { state: "loading" } },
    }));
    try {
      const data = await listDatabases(connId);
      set((s) => ({
        databases: { ...s.databases, [connId]: { state: "ready", data } },
      }));
    } catch (e) {
      set((s) => ({
        databases: {
          ...s.databases,
          [connId]: { state: "error", error: (e as Error).message },
        },
      }));
    }
  },

  async loadSchemas(connId) {
    if (get().schemas[connId]?.state === "ready") return;
    set((s) => ({ schemas: { ...s.schemas, [connId]: { state: "loading" } } }));
    try {
      const data = await listSchemas(connId);
      set((s) => ({
        schemas: { ...s.schemas, [connId]: { state: "ready", data } },
      }));
    } catch (e) {
      set((s) => ({
        schemas: {
          ...s.schemas,
          [connId]: { state: "error", error: (e as Error).message },
        },
      }));
    }
  },

  async loadTables(connId, schema) {
    const key = `${connId}:${schema}`;
    if (get().tables[key]?.state === "ready") return;
    set((s) => ({ tables: { ...s.tables, [key]: { state: "loading" } } }));
    try {
      const data = await listTables(connId, schema);
      set((s) => ({
        tables: { ...s.tables, [key]: { state: "ready", data } },
      }));
    } catch (e) {
      set((s) => ({
        tables: {
          ...s.tables,
          [key]: { state: "error", error: (e as Error).message },
        },
      }));
    }
  },

  async loadColumns(connId, schema, table) {
    const key = `${connId}:${schema}:${table}`;
    if (get().columns[key]?.state === "ready") return;
    set((s) => ({ columns: { ...s.columns, [key]: { state: "loading" } } }));
    try {
      const data = await listColumns(connId, schema, table);
      set((s) => ({
        columns: { ...s.columns, [key]: { state: "ready", data } },
      }));
    } catch (e) {
      set((s) => ({
        columns: {
          ...s.columns,
          [key]: { state: "error", error: (e as Error).message },
        },
      }));
    }
  },

  clear(connId) {
    set((s) => {
      const databases = { ...s.databases };
      const schemas = { ...s.schemas };
      const tables = { ...s.tables };
      const columns = { ...s.columns };
      delete databases[connId];
      delete schemas[connId];
      Object.keys(tables).forEach(
        (k) => k.startsWith(`${connId}:`) && delete tables[k],
      );
      Object.keys(columns).forEach(
        (k) => k.startsWith(`${connId}:`) && delete columns[k],
      );
      return { databases, schemas, tables, columns };
    });
  },
}));
```

- [ ] **Step 5: `SchemaNode.tsx`**

```tsx
// src/features/schema/SchemaNode.tsx
import { useEffect, useState } from "react";
import { ChevronRight, ChevronDown } from "lucide-react";

import { cn } from "@/lib/utils";

interface Props {
  label: string;
  children: React.ReactNode;
  onExpand?: () => void;
  initiallyOpen?: boolean;
  indent?: number;
}

export function SchemaNode({
  label,
  children,
  onExpand,
  initiallyOpen,
  indent = 0,
}: Props) {
  const [open, setOpen] = useState(!!initiallyOpen);

  useEffect(() => {
    if (open) onExpand?.();
  }, [open, onExpand]);

  return (
    <div>
      <button
        type="button"
        className={cn(
          "hover:bg-accent flex w-full items-center gap-1 rounded px-1 py-0.5 text-left text-sm",
        )}
        style={{ paddingLeft: 4 + indent * 12 }}
        onClick={() => setOpen((o) => !o)}
      >
        {open ? (
          <ChevronDown className="size-3.5" />
        ) : (
          <ChevronRight className="size-3.5" />
        )}
        <span>{label}</span>
      </button>
      {open && <div>{children}</div>}
    </div>
  );
}
```

- [ ] **Step 6: `SchemaTree.tsx`**

```tsx
// src/features/schema/SchemaTree.tsx
import { useCallback } from "react";

import { ScrollArea } from "@/components/ui/scroll-area";
import { useConnections } from "@/store/connections";
import { useSchema } from "@/store/schema";

import { SchemaNode } from "./SchemaNode";

export function SchemaTree() {
  const items = useConnections((s) => s.items);
  const connected = items.filter((i) => i.connected);

  if (connected.length === 0) {
    return (
      <p className="text-muted-foreground px-3 py-2 text-xs">
        Connect to a database to browse its schema.
      </p>
    );
  }

  return (
    <ScrollArea className="flex-1">
      <div className="flex flex-col gap-1 p-2">
        {connected.map((c) => (
          <ConnectionBranch key={c.id} connectionId={c.id} name={c.name} />
        ))}
      </div>
    </ScrollArea>
  );
}

function ConnectionBranch({
  connectionId,
  name,
}: {
  connectionId: string;
  name: string;
}) {
  const schemas = useSchema((s) => s.schemas[connectionId]);
  const load = useSchema((s) => s.loadSchemas);

  const onExpand = useCallback(() => {
    load(connectionId);
  }, [load, connectionId]);

  return (
    <SchemaNode label={name} onExpand={onExpand}>
      {schemas?.state === "loading" && <Hint>loading…</Hint>}
      {schemas?.state === "error" && <Hint>{schemas.error}</Hint>}
      {schemas?.state === "ready" &&
        schemas.data!.map((schema) => (
          <SchemaBranch
            key={schema}
            connectionId={connectionId}
            schema={schema}
            indent={1}
          />
        ))}
    </SchemaNode>
  );
}

function SchemaBranch({
  connectionId,
  schema,
  indent,
}: {
  connectionId: string;
  schema: string;
  indent: number;
}) {
  const key = `${connectionId}:${schema}`;
  const tables = useSchema((s) => s.tables[key]);
  const loadTables = useSchema((s) => s.loadTables);

  const onExpand = useCallback(() => {
    loadTables(connectionId, schema);
  }, [loadTables, connectionId, schema]);

  return (
    <SchemaNode label={schema} indent={indent} onExpand={onExpand}>
      {tables?.state === "loading" && <Hint indent={indent + 1}>loading…</Hint>}
      {tables?.state === "error" && (
        <Hint indent={indent + 1}>{tables.error}</Hint>
      )}
      {tables?.state === "ready" &&
        tables.data!.map((table) => (
          <TableBranch
            key={table}
            connectionId={connectionId}
            schema={schema}
            table={table}
            indent={indent + 1}
          />
        ))}
    </SchemaNode>
  );
}

function TableBranch({
  connectionId,
  schema,
  table,
  indent,
}: {
  connectionId: string;
  schema: string;
  table: string;
  indent: number;
}) {
  const key = `${connectionId}:${schema}:${table}`;
  const cols = useSchema((s) => s.columns[key]);
  const loadColumns = useSchema((s) => s.loadColumns);

  const onExpand = useCallback(() => {
    loadColumns(connectionId, schema, table);
  }, [loadColumns, connectionId, schema, table]);

  return (
    <SchemaNode label={table} indent={indent} onExpand={onExpand}>
      {cols?.state === "loading" && <Hint indent={indent + 1}>loading…</Hint>}
      {cols?.state === "error" && <Hint indent={indent + 1}>{cols.error}</Hint>}
      {cols?.state === "ready" &&
        cols.data!.map((c) => (
          <div
            key={c.name}
            className="text-muted-foreground flex justify-between text-xs"
            style={{ paddingLeft: 4 + (indent + 1) * 12, paddingRight: 8 }}
          >
            <span>{c.name}</span>
            <span>
              {c.data_type}
              {!c.is_nullable && " ·"}
              {!c.is_nullable && (
                <span className="text-foreground"> NOT NULL</span>
              )}
            </span>
          </div>
        ))}
    </SchemaNode>
  );
}

function Hint({
  children,
  indent = 0,
}: {
  children: React.ReactNode;
  indent?: number;
}) {
  return (
    <p
      className="text-muted-foreground text-xs italic"
      style={{ paddingLeft: 4 + indent * 12 }}
    >
      {children}
    </p>
  );
}
```

- [ ] **Step 7: Mount in `App.tsx` (3-column layout)**

Update sidebar to two stacked sections (connections + schema):

```tsx
import { SchemaTree } from "@/features/schema/SchemaTree";

// inside <aside>...
<aside className="border-border flex flex-col border-r">
  <div className="flex items-center justify-between p-3">
    <h1 className="text-lg font-semibold">Tusk</h1>
    <Button variant="ghost" size="icon-sm" onClick={toggle}>
      {theme === "light" ? <Moon /> : <Sun />}
    </Button>
  </div>
  <div className="border-border flex flex-col gap-2 border-b p-3">
    <ConnectionForm />
    <ConnectionList />
  </div>
  <SchemaTree />
</aside>;
```

- [ ] **Step 8: Quality gates + manual smoke**

```bash
pnpm typecheck && pnpm lint && pnpm format && pnpm build
pnpm tauri dev   # connect, expand the tree, see schemas/tables/columns
```

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/commands src-tauri/src/lib.rs src components.json package.json pnpm-lock.yaml
git commit -m "feat: lazy-load schema tree (db/schema/table/column)

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: Monaco editor + tab store

**Goal:** Multi-tab SQL editor wired to the active connection, with `Cmd+Enter` to run, `Cmd+T` new tab, `Cmd+W` close tab.

**Files:**

- Create: `src/store/tabs.ts`
- Create: `src/features/editor/EditorPane.tsx`
- Create: `src/features/editor/EditorTabs.tsx`
- Create: `src/features/editor/keymap.ts`
- Modify: `src/App.tsx`

**Steps:**

- [ ] **Step 1: Tab store**

```ts
// src/store/tabs.ts
import { create } from "zustand";

import type { QueryResult } from "@/lib/types";

export interface Tab {
  id: string;
  title: string;
  connectionId: string | null;
  sql: string;
  dirty: boolean;
  lastResult?: QueryResult;
  lastError?: string;
  busy?: boolean;
}

interface TabsState {
  tabs: Tab[];
  activeId: string;
  newTab: (connectionId: string | null) => string;
  closeTab: (id: string) => void;
  updateSql: (id: string, sql: string) => void;
  setActive: (id: string) => void;
  bindConnection: (id: string, connectionId: string | null) => void;
  setResult: (id: string, result: QueryResult) => void;
  setError: (id: string, message: string) => void;
  setBusy: (id: string, busy: boolean) => void;
}

let counter = 1;

const initialId = crypto.randomUUID();
const initialTab: Tab = {
  id: initialId,
  title: `Untitled ${counter++}`,
  connectionId: null,
  sql: "SELECT 1",
  dirty: false,
};

export const useTabs = create<TabsState>((set) => ({
  tabs: [initialTab],
  activeId: initialId,

  newTab(connectionId) {
    const id = crypto.randomUUID();
    set((s) => ({
      tabs: [
        ...s.tabs,
        {
          id,
          title: `Untitled ${counter++}`,
          connectionId,
          sql: "",
          dirty: false,
        },
      ],
      activeId: id,
    }));
    return id;
  },

  closeTab(id) {
    set((s) => {
      const tabs = s.tabs.filter((t) => t.id !== id);
      if (tabs.length === 0) {
        const fresh: Tab = {
          id: crypto.randomUUID(),
          title: `Untitled ${counter++}`,
          connectionId: null,
          sql: "",
          dirty: false,
        };
        return { tabs: [fresh], activeId: fresh.id };
      }
      const activeId =
        s.activeId === id ? tabs[tabs.length - 1].id : s.activeId;
      return { tabs, activeId };
    });
  },

  updateSql(id, sql) {
    set((s) => ({
      tabs: s.tabs.map((t) => (t.id === id ? { ...t, sql, dirty: true } : t)),
    }));
  },

  setActive(id) {
    set({ activeId: id });
  },

  bindConnection(id, connectionId) {
    set((s) => ({
      tabs: s.tabs.map((t) => (t.id === id ? { ...t, connectionId } : t)),
    }));
  },

  setResult(id, result) {
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === id
          ? { ...t, lastResult: result, lastError: undefined, busy: false }
          : t,
      ),
    }));
  },

  setError(id, message) {
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === id
          ? { ...t, lastError: message, lastResult: undefined, busy: false }
          : t,
      ),
    }));
  },

  setBusy(id, busy) {
    set((s) => ({
      tabs: s.tabs.map((t) => (t.id === id ? { ...t, busy } : t)),
    }));
  },
}));
```

- [ ] **Step 2: Keymap helper**

```ts
// src/features/editor/keymap.ts
export type Modifier = "meta" | "ctrl";

export function platformModifier(): Modifier {
  if (typeof navigator !== "undefined" && /Mac/i.test(navigator.platform))
    return "meta";
  return "ctrl";
}

export function isModifier(
  e: KeyboardEvent | React.KeyboardEvent,
  mod: Modifier,
) {
  return mod === "meta" ? e.metaKey : e.ctrlKey;
}
```

- [ ] **Step 3: `EditorPane.tsx`**

```tsx
// src/features/editor/EditorPane.tsx
import { useCallback, useEffect, useRef } from "react";
import Editor, { type OnMount } from "@monaco-editor/react";
import { Play } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { executeQuery } from "@/lib/tauri";
import { useTheme } from "@/hooks/use-theme";
import { useConnections } from "@/store/connections";
import { useTabs } from "@/store/tabs";

import { EditorTabs } from "./EditorTabs";
import { isModifier, platformModifier } from "./keymap";

export function EditorPane() {
  const { theme } = useTheme();
  const tabs = useTabs((s) => s.tabs);
  const activeId = useTabs((s) => s.activeId);
  const updateSql = useTabs((s) => s.updateSql);
  const newTab = useTabs((s) => s.newTab);
  const closeTab = useTabs((s) => s.closeTab);
  const setBusy = useTabs((s) => s.setBusy);
  const setResult = useTabs((s) => s.setResult);
  const setError = useTabs((s) => s.setError);
  const activeConnection = useConnections((s) => s.activeId);
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);

  const activeTab = tabs.find((t) => t.id === activeId)!;
  const connectionForTab = activeTab.connectionId ?? activeConnection;

  const run = useCallback(async () => {
    if (!connectionForTab) {
      toast.error("Select a connected database first");
      return;
    }
    setBusy(activeTab.id, true);
    try {
      const result = await executeQuery(connectionForTab, activeTab.sql);
      setResult(activeTab.id, result);
    } catch (e) {
      const msg = e instanceof Error ? e.message : "Query failed";
      setError(activeTab.id, msg);
      toast.error(msg);
    }
  }, [
    activeTab.id,
    activeTab.sql,
    connectionForTab,
    setBusy,
    setError,
    setResult,
  ]);

  useEffect(() => {
    const mod = platformModifier();
    function onKey(e: KeyboardEvent) {
      if (!isModifier(e, mod)) return;
      if (e.key === "Enter") {
        e.preventDefault();
        run();
      } else if (e.key.toLowerCase() === "t") {
        e.preventDefault();
        newTab(connectionForTab);
      } else if (e.key.toLowerCase() === "w") {
        e.preventDefault();
        closeTab(activeTab.id);
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [activeTab.id, closeTab, connectionForTab, newTab, run]);

  return (
    <div className="flex flex-1 flex-col">
      <EditorTabs />
      <div className="flex flex-1 flex-col">
        <div className="border-border flex items-center justify-between border-b px-3 py-1.5">
          <span className="text-muted-foreground text-xs">
            {connectionForTab
              ? `Running on: ${connectionForTab}`
              : "No connection"}
          </span>
          <Button size="sm" onClick={run} disabled={activeTab.busy}>
            <Play /> Run ({platformModifier() === "meta" ? "⌘" : "Ctrl"}+Enter)
          </Button>
        </div>
        <div className="min-h-0 flex-1">
          <Editor
            height="100%"
            language="sql"
            theme={theme === "dark" ? "vs-dark" : "vs"}
            value={activeTab.sql}
            onChange={(v) => updateSql(activeTab.id, v ?? "")}
            onMount={(ed) => {
              editorRef.current = ed;
            }}
            options={{
              minimap: { enabled: false },
              fontSize: 13,
              fontFamily:
                "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
              scrollBeyondLastLine: false,
              automaticLayout: true,
              wordWrap: "on",
            }}
          />
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: `EditorTabs.tsx`**

```tsx
// src/features/editor/EditorTabs.tsx
import { Plus, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useTabs } from "@/store/tabs";
import { cn } from "@/lib/utils";

export function EditorTabs() {
  const tabs = useTabs((s) => s.tabs);
  const activeId = useTabs((s) => s.activeId);
  const setActive = useTabs((s) => s.setActive);
  const newTab = useTabs((s) => s.newTab);
  const closeTab = useTabs((s) => s.closeTab);

  return (
    <div className="border-border bg-muted/30 flex items-center gap-1 border-b px-2 py-1">
      {tabs.map((t) => (
        <button
          key={t.id}
          onClick={() => setActive(t.id)}
          className={cn(
            "group flex items-center gap-1 rounded px-2 py-1 text-xs",
            t.id === activeId ? "bg-background border" : "hover:bg-accent",
          )}
        >
          <span>
            {t.title}
            {t.dirty && "•"}
          </span>
          <span
            role="button"
            className="rounded p-0.5 opacity-50 hover:opacity-100"
            onClick={(e) => {
              e.stopPropagation();
              closeTab(t.id);
            }}
          >
            <X className="size-3" />
          </span>
        </button>
      ))}
      <Button size="icon-xs" variant="ghost" onClick={() => newTab(null)}>
        <Plus />
      </Button>
    </div>
  );
}
```

- [ ] **Step 5: Replace ad-hoc query box in `App.tsx`**

Remove the `<textarea>` + `<pre>` block; replace `<main>` body:

```tsx
import { EditorPane } from "@/features/editor/EditorPane";

// inside the right column:
<main className="flex min-h-0 flex-col">
  <EditorPane />
</main>;
```

(Make sure the parent is `grid grid-cols-[280px_1fr]` and that `main` is `min-h-0 flex flex-col` so Monaco fills.)

- [ ] **Step 6: Quality gates + smoke**

```bash
pnpm typecheck && pnpm lint && pnpm format && pnpm build
pnpm tauri dev
```

Verify:

- Tabs render. New tab button works.
- Cmd+Enter executes query (the result appears as raw JSON until Task 10 — adapt: just check the network of `last_result` via React DevTools or wait for Task 10).
- Cmd+T opens a new tab, Cmd+W closes it.

- [ ] **Step 7: Commit**

```bash
git add src
git commit -m "feat(frontend): Monaco SQL editor with multi-tab store

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 10: Result grid (TanStack + virtualized)

**Goal:** Render `lastResult` of the active tab in a virtualized grid with sortable columns and proper cell formatting; hook the editor "Run" path so users see structured output.

**Files:**

- Create: `src/lib/sql.ts`
- Create: `src/store/results.ts` _(unused — results live on the tab; skip if redundant)_
- Create: `src/features/results/cells.tsx`
- Create: `src/features/results/ResultsHeader.tsx`
- Create: `src/features/results/ResultsGrid.tsx`
- Modify: `src/features/editor/EditorPane.tsx` (apply auto-LIMIT, render results pane)

**Steps:**

- [ ] **Step 1: `src/lib/sql.ts` — auto-LIMIT helper**

```ts
const LIMIT_RE = /\blimit\s+\d+\b/i;
const SELECT_RE = /^\s*select\b/i;

export function withAutoLimit(sql: string, limit = 1000): string {
  if (!SELECT_RE.test(sql)) return sql;
  if (LIMIT_RE.test(sql)) return sql;
  // strip trailing semicolon for the merge
  const trimmed = sql.replace(/;\s*$/, "");
  return `${trimmed} LIMIT ${limit}`;
}
```

- [ ] **Step 2: `cells.tsx`**

```tsx
// src/features/results/cells.tsx
import { useState } from "react";

export function Cell({ value, type }: { value: unknown; type: string }) {
  if (value === null || value === undefined) {
    return <span className="text-muted-foreground italic">NULL</span>;
  }
  if (type === "json" || type === "jsonb" || typeof value === "object") {
    return <JsonCell value={value} />;
  }
  if (typeof value === "boolean") {
    return <span>{value ? "true" : "false"}</span>;
  }
  return <span className="font-mono">{String(value)}</span>;
}

function JsonCell({ value }: { value: unknown }) {
  const [open, setOpen] = useState(false);
  const text = JSON.stringify(value);
  const truncated = text.length > 80 ? `${text.slice(0, 77)}…` : text;
  return (
    <button
      onClick={() => setOpen((o) => !o)}
      className="text-left font-mono"
      title={open ? "click to collapse" : "click to expand"}
    >
      {open ? (
        <pre className="text-xs whitespace-pre-wrap">
          {JSON.stringify(value, null, 2)}
        </pre>
      ) : (
        truncated
      )}
    </button>
  );
}
```

- [ ] **Step 3: `ResultsHeader.tsx`**

```tsx
// src/features/results/ResultsHeader.tsx
import type { QueryResult } from "@/lib/types";

interface Props {
  result?: QueryResult;
  error?: string;
  busy?: boolean;
}

export function ResultsHeader({ result, error, busy }: Props) {
  return (
    <div className="border-border bg-muted/40 flex items-center gap-3 border-b px-3 py-1.5 text-xs">
      {busy && <span className="text-muted-foreground">Running…</span>}
      {!busy && error && <span className="text-destructive">{error}</span>}
      {!busy && result && (
        <>
          <span>{result.rowCount} rows</span>
          <span className="text-muted-foreground">·</span>
          <span>{result.durationMs} ms</span>
        </>
      )}
      {!busy && !result && !error && (
        <span className="text-muted-foreground">
          No result yet — Cmd+Enter to run.
        </span>
      )}
    </div>
  );
}
```

- [ ] **Step 4: `ResultsGrid.tsx`**

```tsx
// src/features/results/ResultsGrid.tsx
import { useMemo, useRef } from "react";
import {
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type ColumnDef,
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";

import type { QueryResult } from "@/lib/types";

import { Cell } from "./cells";

type Row = Record<string, unknown>;

export function ResultsGrid({ result }: { result: QueryResult }) {
  const data = useMemo<Row[]>(
    () =>
      result.rows.map((row) => {
        const obj: Row = {};
        result.columns.forEach((c, i) => (obj[c.name] = row[i]));
        return obj;
      }),
    [result],
  );

  const columns = useMemo<ColumnDef<Row>[]>(
    () =>
      result.columns.map((c) => ({
        accessorKey: c.name,
        header: () => (
          <div className="flex flex-col leading-tight">
            <span className="text-foreground text-xs font-medium">
              {c.name}
            </span>
            <span className="text-muted-foreground text-[10px]">
              {c.type_name}
            </span>
          </div>
        ),
        cell: (info) => <Cell value={info.getValue()} type={c.type_name} />,
      })),
    [result.columns],
  );

  const table = useReactTable({
    data,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  const parentRef = useRef<HTMLDivElement>(null);
  const rowVirtualizer = useVirtualizer({
    count: table.getRowModel().rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 28,
    overscan: 12,
  });

  return (
    <div ref={parentRef} className="flex-1 overflow-auto font-mono text-xs">
      <table className="w-full border-collapse">
        <thead className="bg-muted/50 sticky top-0 z-10">
          {table.getHeaderGroups().map((hg) => (
            <tr key={hg.id}>
              {hg.headers.map((h) => (
                <th
                  key={h.id}
                  onClick={h.column.getToggleSortingHandler()}
                  className="border-border cursor-pointer border-b px-3 py-1.5 text-left"
                >
                  {flexRender(h.column.columnDef.header, h.getContext())}
                  {h.column.getIsSorted() === "asc" && " ▲"}
                  {h.column.getIsSorted() === "desc" && " ▼"}
                </th>
              ))}
            </tr>
          ))}
        </thead>
        <tbody
          style={{
            height: `${rowVirtualizer.getTotalSize()}px`,
            position: "relative",
            display: "block",
          }}
        >
          {rowVirtualizer.getVirtualItems().map((vi) => {
            const row = table.getRowModel().rows[vi.index];
            return (
              <tr
                key={row.id}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  display: "table",
                  tableLayout: "fixed",
                  width: "100%",
                  transform: `translateY(${vi.start}px)`,
                }}
              >
                {row.getVisibleCells().map((cell) => (
                  <td
                    key={cell.id}
                    className="border-border max-w-[24rem] truncate border-b px-3 py-1"
                  >
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </td>
                ))}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
```

- [ ] **Step 5: Wire results pane into `EditorPane.tsx`**

Add a settings flag for auto-LIMIT (in-component constant for now, surfaced in Task 11):

```tsx
import { ResultsGrid } from "@/features/results/ResultsGrid";
import { ResultsHeader } from "@/features/results/ResultsHeader";
import { withAutoLimit } from "@/lib/sql";

const AUTO_LIMIT_DEFAULT = 1000;
```

In `run()`, replace the call:

```tsx
const sqlToRun = withAutoLimit(activeTab.sql, AUTO_LIMIT_DEFAULT);
const result = await executeQuery(connectionForTab, sqlToRun);
```

After the editor `<div>`, add:

```tsx
<div className="flex max-h-[45vh] min-h-[120px] flex-col">
  <ResultsHeader
    result={activeTab.lastResult}
    error={activeTab.lastError}
    busy={activeTab.busy}
  />
  {activeTab.lastResult && <ResultsGrid result={activeTab.lastResult} />}
</div>
```

(Restructure so the `EditorPane` flex column is `[tabs][toolbar][editor 1fr][results auto]`.)

- [ ] **Step 6: Quality gates**

```bash
pnpm typecheck && pnpm lint && pnpm format && pnpm build
```

- [ ] **Step 7: Manual smoke**

```bash
pnpm tauri dev
# In the docker postgres:
#   CREATE TABLE big AS SELECT i, 'row '||i AS name, ('{"k":'||i||'}')::jsonb AS payload
#   FROM generate_series(1, 50000) i;
# Then: SELECT * FROM big;
```

Expected:

- Result grid auto-applies `LIMIT 1000`.
- Headers show `name (text)`, `payload (jsonb)`, etc.
- `null` cells render in italics.
- Click `payload` cell → expands.
- Click column header → sort indicator appears.
- Smooth scroll, no obvious jank for 1000 rows.

- [ ] **Step 8: Commit**

```bash
git add src
git commit -m "feat(frontend): TanStack result grid with virtualization + auto-LIMIT

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 11: Settings, connection-lost watcher, manual verification

**Goal:** Surface the auto-LIMIT toggle in a settings store (persisted in localStorage), emit `connection:lost` from Rust when the SSH child exits, and wrap up with a manual verification checklist that exercises the Week 2 acceptance criteria.

**Files:**

- Create: `src/store/settings.ts`
- Modify: `src/features/editor/EditorPane.tsx`
- Modify: `src/App.tsx` (small "Auto LIMIT" toggle in toolbar)
- Modify: `src-tauri/src/db/pool.rs` (event emit hook)
- Modify: `src-tauri/src/commands/connections.rs` (spawn watcher tokio task)
- Modify: `src/store/connections.ts` (listen for lost event)
- Modify: `src-tauri/Cargo.toml` (already has `tauri` — no new deps)
- Create: `docs/superpowers/plans/manual-verification-week-2.md`

**Steps:**

- [ ] **Step 1: Settings store**

```ts
// src/store/settings.ts
import { create } from "zustand";
import { persist } from "zustand/middleware";

interface SettingsState {
  autoLimit: number; // 0 = off
  setAutoLimit: (v: number) => void;
}

export const useSettings = create<SettingsState>()(
  persist(
    (set) => ({
      autoLimit: 1000,
      setAutoLimit: (v) => set({ autoLimit: v }),
    }),
    { name: "tusk-settings" },
  ),
);
```

- [ ] **Step 2: Use settings in `EditorPane.tsx`**

Replace `AUTO_LIMIT_DEFAULT` with:

```tsx
import { useSettings } from "@/store/settings";

const autoLimit = useSettings((s) => s.autoLimit);
const sqlToRun =
  autoLimit > 0 ? withAutoLimit(activeTab.sql, autoLimit) : activeTab.sql;
```

- [ ] **Step 3: Toolbar toggle in `App.tsx`**

Inside `<aside>` footer (under the schema tree), add:

```tsx
import { useSettings } from "@/store/settings";

// inside the sidebar bottom:
<div className="border-border border-t p-3 text-xs">
  <label className="flex items-center justify-between gap-2">
    <span className="text-muted-foreground">Auto LIMIT</span>
    <input
      type="number"
      min={0}
      step={100}
      className="border-input w-24 rounded border px-2 py-1"
      value={useSettings((s) => s.autoLimit)}
      onChange={(e) =>
        useSettings.getState().setAutoLimit(Number(e.target.value) || 0)
      }
    />
  </label>
  <p className="text-muted-foreground mt-1">
    0 = off. Skipped if SQL has its own LIMIT.
  </p>
</div>;
```

- [ ] **Step 4: Rust — emit `connection:lost` on tunnel exit**

In `commands/connections.rs::connect`, after `registry.connect_tunneled(...)?;` for both SSH branches, spawn a watcher. Refactor to obtain the tunnel `Child` PID via a helper. Simplest path: have `open_tunnel` accept an `app_handle: tauri::AppHandle` and own the watcher itself. Update signature:

`ssh/tunnel.rs`:

```rust
use tauri::{AppHandle, Emitter};

pub async fn open_tunnel(
    app: AppHandle,
    connection_id: String,
    spec: TunnelSpec,
) -> TuskResult<TunnelHandle> {
    // ... existing body that spawns and waits for readiness ...
    let mut handle = TunnelHandle { child, local_port };

    // Spawn a task that emits when the child exits.
    let id_for_task = connection_id.clone();
    let pid = handle.child.id();
    let app_for_task = app.clone();
    tokio::spawn(async move {
        // Poll: probe whether the child PID is still alive by sending signal 0.
        // We use `nix` (not raw libc) because the crate forbids `unsafe_code`.
        //
        // Windows path is a no-op for now — auto-detect of tunnel death on
        // Windows is a v1.5 follow-up.
        #[cfg(unix)]
        {
            use nix::sys::signal::kill;
            use nix::unistd::Pid;
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                let alive = kill(Pid::from_raw(pid as i32), None).is_ok();
                if !alive {
                    let _ = app_for_task.emit("connection:lost", &id_for_task);
                    break;
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = (app_for_task, id_for_task, pid); // silence unused
        }
    });

    Ok(handle)
}
```

Add to `Cargo.toml`:

```toml
[target.'cfg(unix)'.dependencies]
nix = { version = "0.29", features = ["signal"] }
```

Update `commands/connections.rs` callsites: pass `app.clone()` and `id.clone()`:

```rust
#[tauri::command]
pub async fn connect(
    app: tauri::AppHandle,
    store: State<'_, StateStore>,
    registry: State<'_, ConnectionRegistry>,
    id: String,
) -> TuskResult<()> {
    // ... existing logic, but call:
    let tunnel = open_tunnel(app.clone(), id.clone(), TunnelSpec { ... }).await?;
}
```

- [ ] **Step 5: Frontend listens for `connection:lost`**

In `src/store/connections.ts`, add a top-level subscription set up once on first import:

```ts
import { listen } from "@tauri-apps/api/event";

let subscribed = false;
function ensureLostListener() {
  if (subscribed) return;
  subscribed = true;
  listen<string>("connection:lost", (e) => {
    const id = e.payload;
    useConnections.getState().refresh();
    import("sonner").then(({ toast }) => toast.error(`Lost connection ${id}`));
  });
}

ensureLostListener();
```

Append at the bottom of the file (after the `create` block).

- [ ] **Step 6: Manual verification doc**

`docs/superpowers/plans/manual-verification-week-2.md`:

````markdown
# Week 2 — Manual Verification Checklist

Run after Task 11. Postgres docker must be up:

```bash
docker compose -f infra/postgres/docker-compose.yml up -d
pnpm tauri dev
```

## Direct TCP

- [ ] `+ New connection` → Direct TCP tab → name `local`, host `127.0.0.1`,
      port `55432`, user/password `tusk`, database `tusk_test`, SSL `disable`.
      Save.
- [ ] Click plug icon → green dot, "Connected to local" toast.
- [ ] Schema tree expands to show `public` schema → tables (initially empty
      unless we seed) → no error.
- [ ] In the editor: `CREATE TABLE smoke (id int, name text);` then `Cmd+Enter`.
      Header: `0 rows · <X> ms`.
- [ ] `INSERT INTO smoke VALUES (1, 'a'), (2, 'b'), (3, NULL);` then
      `SELECT * FROM smoke;` — grid shows 3 rows, NULL italicised.

## SSH alias (your own ~/.ssh/config)

- [ ] `+ New connection` → SSH alias tab → list shows your hosts.
- [ ] Click an alias that maps to a Postgres bastion + ProxyJump (e.g. oci-db).
      Fill Postgres host (e.g. 127.0.0.1) / port (5432) / user / password /
      database. Save → connect.
- [ ] Schema tree loads.
- [ ] Run `SELECT version();` — succeeds.

## SSH manual

- [ ] `+ New connection` → SSH manual → SSH host/user/port/key path. Save and
      connect to a known reachable target.

## Editor / tabs

- [ ] `Cmd+T` opens a fresh tab. `Cmd+W` closes it. With one tab open,
      `Cmd+W` should reset to a fresh empty tab.
- [ ] `Cmd+Enter` runs the active tab's SQL.

## Result grid

- [ ] On a 50k-row table, `SELECT *` is auto-LIMITed to 1000.
- [ ] Setting auto-LIMIT to 0 disables auto-append (next run shows full set —
      or, given safety, run with `LIMIT 5000`).
- [ ] JSON cell expands on click.
- [ ] Sorting toggles arrows in the header.

## Error paths

- [ ] Wrong password → connect fails with toast (`TuskError(Connection)`).
- [ ] Bogus SSH alias → connect fails after ≤6s (`TuskError(Tunnel)`).
- [ ] Kill the tunnel from the OS (`pkill -f 'ssh -N -L'`) — within ~1s the
      sidebar dot turns grey, "Lost connection" toast.

## Theme + brand

- [ ] Toggle theme in light/dark — palette stays consistent (Tusk Amber
      visible on `+ New connection` button).
````

- [ ] **Step 7: Quality gates (full set)**

```bash
pnpm typecheck && pnpm lint && pnpm format:check
pnpm build
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
cargo test --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 8: Walk the manual verification checklist**

Run through `docs/superpowers/plans/manual-verification-week-2.md`. Tick the boxes (in your own copy / by hand). Open issues for any failures.

- [ ] **Step 9: Commit**

```bash
git add src src-tauri docs/superpowers/plans/manual-verification-week-2.md
git commit -m "feat: settings store, connection-lost event, Week 2 verification doc

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

- [ ] **Step 10: Tag end of Week 2**

```bash
git tag week-2-complete
```

---

## Self-review (post-write)

| Spec section                         | Implemented in                                          |
| ------------------------------------ | ------------------------------------------------------- |
| §3 Architecture                      | T1–T11 (registry struct: T3)                            |
| §4 Libraries                         | T1 (anyhow/thiserror/keyring), T2 (rusqlite), T3 (sqlx) |
| §5 Connection model + SQLite schema  | T2                                                      |
| §6 SSH integration (config + tunnel) | T5 + T6 + T11 (lost watcher)                            |
| §7 Connection-add UX                 | T4 (Direct TCP) + T7 (SSH tabs)                         |
| §8 Schema tree                       | T8                                                      |
| §9 SQL editor                        | T9                                                      |
| §10 Result grid + auto-LIMIT         | T10 + T11 (toggle)                                      |
| §11 Data flow query                  | T3 (`execute_query`) + T9/T10 (frontend)                |
| §12 Errors                           | T1 (`TuskError`) + frontend wrapper (T4)                |
| §13 Testing                          | T1/T2/T5/T6 unit + T3 integration                       |
| §14 Folder structure                 | T2/T3/T5/T6/T8/T9/T10                                   |
| §15 Slice order                      | Tasks ordered to match                                  |

No `TBD`/`TODO` placeholders. Type names consistent: `ConnectionRecord`,
`NewConnection`, `ConnectionListItem`, `QueryResult`, `SshHost`,
`TunnelHandle`, `TunnelSpec`, `SshTarget`, `ColumnInfo` — used identically
across Rust + TS.

Known limitations carried forward:

- Connection-lost watcher uses Unix `kill(pid, 0)`; Windows path falls back to
  no-op (documented in T11 inline). Address in v1.5.
- `execute_query` cell decoder is best-effort across PG types; rare types
  render as `"<unsupported type>"`. Sufficient for Week 2 acceptance.
