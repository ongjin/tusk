use sqlx::postgres::PgPoolOptions;

const URL: &str = "postgres://tusk:tusk@127.0.0.1:55432/tusk_test";

#[tokio::test]
async fn begin_commit_round_trip_visible_to_other_session() {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(URL)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS tx_t")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE tx_t (id int)")
        .execute(&pool)
        .await
        .unwrap();

    let mut a = pool.acquire().await.unwrap();
    sqlx::query("BEGIN").execute(&mut *a).await.unwrap();
    sqlx::query("INSERT INTO tx_t VALUES (1)")
        .execute(&mut *a)
        .await
        .unwrap();
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM tx_t")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 0);
    sqlx::query("COMMIT").execute(&mut *a).await.unwrap();
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM tx_t")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn rollback_undoes_writes() {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(URL)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS tx_t2")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE tx_t2 (id int)")
        .execute(&pool)
        .await
        .unwrap();
    let mut a = pool.acquire().await.unwrap();
    sqlx::query("BEGIN").execute(&mut *a).await.unwrap();
    sqlx::query("INSERT INTO tx_t2 VALUES (1)")
        .execute(&mut *a)
        .await
        .unwrap();
    sqlx::query("ROLLBACK").execute(&mut *a).await.unwrap();
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM tx_t2")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 0);
}
