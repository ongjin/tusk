//! Requires docker postgres up at 127.0.0.1:55432/tusk_test (user/pwd: tusk/tusk).
use sqlx::postgres::PgPoolOptions;
use tusk_lib::db::pg_meta::{fetch_table_meta, MetaCache};

const URL: &str = "postgres://tusk:tusk@127.0.0.1:55432/tusk_test";

async fn pool() -> sqlx::PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .connect(URL)
        .await
        .unwrap()
}

#[tokio::test]
async fn fetches_pk_and_columns() {
    let pool = pool().await;
    sqlx::query("DROP TABLE IF EXISTS pg_meta_t CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE pg_meta_t (id int primary key, name text not null, note text)")
        .execute(&pool)
        .await
        .unwrap();

    let cache = MetaCache::new();
    let m = fetch_table_meta(&pool, &cache, "c1", "public", "pg_meta_t")
        .await
        .unwrap();
    assert_eq!(m.pk_columns, vec!["id".to_string()]);
    assert_eq!(m.columns.len(), 3);
    assert!(!m.columns[0].nullable);
    assert!(!m.columns[1].nullable);
    assert!(m.columns[2].nullable);
}

#[tokio::test]
async fn enum_values_loaded() {
    let pool = pool().await;
    sqlx::query("DROP TABLE IF EXISTS pg_meta_e CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP TYPE IF EXISTS mood2")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TYPE mood2 AS ENUM ('sad','ok','happy')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE pg_meta_e (id int primary key, m mood2)")
        .execute(&pool)
        .await
        .unwrap();
    let cache = MetaCache::new();
    let m = fetch_table_meta(&pool, &cache, "c1", "public", "pg_meta_e")
        .await
        .unwrap();
    let mcol = m.columns.iter().find(|c| c.name == "m").unwrap();
    assert_eq!(
        mcol.enum_values.as_ref().unwrap(),
        &vec!["sad".to_string(), "ok".into(), "happy".into()]
    );
}

#[tokio::test]
async fn fk_target_resolved() {
    let pool = pool().await;
    sqlx::query("DROP TABLE IF EXISTS pg_meta_child CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS pg_meta_parent CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE pg_meta_parent (id int primary key)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE pg_meta_child (id int primary key, p int references pg_meta_parent(id))",
    )
    .execute(&pool)
    .await
    .unwrap();
    let cache = MetaCache::new();
    let m = fetch_table_meta(&pool, &cache, "c1", "public", "pg_meta_child")
        .await
        .unwrap();
    let pcol = m.columns.iter().find(|c| c.name == "p").unwrap();
    let fk = pcol.fk.as_ref().unwrap();
    assert_eq!(fk.schema, "public");
    assert_eq!(fk.table, "pg_meta_parent");
    assert_eq!(fk.column, "id");
}

#[tokio::test]
async fn cache_returns_stale_meta_within_ttl() {
    // Prove fetch_table_meta consults the cache: drop the table after first
    // fetch and confirm the second fetch still returns cached metadata
    // (would error if it hit the DB).
    let pool = pool().await;
    sqlx::query("DROP TABLE IF EXISTS pg_meta_cache_t CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE pg_meta_cache_t (id int primary key, name text)")
        .execute(&pool)
        .await
        .unwrap();

    let cache = MetaCache::new();
    let first = fetch_table_meta(&pool, &cache, "c1", "public", "pg_meta_cache_t")
        .await
        .unwrap();
    assert_eq!(first.columns.len(), 2);

    // Drop the table; if cache is consulted, second call still returns 2 cols.
    sqlx::query("DROP TABLE pg_meta_cache_t")
        .execute(&pool)
        .await
        .unwrap();

    let second = fetch_table_meta(&pool, &cache, "c1", "public", "pg_meta_cache_t")
        .await
        .unwrap();
    assert_eq!(second.columns.len(), 2);
}

#[tokio::test]
async fn invalidate_conn_clears_entries() {
    let pool = pool().await;
    sqlx::query("DROP TABLE IF EXISTS pg_meta_inv_t CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE pg_meta_inv_t (id int primary key)")
        .execute(&pool)
        .await
        .unwrap();

    let cache = MetaCache::new();
    let _ = fetch_table_meta(&pool, &cache, "c1", "public", "pg_meta_inv_t")
        .await
        .unwrap();

    sqlx::query("DROP TABLE pg_meta_inv_t")
        .execute(&pool)
        .await
        .unwrap();
    cache.invalidate_conn("c1");

    // After invalidation + DROP, fetching must hit the DB and error out.
    let res = fetch_table_meta(&pool, &cache, "c1", "public", "pg_meta_inv_t").await;
    assert!(res.is_err(), "expected error after invalidate, got {res:?}");
}
