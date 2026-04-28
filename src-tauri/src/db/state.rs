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
        let db = self.db.lock().expect("state lock poisoned");
        let mut stmt = db
            .prepare(
                "SELECT id, name, host, port, db_user, database, ssl_mode, ssh_kind,
                        ssh_alias, ssh_host, ssh_port, ssh_user, ssh_key_path,
                        created_at, updated_at
                 FROM connections WHERE id = ?1",
            )
            .map_err(|e| TuskError::State(e.to_string()))?;
        let mut rows = stmt
            .query(params![id])
            .map_err(|e| TuskError::State(e.to_string()))?;
        let Some(row) = rows.next().map_err(|e| TuskError::State(e.to_string()))? else {
            return Ok(None);
        };

        let ssh_kind_str: String = row.get(7).map_err(|e| TuskError::State(e.to_string()))?;
        Ok(Some(ConnectionRecord {
            id: row.get(0).map_err(|e| TuskError::State(e.to_string()))?,
            name: row.get(1).map_err(|e| TuskError::State(e.to_string()))?,
            host: row.get(2).map_err(|e| TuskError::State(e.to_string()))?,
            port: row.get(3).map_err(|e| TuskError::State(e.to_string()))?,
            db_user: row.get(4).map_err(|e| TuskError::State(e.to_string()))?,
            database: row.get(5).map_err(|e| TuskError::State(e.to_string()))?,
            ssl_mode: row.get(6).map_err(|e| TuskError::State(e.to_string()))?,
            ssh_kind: SshKind::parse(&ssh_kind_str)?,
            ssh_alias: row.get(8).map_err(|e| TuskError::State(e.to_string()))?,
            ssh_host: row.get(9).map_err(|e| TuskError::State(e.to_string()))?,
            ssh_port: row.get(10).map_err(|e| TuskError::State(e.to_string()))?,
            ssh_user: row.get(11).map_err(|e| TuskError::State(e.to_string()))?,
            ssh_key_path: row.get(12).map_err(|e| TuskError::State(e.to_string()))?,
            created_at: row.get(13).map_err(|e| TuskError::State(e.to_string()))?,
            updated_at: row.get(14).map_err(|e| TuskError::State(e.to_string()))?,
        }))
    }

    pub fn delete(&self, id: &str) -> TuskResult<()> {
        let db = self.db.lock().expect("state lock poisoned");
        db.execute("DELETE FROM connections WHERE id = ?1", params![id])
            .map_err(|e| TuskError::State(e.to_string()))?;
        Ok(())
    }

    pub fn insert_history_entry(&self, e: &HistoryEntry) -> TuskResult<()> {
        let db = self.db.lock().expect("state lock poisoned");
        db.execute(
            "INSERT INTO history_entry
             (id, conn_id, source, tx_id, sql_preview, sql_full, started_at,
              duration_ms, row_count, status, error_message, statement_count)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                e.id,
                e.conn_id,
                e.source,
                e.tx_id,
                e.sql_preview,
                e.sql_full,
                e.started_at,
                e.duration_ms,
                e.row_count,
                e.status,
                e.error_message,
                e.statement_count,
            ],
        )
        .map_err(|err| TuskError::History(err.to_string()))?;
        Ok(())
    }

    pub fn append_history_statement(&self, s: &HistoryStatement) -> TuskResult<()> {
        let db = self.db.lock().expect("state lock poisoned");
        db.execute(
            "INSERT INTO history_statement
             (id, entry_id, ordinal, sql, duration_ms, row_count, status, error_message)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                s.id,
                s.entry_id,
                s.ordinal,
                s.sql,
                s.duration_ms,
                s.row_count,
                s.status,
                s.error_message,
            ],
        )
        .map_err(|err| TuskError::History(err.to_string()))?;
        Ok(())
    }

    pub fn update_history_entry_finalize(
        &self,
        id: &str,
        duration_ms: i64,
        row_count: Option<i64>,
        status: &str,
        error: Option<&str>,
        statement_count: i64,
    ) -> TuskResult<()> {
        let db = self.db.lock().expect("state lock poisoned");
        db.execute(
            "UPDATE history_entry
             SET duration_ms = ?2,
                 row_count = ?3,
                 status = ?4,
                 error_message = ?5,
                 statement_count = ?6
             WHERE id = ?1",
            params![id, duration_ms, row_count, status, error, statement_count],
        )
        .map_err(|err| TuskError::History(err.to_string()))?;
        Ok(())
    }

    pub fn list_history(
        &self,
        conn_id: Option<&str>,
        query: Option<&str>,
        limit: i64,
    ) -> TuskResult<Vec<HistoryEntry>> {
        let db = self.db.lock().expect("state lock poisoned");

        let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<HistoryEntry> {
            Ok(HistoryEntry {
                id: row.get(0)?,
                conn_id: row.get(1)?,
                source: row.get(2)?,
                tx_id: row.get(3)?,
                sql_preview: row.get(4)?,
                sql_full: row.get(5)?,
                started_at: row.get(6)?,
                duration_ms: row.get(7)?,
                row_count: row.get(8)?,
                status: row.get(9)?,
                error_message: row.get(10)?,
                statement_count: row.get(11)?,
            })
        };

        let mut out: Vec<HistoryEntry> = Vec::new();
        match (conn_id, query) {
            (Some(cid), Some(q)) => {
                let pattern = format!("%{q}%");
                let mut stmt = db
                    .prepare(
                        "SELECT id, conn_id, source, tx_id, sql_preview, sql_full, started_at,
                                duration_ms, row_count, status, error_message, statement_count
                         FROM history_entry
                         WHERE conn_id = ?1 AND sql_preview LIKE ?2
                         ORDER BY started_at DESC
                         LIMIT ?3",
                    )
                    .map_err(|e| TuskError::History(e.to_string()))?;
                let rows = stmt
                    .query_map(params![cid, pattern, limit], map_row)
                    .map_err(|e| TuskError::History(e.to_string()))?;
                for r in rows {
                    out.push(r.map_err(|e| TuskError::History(e.to_string()))?);
                }
            }
            (Some(cid), None) => {
                let mut stmt = db
                    .prepare(
                        "SELECT id, conn_id, source, tx_id, sql_preview, sql_full, started_at,
                                duration_ms, row_count, status, error_message, statement_count
                         FROM history_entry
                         WHERE conn_id = ?1
                         ORDER BY started_at DESC
                         LIMIT ?2",
                    )
                    .map_err(|e| TuskError::History(e.to_string()))?;
                let rows = stmt
                    .query_map(params![cid, limit], map_row)
                    .map_err(|e| TuskError::History(e.to_string()))?;
                for r in rows {
                    out.push(r.map_err(|e| TuskError::History(e.to_string()))?);
                }
            }
            (None, Some(q)) => {
                let pattern = format!("%{q}%");
                let mut stmt = db
                    .prepare(
                        "SELECT id, conn_id, source, tx_id, sql_preview, sql_full, started_at,
                                duration_ms, row_count, status, error_message, statement_count
                         FROM history_entry
                         WHERE sql_preview LIKE ?1
                         ORDER BY started_at DESC
                         LIMIT ?2",
                    )
                    .map_err(|e| TuskError::History(e.to_string()))?;
                let rows = stmt
                    .query_map(params![pattern, limit], map_row)
                    .map_err(|e| TuskError::History(e.to_string()))?;
                for r in rows {
                    out.push(r.map_err(|e| TuskError::History(e.to_string()))?);
                }
            }
            (None, None) => {
                let mut stmt = db
                    .prepare(
                        "SELECT id, conn_id, source, tx_id, sql_preview, sql_full, started_at,
                                duration_ms, row_count, status, error_message, statement_count
                         FROM history_entry
                         ORDER BY started_at DESC
                         LIMIT ?1",
                    )
                    .map_err(|e| TuskError::History(e.to_string()))?;
                let rows = stmt
                    .query_map(params![limit], map_row)
                    .map_err(|e| TuskError::History(e.to_string()))?;
                for r in rows {
                    out.push(r.map_err(|e| TuskError::History(e.to_string()))?);
                }
            }
        }
        Ok(out)
    }

    pub fn list_history_statements(&self, entry_id: &str) -> TuskResult<Vec<HistoryStatement>> {
        let db = self.db.lock().expect("state lock poisoned");
        let mut stmt = db
            .prepare(
                "SELECT id, entry_id, ordinal, sql, duration_ms, row_count, status, error_message
                 FROM history_statement
                 WHERE entry_id = ?1
                 ORDER BY ordinal ASC",
            )
            .map_err(|e| TuskError::History(e.to_string()))?;
        let rows = stmt
            .query_map(params![entry_id], |row| {
                Ok(HistoryStatement {
                    id: row.get(0)?,
                    entry_id: row.get(1)?,
                    ordinal: row.get(2)?,
                    sql: row.get(3)?,
                    duration_ms: row.get(4)?,
                    row_count: row.get(5)?,
                    status: row.get(6)?,
                    error_message: row.get(7)?,
                })
            })
            .map_err(|e| TuskError::History(e.to_string()))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| TuskError::History(e.to_string()))?);
        }
        Ok(out)
    }
}

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

    fn entry_fixture(id: &str, conn_id: &str, preview: &str, status: &str) -> HistoryEntry {
        HistoryEntry {
            id: id.into(),
            conn_id: conn_id.into(),
            source: "editor".into(),
            tx_id: None,
            sql_preview: preview.into(),
            sql_full: Some(preview.into()),
            started_at: 1_700_000_000_000,
            duration_ms: 12,
            row_count: Some(0),
            status: status.into(),
            error_message: None,
            statement_count: 1,
        }
    }

    #[test]
    fn history_entry_round_trip() {
        let store = StateStore::open_in_memory().unwrap();
        let entry = entry_fixture("e1", "c1", "SELECT 1", "ok");
        store.insert_history_entry(&entry).unwrap();

        let listed = store.list_history(Some("c1"), None, 10).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "e1");
        assert_eq!(listed[0].sql_preview, "SELECT 1");
        assert_eq!(listed[0].status, "ok");
        assert_eq!(listed[0].statement_count, 1);

        store
            .update_history_entry_finalize("e1", 99, Some(5), "ok", None, 2)
            .unwrap();
        let after = store.list_history(Some("c1"), None, 10).unwrap();
        assert_eq!(after[0].duration_ms, 99);
        assert_eq!(after[0].row_count, Some(5));
        assert_eq!(after[0].statement_count, 2);
    }

    #[test]
    fn history_search_uses_like() {
        let store = StateStore::open_in_memory().unwrap();
        store
            .insert_history_entry(&entry_fixture("a", "c1", "SELECT * FROM users", "ok"))
            .unwrap();
        store
            .insert_history_entry(&entry_fixture("b", "c1", "INSERT INTO orders", "ok"))
            .unwrap();
        store
            .insert_history_entry(&entry_fixture("c", "c2", "SELECT id FROM users", "ok"))
            .unwrap();

        let users = store.list_history(None, Some("users"), 10).unwrap();
        assert_eq!(users.len(), 2);

        let c1_users = store.list_history(Some("c1"), Some("users"), 10).unwrap();
        assert_eq!(c1_users.len(), 1);
        assert_eq!(c1_users[0].id, "a");

        let none = store.list_history(Some("c2"), Some("orders"), 10).unwrap();
        assert!(none.is_empty());

        let all = store.list_history(None, None, 10).unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn statements_attach_to_entry() {
        let store = StateStore::open_in_memory().unwrap();
        store
            .insert_history_entry(&entry_fixture("e1", "c1", "BEGIN; SELECT 1; COMMIT", "ok"))
            .unwrap();
        for (i, sql) in ["BEGIN", "SELECT 1", "COMMIT"].iter().enumerate() {
            store
                .append_history_statement(&HistoryStatement {
                    id: format!("s{i}"),
                    entry_id: "e1".into(),
                    ordinal: i as i64,
                    sql: (*sql).into(),
                    duration_ms: 1,
                    row_count: None,
                    status: "ok".into(),
                    error_message: None,
                })
                .unwrap();
        }

        let listed = store.list_history_statements("e1").unwrap();
        assert_eq!(listed.len(), 3);
        assert_eq!(listed[0].ordinal, 0);
        assert_eq!(listed[0].sql, "BEGIN");
        assert_eq!(listed[2].sql, "COMMIT");
    }
}
