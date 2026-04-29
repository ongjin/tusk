//! Shared helpers for integration tests that require a live Postgres connection.
//! Requires the docker-compose stack from `infra/postgres/docker-compose.yml`.

pub use tusk_lib::db::pool::{ConnectionRegistry, DirectConnectSpec};

/// Spin up a fresh `ConnectionRegistry`, connect to the docker pgvector test DB
/// using a unique `connection_id`, and return both.
///
/// Panics on failure — callers can guard with [`skip_if_no_postgres`] first.
pub async fn test_registry_with_connection() -> (ConnectionRegistry, String) {
    let registry = ConnectionRegistry::new();
    let host = std::env::var("TUSK_TEST_PG_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("TUSK_TEST_PG_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(55432);
    let spec = DirectConnectSpec {
        host,
        port,
        user: "tusk".into(),
        password: "tusk".into(),
        database: "tusk_test".into(),
        ssl_mode: "disable".into(),
    };
    let conn_id = format!(
        "test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    registry
        .connect_direct(&conn_id, spec)
        .await
        .expect("connect_direct failed");
    (registry, conn_id)
}

/// Returns `true` (skip) if the test Postgres instance is not reachable.
pub fn skip_if_no_postgres() -> bool {
    use std::net::TcpStream;
    use std::time::Duration;
    let host = std::env::var("TUSK_TEST_PG_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("TUSK_TEST_PG_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(55432);
    TcpStream::connect_timeout(
        &format!("{host}:{port}").parse().unwrap(),
        Duration::from_secs(1),
    )
    .is_err()
}
