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
