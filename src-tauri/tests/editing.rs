use sqlx::postgres::PgPoolOptions;
use tusk_lib::commands::editing::*;
use tusk_lib::db::decoder::Cell;

const URL: &str = "postgres://tusk:tusk@127.0.0.1:55432/tusk_test";

async fn pool() -> sqlx::PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .connect(URL)
        .await
        .unwrap()
}

#[tokio::test]
async fn pkonly_update_round_trip() {
    let pool = pool().await;
    sqlx::query("DROP TABLE IF EXISTS edit_t")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE edit_t (id int primary key, email text)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO edit_t VALUES (1, 'old@x')")
        .execute(&pool)
        .await
        .unwrap();

    let b = PendingBatch {
        batch_id: "b1".into(),
        op: PendingOp::Update,
        table: TableRef {
            schema: "public".into(),
            name: "edit_t".into(),
        },
        pk_columns: vec!["id".into()],
        pk_values: vec![Cell::Int(1)],
        edits: vec![ColumnEdit {
            column: "email".into(),
            next: Cell::Text("new@x".into()),
        }],
        captured_row: vec![Cell::Int(1), Cell::Text("old@x".into())],
        captured_columns: vec!["id".into(), "email".into()],
    };
    let built = build_update(&b, ConflictMode::PkOnly).unwrap();
    let mut tx = pool.begin().await.unwrap();
    let q = sqlx::query(&built.parameterized_sql);
    let q = bind_cells(q, &built.binds);
    let res = q.execute(&mut *tx).await.unwrap();
    assert_eq!(res.rows_affected(), 1);
    tx.commit().await.unwrap();

    let v: String = sqlx::query_scalar("SELECT email FROM edit_t WHERE id=1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(v, "new@x");
}

#[tokio::test]
async fn strict_detects_concurrent_change() {
    let pool = pool().await;
    sqlx::query("DROP TABLE IF EXISTS edit_t2")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE edit_t2 (id int primary key, email text, name text)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO edit_t2 VALUES (1, 'old@x', 'old-name')")
        .execute(&pool)
        .await
        .unwrap();

    // Concurrent mutation — simulates "someone else changed it" — modifies a
    // non-edited column so Strict mode (which skips the edited column) detects.
    sqlx::query("UPDATE edit_t2 SET name = 'other-name' WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();

    let b = PendingBatch {
        batch_id: "b1".into(),
        op: PendingOp::Update,
        table: TableRef {
            schema: "public".into(),
            name: "edit_t2".into(),
        },
        pk_columns: vec!["id".into()],
        pk_values: vec![Cell::Int(1)],
        edits: vec![ColumnEdit {
            column: "email".into(),
            next: Cell::Text("new@x".into()),
        }],
        // Captured snapshot — `name` is stale!
        captured_row: vec![
            Cell::Int(1),
            Cell::Text("old@x".into()),
            Cell::Text("old-name".into()),
        ],
        captured_columns: vec!["id".into(), "email".into(), "name".into()],
    };
    let built = build_update(&b, ConflictMode::Strict).unwrap();
    let mut tx = pool.begin().await.unwrap();
    let q = sqlx::query(&built.parameterized_sql);
    let q = bind_cells(q, &built.binds);
    let res = q.execute(&mut *tx).await.unwrap();
    assert_eq!(res.rows_affected(), 0); // conflict detected
}
