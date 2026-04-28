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
        self.inner
            .lock()
            .expect("registry poisoned")
            .contains_key(connection_id)
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
        other => return Err(TuskError::Connection(format!("unknown ssl_mode '{other}'"))),
    })
}
