// src-tauri/src/db/pool.rs
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use sqlx::pool::PoolConnection;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use sqlx::PgPool;
use sqlx::Postgres;

use crate::errors::{TuskError, TuskResult};
use crate::ssh::tunnel::TunnelHandle;

#[derive(Debug, Clone)]
pub struct DirectConnectSpec {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
    pub ssl_mode: String,
}

pub struct StickyTx {
    pub tx_id: String,
    pub conn: PoolConnection<Postgres>,
    pub started_at: Instant,
    pub backend_pid: i32,
    pub statement_count: u32,
    pub history_entry_id: String,
}

pub struct ActiveConnection {
    pub pool: PgPool,
    pub tunnel: Option<TunnelHandle>,
    pub tx_slot: Mutex<Option<StickyTx>>,
}

#[derive(Default)]
pub struct ConnectionRegistry {
    inner: Mutex<HashMap<String, Arc<ActiveConnection>>>,
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
            Arc::new(ActiveConnection {
                pool,
                tunnel: None,
                tx_slot: Mutex::new(None),
            }),
        );
        Ok(())
    }

    /// Builds a pool against an already-open SSH local-forward.
    ///
    /// `spec.host` and `spec.port` are **ignored** — connections always go
    /// through `127.0.0.1:tunnel.local_port`. The caller still has to pass
    /// the original Postgres `user`/`password`/`database`/`ssl_mode` though,
    /// since those land in the connection string unchanged.
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
            Arc::new(ActiveConnection {
                pool,
                tunnel: Some(tunnel),
                tx_slot: Mutex::new(None),
            }),
        );
        Ok(())
    }

    pub async fn disconnect(&self, connection_id: &str) -> TuskResult<()> {
        let active = {
            let mut guard = self.inner.lock().expect("registry poisoned");
            guard.remove(connection_id)
        };
        if let Some(active) = active {
            // Take the sticky tx out (if any) so we can rollback.
            let sticky_opt = {
                let mut slot = active.tx_slot.lock().expect("tx slot poisoned");
                slot.take()
            };
            if let Some(mut sticky) = sticky_opt {
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(1),
                    sqlx::query("ROLLBACK").execute(&mut *sticky.conn),
                )
                .await;
            }
            // active drops, pool drops, tunnel drops naturally.
        }
        Ok(())
    }

    pub fn handle(&self, connection_id: &str) -> TuskResult<Arc<ActiveConnection>> {
        let guard = self.inner.lock().expect("registry poisoned");
        guard
            .get(connection_id)
            .cloned()
            .ok_or_else(|| TuskError::Connection(format!("not connected: {connection_id}")))
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
