use serde_json::Value as JsonValue;
use sqlx::postgres::PgPoolOptions;

use tusk_lib::commands::sqlast::{classify_for_explain, ExplainCategory};
use tusk_lib::db::explain_runner::{category_to_exec_mode, wrap_for_explain, ExplainExecMode};

#[tokio::test]
#[ignore]
async fn explain_select_against_live_pg() {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect("postgres://tusk:tusk@127.0.0.1:55432/tusk_test")
        .await
        .expect("connect");
    let wrapped = wrap_for_explain("SELECT 1", ExplainExecMode::SelectAnalyze);
    let row: (JsonValue,) = sqlx::query_as(&wrapped)
        .fetch_one(&pool)
        .await
        .expect("explain ok");
    let arr = row.0.as_array().expect("array");
    assert_eq!(arr.len(), 1);
    assert!(arr[0]["Plan"].is_object());
}

#[test]
fn classify_basic() {
    assert_eq!(
        category_to_exec_mode(classify_for_explain("SELECT 1"), false),
        Some(ExplainExecMode::SelectAnalyze)
    );
    assert!(matches!(
        classify_for_explain("BEGIN"),
        ExplainCategory::NotExplainable
    ));
}

async fn seed_demo_table(pool: &sqlx::PgPool, table: &str) {
    sqlx::query(&format!("DROP TABLE IF EXISTS {table}"))
        .fetch_all(pool)
        .await
        .unwrap();
    sqlx::query(&format!("CREATE TABLE {table} (id int, label text)"))
        .fetch_all(pool)
        .await
        .unwrap();
    sqlx::query(&format!(
        "INSERT INTO {table} VALUES (1,'a'),(2,'b'),(3,'c')"
    ))
    .fetch_all(pool)
    .await
    .unwrap();
}

#[tokio::test]
#[ignore]
async fn analyze_anyway_rolls_back_delete() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect("postgres://tusk:tusk@127.0.0.1:55432/tusk_test")
        .await
        .unwrap();
    let table = "week5_demo_delete";
    seed_demo_table(&pool, table).await;

    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("BEGIN").fetch_all(&mut *conn).await.unwrap();
    let r: Result<(JsonValue,), sqlx::Error> = sqlx::query_as(&format!(
        "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) DELETE FROM {table}"
    ))
    .fetch_one(&mut *conn)
    .await;
    sqlx::query("ROLLBACK").fetch_all(&mut *conn).await.unwrap();
    assert!(r.is_ok(), "EXPLAIN ANALYZE DELETE should succeed");
    drop(conn);

    let count: (i64,) = sqlx::query_as(&format!("SELECT count(*) FROM {table}"))
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count.0, 3,
        "DELETE inside EXPLAIN ANALYZE must be rolled back"
    );
}

#[tokio::test]
#[ignore]
async fn analyze_anyway_rolls_back_on_error() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect("postgres://tusk:tusk@127.0.0.1:55432/tusk_test")
        .await
        .unwrap();
    let table = "week5_demo_err";
    seed_demo_table(&pool, table).await;

    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("BEGIN").fetch_all(&mut *conn).await.unwrap();
    let _ = sqlx::query("EXPLAIN ANALYZE DELETE FROM no_such_table_x9")
        .fetch_all(&mut *conn)
        .await;
    sqlx::query("ROLLBACK").fetch_all(&mut *conn).await.unwrap();
    drop(conn);

    let count: (i64,) = sqlx::query_as(&format!("SELECT count(*) FROM {table}"))
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 3);
}
