//! Integration tests for vector introspection commands.
//! Requires `docker compose -f infra/postgres/docker-compose.yml up -d`.
//! Skipped automatically if the test DB is unreachable.

mod common;

use common::test_registry_with_connection;
use tauri::Manager;
use tusk_lib::commands::vector::{
    list_vector_columns, list_vector_indexes, sample_vectors, VectorColumn,
};

// ── helpers ──────────────────────────────────────────────────────────────────

/// Build a Tauri mock app, manage `registry`, and return both.
/// `tauri::State::from(&registry)` is not available in Tauri 2 because
/// `State`'s inner field is private. The idiomatic test alternative is to
/// use `tauri::test::mock_app`, call `app.manage(registry)`, then obtain
/// `State<'_, ConnectionRegistry>` via `app.state::<ConnectionRegistry>()`.
macro_rules! with_state {
    ($registry:expr, $app:ident, $state:ident) => {
        let $app = tauri::test::mock_app();
        $app.manage($registry);
        let $state = $app.state::<tusk_lib::db::pool::ConnectionRegistry>();
    };
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn list_vector_columns_returns_dim_and_index_flag() {
    if common::skip_if_no_postgres() {
        eprintln!("Postgres not running — test skipped");
        return;
    }

    let (registry, conn_id) = test_registry_with_connection().await;
    let pool = registry.pool(&conn_id).unwrap();

    sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS w6_vc_a, w6_vc_b CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE w6_vc_a (id serial primary key, emb vector(8))")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE w6_vc_b (id serial primary key, emb vector(16))")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "CREATE INDEX ON w6_vc_a USING hnsw (emb vector_cosine_ops) WITH (m=16, ef_construction=64)",
    )
    .execute(&pool)
    .await
    .unwrap();

    with_state!(registry, app, state);

    let cols: Vec<VectorColumn> = list_vector_columns(state, conn_id.clone()).await.unwrap();
    let a = cols.iter().find(|c| c.table == "w6_vc_a").unwrap();
    let b = cols.iter().find(|c| c.table == "w6_vc_b").unwrap();
    assert_eq!(a.dim, 8);
    assert!(a.has_index);
    assert_eq!(b.dim, 16);
    assert!(!b.has_index);

    app.state::<tusk_lib::db::pool::ConnectionRegistry>()
        .inner()
        .disconnect(&conn_id)
        .await
        .ok();
}

#[tokio::test]
async fn list_vector_indexes_parses_hnsw_params() {
    if common::skip_if_no_postgres() {
        eprintln!("Postgres not running — test skipped");
        return;
    }

    let (registry, conn_id) = test_registry_with_connection().await;
    let pool = registry.pool(&conn_id).unwrap();

    sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS w6_vi_h CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE w6_vi_h (id serial primary key, emb vector(8))")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "CREATE INDEX ON w6_vi_h USING hnsw (emb vector_cosine_ops) WITH (m=8, ef_construction=32)",
    )
    .execute(&pool)
    .await
    .unwrap();

    with_state!(registry, app, state);

    let idx = list_vector_indexes(state, conn_id.clone(), "public".into(), "w6_vi_h".into())
        .await
        .unwrap();
    assert_eq!(idx.len(), 1);
    let i = &idx[0];
    assert_eq!(i.method, "hnsw");
    assert_eq!(i.params.m, Some(8));
    assert_eq!(i.params.ef_construction, Some(32));
    assert_eq!(i.params.ops.as_deref(), Some("vector_cosine_ops"));
    assert!(i.size_bytes >= 0);

    app.state::<tusk_lib::db::pool::ConnectionRegistry>()
        .inner()
        .disconnect(&conn_id)
        .await
        .ok();
}

#[tokio::test]
async fn list_vector_indexes_parses_ivfflat_lists() {
    if common::skip_if_no_postgres() {
        eprintln!("Postgres not running — test skipped");
        return;
    }

    let (registry, conn_id) = test_registry_with_connection().await;
    let pool = registry.pool(&conn_id).unwrap();

    sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS w6_vi_iv CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE w6_vi_iv (id serial primary key, emb vector(8))")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO w6_vi_iv (emb) SELECT array_fill(random()::float4, ARRAY[8])::vector FROM generate_series(1,200)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("CREATE INDEX ON w6_vi_iv USING ivfflat (emb vector_l2_ops) WITH (lists=10)")
        .execute(&pool)
        .await
        .unwrap();

    with_state!(registry, app, state);

    let idx = list_vector_indexes(state, conn_id.clone(), "public".into(), "w6_vi_iv".into())
        .await
        .unwrap();
    assert_eq!(idx.len(), 1);
    assert_eq!(idx[0].method, "ivfflat");
    assert_eq!(idx[0].params.lists, Some(10));
    assert_eq!(idx[0].params.ops.as_deref(), Some("vector_l2_ops"));

    app.state::<tusk_lib::db::pool::ConnectionRegistry>()
        .inner()
        .disconnect(&conn_id)
        .await
        .ok();
}

#[tokio::test]
async fn sample_vectors_returns_pk_and_vector() {
    if common::skip_if_no_postgres() {
        eprintln!("Postgres not running — test skipped");
        return;
    }

    let (registry, conn_id) = test_registry_with_connection().await;
    let pool = registry.pool(&conn_id).unwrap();

    sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS w6_sv_a CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE w6_sv_a (id serial primary key, emb vector(4))")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO w6_sv_a (emb) SELECT array_fill(g::float4 / 10.0, ARRAY[4])::vector FROM generate_series(1,10) g",
    )
    .execute(&pool)
    .await
    .unwrap();

    with_state!(registry, app, state);

    let s = sample_vectors(
        state,
        conn_id.clone(),
        "public".into(),
        "w6_sv_a".into(),
        "emb".into(),
        vec!["id".to_string()],
        5,
    )
    .await
    .unwrap();
    assert_eq!(s.rows.len(), 5);
    assert_eq!(s.rows[0].vec.len(), 4);
    // reltuples can be -1 for fresh tables that haven't been ANALYZEd
    assert!(s.total_rows >= -1);
    assert!(s.rows[0].pk_json.get("id").is_some());

    app.state::<tusk_lib::db::pool::ConnectionRegistry>()
        .inner()
        .disconnect(&conn_id)
        .await
        .ok();
}

#[tokio::test]
async fn sample_vectors_handles_composite_pk() {
    if common::skip_if_no_postgres() {
        eprintln!("Postgres not running — test skipped");
        return;
    }

    let (registry, conn_id) = test_registry_with_connection().await;
    let pool = registry.pool(&conn_id).unwrap();

    sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS w6_sv_c CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE w6_sv_c (tenant int, id int, emb vector(3), PRIMARY KEY (tenant, id))",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO w6_sv_c VALUES (1,1,'[0.1,0.2,0.3]'::vector), (1,2,'[0.4,0.5,0.6]'::vector)",
    )
    .execute(&pool)
    .await
    .unwrap();

    with_state!(registry, app, state);

    let s = sample_vectors(
        state,
        conn_id.clone(),
        "public".into(),
        "w6_sv_c".into(),
        "emb".into(),
        vec!["tenant".to_string(), "id".to_string()],
        10,
    )
    .await
    .unwrap();
    assert_eq!(s.rows.len(), 2);
    let pk = s.rows[0].pk_json.as_object().unwrap();
    assert!(pk.contains_key("tenant"));
    assert!(pk.contains_key("id"));
    assert_eq!(s.rows[0].vec.len(), 3);

    app.state::<tusk_lib::db::pool::ConnectionRegistry>()
        .inner()
        .disconnect(&conn_id)
        .await
        .ok();
}
