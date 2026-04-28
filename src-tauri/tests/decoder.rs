//! Integration tests for db::decoder against a real postgres.
//! Requires `docker compose -f infra/postgres/docker-compose.yml up -d`.

use sqlx::postgres::PgPoolOptions;
use tusk_lib::db::decoder::{columns_of, decode_row, Cell};

const URL: &str = "postgres://tusk:tusk@127.0.0.1:55432/tusk_test";

fn skip_if_no_postgres() -> bool {
    use std::net::TcpStream;
    use std::time::Duration;
    TcpStream::connect_timeout(&"127.0.0.1:55432".parse().unwrap(), Duration::from_secs(1)).is_err()
}

async fn pool() -> sqlx::PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .connect(URL)
        .await
        .expect("docker postgres must be up")
}

#[tokio::test]
async fn decodes_core_types_round_trip() {
    if skip_if_no_postgres() {
        eprintln!("Postgres not running on 127.0.0.1:55432 — test skipped");
        return;
    }
    let pool = pool().await;
    let rows = sqlx::query(
        "SELECT
            true::bool                              AS b,
            (32767::int2)                            AS i2,
            (2147483647::int4)                       AS i4,
            (9223372036854775807::int8)              AS i8,
            (1.5::float4)                            AS f4,
            (1.5::float8)                            AS f8,
            (1.234::numeric)                         AS num,
            'hello'::text                            AS t,
            '\\x01ff'::bytea                          AS bytes,
            '550e8400-e29b-41d4-a716-446655440000'::uuid AS id,
            '10.0.0.1/32'::inet                      AS ip,
            '2026-04-28'::date                       AS d,
            '12:34:56'::time                         AS tm,
            '2026-04-28 12:34:56'::timestamp         AS ts,
            '2026-04-28 12:34:56+00'::timestamptz    AS tstz,
            '1 hour 30 minutes'::interval            AS iv,
            '{\"k\":1}'::jsonb                        AS jb,
            ARRAY[1,2,3]::int4[]                     AS arr,
            NULL::int                                AS nul",
    )
    .fetch_all(&pool)
    .await
    .expect("query");

    let cols = columns_of(&rows[0]);
    let cells = decode_row(&rows[0], &cols);
    assert!(matches!(cells[0], Cell::Bool(true)));
    assert!(matches!(cells[1], Cell::Int(32767)));
    assert!(matches!(cells[2], Cell::Int(2147483647)));
    if let Cell::Bigint(s) = &cells[3] {
        assert_eq!(s, "9223372036854775807");
    } else {
        panic!("i8")
    }
    assert!(matches!(cells[4], Cell::Float(_)));
    assert!(matches!(cells[5], Cell::Float(_)));
    // sqlx's BigDecimal decode of `1.234::numeric` preserves PG's stored scale (4 digits → "1.2340").
    // We deliberately do NOT call .normalized() in the decoder so editing round-trips can preserve scale
    // (e.g. UPDATE on numeric(10,4) columns where trailing zeros are semantically significant).
    if let Cell::Numeric(s) = &cells[6] {
        assert_eq!(s, "1.2340");
    } else {
        panic!("num")
    }
    if let Cell::Text(s) = &cells[7] {
        assert_eq!(s, "hello");
    } else {
        panic!("text")
    }
    assert!(matches!(cells[8], Cell::Bytea { .. }));
    assert!(matches!(cells[9], Cell::Uuid(_)));
    assert!(matches!(cells[10], Cell::Inet(_)));
    assert!(matches!(cells[11], Cell::Date(_)));
    assert!(matches!(cells[12], Cell::Time(_)));
    assert!(matches!(cells[13], Cell::Timestamp(_)));
    assert!(matches!(cells[14], Cell::Timestamptz(_)));
    if let Cell::Interval { iso } = &cells[15] {
        assert!(iso.starts_with("PT"));
        assert!(iso.contains('H'));
    } else {
        panic!("interval")
    }
    assert!(matches!(cells[16], Cell::Json(_)));
    if let Cell::Array { elem, values } = &cells[17] {
        assert_eq!(elem, "int4");
        assert_eq!(values.len(), 3);
    } else {
        panic!("array")
    }
    assert!(matches!(cells[18], Cell::Null));
}
