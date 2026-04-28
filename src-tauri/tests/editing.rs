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
    sqlx::query("CREATE TABLE edit_t2 (id int primary key, email text)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO edit_t2 VALUES (1, 'old@x')")
        .execute(&pool)
        .await
        .unwrap();

    // Concurrent mutation — simulates "someone else changed it"
    sqlx::query("UPDATE edit_t2 SET email = 'other@x' WHERE id = 1")
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
        // captured_row is the row state when the user started editing.
        // Concurrent UPDATE has changed email to 'other@x' since.
        captured_row: vec![Cell::Int(1), Cell::Text("old@x".into())],
        captured_columns: vec!["id".into(), "email".into()],
    };
    let built = build_update(&b, ConflictMode::Strict).unwrap();
    let mut tx = pool.begin().await.unwrap();
    let q = sqlx::query(&built.parameterized_sql);
    let q = bind_cells(q, &built.binds);
    let res = q.execute(&mut *tx).await.unwrap();
    assert_eq!(res.rows_affected(), 0); // conflict detected via edited column
}

#[tokio::test]
async fn multi_batch_atomic_rollback_on_conflict() {
    // Two-row batch: first row updates cleanly, second row's strict WHERE
    // misses (concurrent change). Asserts that when we roll back the tx,
    // row 1's successful UPDATE is discarded — the table is unchanged.
    let pool = pool().await;
    sqlx::query("DROP TABLE IF EXISTS edit_t3")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE edit_t3 (id int primary key, email text)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO edit_t3 VALUES (1, 'a@x'), (2, 'b@x')")
        .execute(&pool)
        .await
        .unwrap();

    // Concurrent change on row 2 only — row 1 stays at 'a@x'.
    sqlx::query("UPDATE edit_t3 SET email = 'other@x' WHERE id = 2")
        .execute(&pool)
        .await
        .unwrap();

    let b1 = PendingBatch {
        batch_id: "row1".into(),
        op: PendingOp::Update,
        table: TableRef {
            schema: "public".into(),
            name: "edit_t3".into(),
        },
        pk_columns: vec!["id".into()],
        pk_values: vec![Cell::Int(1)],
        edits: vec![ColumnEdit {
            column: "email".into(),
            next: Cell::Text("a-new@x".into()),
        }],
        captured_row: vec![Cell::Int(1), Cell::Text("a@x".into())],
        captured_columns: vec!["id".into(), "email".into()],
    };
    let b2 = PendingBatch {
        batch_id: "row2".into(),
        op: PendingOp::Update,
        table: TableRef {
            schema: "public".into(),
            name: "edit_t3".into(),
        },
        pk_columns: vec!["id".into()],
        pk_values: vec![Cell::Int(2)],
        edits: vec![ColumnEdit {
            column: "email".into(),
            next: Cell::Text("b-new@x".into()),
        }],
        // captured snapshot is stale — concurrent UPDATE moved row 2 to 'other@x'.
        captured_row: vec![Cell::Int(2), Cell::Text("b@x".into())],
        captured_columns: vec!["id".into(), "email".into()],
    };

    let mut tx = pool.begin().await.unwrap();

    // Apply batch 1 — succeeds within the tx.
    let built1 = build_update(&b1, ConflictMode::Strict).unwrap();
    let q1 = sqlx::query(&built1.parameterized_sql);
    let q1 = bind_cells(q1, &built1.binds);
    let r1 = q1.execute(&mut *tx).await.unwrap();
    assert_eq!(r1.rows_affected(), 1);

    // Apply batch 2 — strict conflict (rows_affected = 0).
    let built2 = build_update(&b2, ConflictMode::Strict).unwrap();
    let q2 = sqlx::query(&built2.parameterized_sql);
    let q2 = bind_cells(q2, &built2.binds);
    let r2 = q2.execute(&mut *tx).await.unwrap();
    assert_eq!(r2.rows_affected(), 0);

    // Roll back — atomic semantics demand row 1's successful UPDATE is undone.
    tx.rollback().await.unwrap();

    let row1: String = sqlx::query_scalar("SELECT email FROM edit_t3 WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row1, "a@x", "row 1 must be unchanged after rollback");

    let row2: String = sqlx::query_scalar("SELECT email FROM edit_t3 WHERE id = 2")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        row2, "other@x",
        "row 2 must still reflect the concurrent change"
    );
}
