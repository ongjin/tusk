use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

const URL: &str = "postgres://tusk:tusk@127.0.0.1:55432/tusk_test";

/// Issues `pg_cancel_backend(pid)` from a sibling pooled connection while a
/// long-running `pg_sleep(10)` is in flight on a checked-out connection. The
/// victim should error with the canonical "canceling statement" message.
#[tokio::test(flavor = "multi_thread")]
async fn cancel_long_running_select() {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(URL)
        .await
        .unwrap();

    let mut victim = pool.acquire().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *victim)
        .await
        .unwrap();

    let pool2 = pool.clone();
    let canceller = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let _: bool = sqlx::query_scalar("SELECT pg_cancel_backend($1)")
            .bind(pid)
            .fetch_one(&pool2)
            .await
            .unwrap();
    });

    let res = sqlx::query("SELECT pg_sleep(10)")
        .execute(&mut *victim)
        .await;
    canceller.await.unwrap();

    assert!(res.is_err(), "expected cancellation error, got Ok");
    let err = res.unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("canceling statement"),
        "expected 'canceling statement' in error, got: {msg}"
    );
}
