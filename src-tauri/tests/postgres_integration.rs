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
    TcpStream::connect_timeout(
        &format!("{host}:{port}").parse().unwrap(),
        Duration::from_secs(1),
    )
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
