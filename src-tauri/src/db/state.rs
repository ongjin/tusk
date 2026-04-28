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
