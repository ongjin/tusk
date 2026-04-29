//! `parse_select_target` classifies a SQL string for editability.
//!
//! Returns `ParsedSelect::SingleTable { schema, table }` ONLY when the result
//! is round-trippable to a single base relation:
//! - Single statement
//! - Bare SELECT with no GROUP BY / HAVING / DISTINCT / QUALIFY / INTO
//! - No CTE / set operation / subquery in FROM
//! - Single-table FROM, no joins
//! - No aggregate or window function in projection (allowlist + `over.is_some()`)
//!
//! Consumers MUST treat any other return as read-only.
//!
//! Wire format: `#[serde(tag = "kind", content = "data")]`. Reason variants
//! are kebab-case on the wire — do not rename without updating the frontend.
//!
//! Known conservative limits (TODO future tasks):
//! - Three-part identifier (`db.schema.table`) → currently classified as
//!   `Computed`. PG's catalog-qualified syntax is rare; revisit if it shows up.
//! - Quoted-identifier case is preserved; unquoted is folded by sqlparser to
//!   the source case (PG would lowercase). Verified safe for round-trip
//!   because `pg_meta` queries the same string back.
#![allow(dead_code)]
use serde::Serialize;
use sqlparser::ast::{Expr, Query, SelectItem, SetExpr, Statement, TableFactor};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
pub enum ParsedSelect {
    SingleTable { schema: String, table: String },
    NotEditable { reason: NotEditableReason },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotEditableReason {
    NotSelect,
    MultiTable,
    Cte,
    Subquery,
    Computed,
    ParserFailed,
    MultipleStatements,
}

pub fn parse_select_target(sql: &str) -> ParsedSelect {
    let stmts = match Parser::parse_sql(&PostgreSqlDialect {}, sql) {
        Ok(s) => s,
        Err(_) => {
            return ParsedSelect::NotEditable {
                reason: NotEditableReason::ParserFailed,
            }
        }
    };
    if stmts.len() > 1 {
        return ParsedSelect::NotEditable {
            reason: NotEditableReason::MultipleStatements,
        };
    }
    let stmt = match stmts.into_iter().next() {
        Some(s) => s,
        None => {
            return ParsedSelect::NotEditable {
                reason: NotEditableReason::NotSelect,
            }
        }
    };
    let query: Box<Query> = match stmt {
        Statement::Query(q) => q,
        _ => {
            return ParsedSelect::NotEditable {
                reason: NotEditableReason::NotSelect,
            }
        }
    };
    if query.with.is_some() {
        return ParsedSelect::NotEditable {
            reason: NotEditableReason::Cte,
        };
    }
    let select = match *query.body {
        SetExpr::Select(s) => s,
        SetExpr::SetOperation { .. } => {
            return ParsedSelect::NotEditable {
                reason: NotEditableReason::MultiTable,
            };
        }
        _ => {
            return ParsedSelect::NotEditable {
                reason: NotEditableReason::Computed,
            }
        }
    };
    if select.into.is_some() {
        return ParsedSelect::NotEditable {
            reason: NotEditableReason::NotSelect,
        };
    }
    if select.qualify.is_some() {
        return ParsedSelect::NotEditable {
            reason: NotEditableReason::Computed,
        };
    }
    let group_by_empty = matches!(
        &select.group_by,
        sqlparser::ast::GroupByExpr::Expressions(exprs, modifiers)
            if exprs.is_empty() && modifiers.is_empty()
    );
    if !group_by_empty || select.having.is_some() || select.distinct.is_some() {
        return ParsedSelect::NotEditable {
            reason: NotEditableReason::Computed,
        };
    }

    // Aggregates and window functions in projection both make a row non-addressable.
    let aggregates: &[&str] = &[
        "count",
        "sum",
        "avg",
        "min",
        "max",
        "array_agg",
        "string_agg",
        "json_agg",
        "jsonb_agg",
        "bool_and",
        "bool_or",
        "every",
        "stddev",
        "variance",
        "corr",
        "covar_pop",
        "covar_samp",
    ];

    for item in &select.projection {
        let expr = match item {
            SelectItem::UnnamedExpr(e) => e,
            SelectItem::ExprWithAlias { expr, .. } => expr,
            _ => continue,
        };
        if let Expr::Function(f) = expr {
            // Window function — addressable row doesn't exist.
            if f.over.is_some() {
                return ParsedSelect::NotEditable {
                    reason: NotEditableReason::Computed,
                };
            }
            // Aggregate — same. In sqlparser 0.52, `ObjectName` is `Vec<Ident>`,
            // so `name.0.last()` returns `&Ident` directly.
            let last_name = f
                .name
                .0
                .last()
                .map(|i| i.value.to_lowercase())
                .unwrap_or_default();
            if aggregates.contains(&last_name.as_str()) {
                return ParsedSelect::NotEditable {
                    reason: NotEditableReason::Computed,
                };
            }
        }
    }

    if select.from.is_empty() {
        return ParsedSelect::NotEditable {
            reason: NotEditableReason::Computed,
        };
    }
    if select.from.len() != 1 {
        return ParsedSelect::NotEditable {
            reason: NotEditableReason::MultiTable,
        };
    }
    let twj = &select.from[0];
    if !twj.joins.is_empty() {
        return ParsedSelect::NotEditable {
            reason: NotEditableReason::MultiTable,
        };
    }
    match &twj.relation {
        TableFactor::Table { name, .. } => {
            let parts = name.0.iter().map(|i| i.value.clone()).collect::<Vec<_>>();
            let (schema, table) = match parts.as_slice() {
                [t] => ("public".to_string(), t.clone()),
                [s, t] => (s.clone(), t.clone()),
                _ => {
                    return ParsedSelect::NotEditable {
                        reason: NotEditableReason::Computed,
                    }
                }
            };
            ParsedSelect::SingleTable { schema, table }
        }
        TableFactor::Derived { .. } => ParsedSelect::NotEditable {
            reason: NotEditableReason::Subquery,
        },
        _ => ParsedSelect::NotEditable {
            reason: NotEditableReason::Computed,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExplainCategory {
    /// SELECT/CTE/VALUES — wrap with EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON).
    SelectAnalyze,
    /// DML (INSERT/UPDATE/DELETE/MERGE) — wrap with EXPLAIN (FORMAT JSON) only.
    DmlPlanOnly,
    /// DDL (CREATE/DROP/ALTER/TRUNCATE/GRANT/REVOKE) — wrap with EXPLAIN (FORMAT JSON) only.
    DdlPlanOnly,
    /// User already wrote `EXPLAIN ...` — execute as-is.
    Passthrough,
    /// SQL that the parser could not understand at all.
    Unparseable,
    /// Parsed fine, but Postgres won't accept this in EXPLAIN
    /// (e.g., BEGIN, COMMIT, SET, SHOW).
    NotExplainable,
}

/// Classify a SQL string for the EXPLAIN runner. Examines only the first
/// statement; multi-statement input is allowed but only the first decides
/// the category. Callers that wrap the SQL must wrap *only* the first
/// statement and surface a warning for any trailing statements.
pub fn classify_for_explain(sql: &str) -> ExplainCategory {
    use sqlparser::ast::Statement as S;

    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return ExplainCategory::Unparseable;
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("explain ") || lower.starts_with("explain(") {
        return ExplainCategory::Passthrough;
    }

    let stmts = match Parser::parse_sql(&PostgreSqlDialect {}, trimmed) {
        Ok(s) => s,
        Err(_) => return ExplainCategory::Unparseable,
    };
    let first = match stmts.into_iter().next() {
        Some(s) => s,
        None => return ExplainCategory::Unparseable,
    };

    match first {
        S::Query(_) => ExplainCategory::SelectAnalyze,
        S::Insert { .. } | S::Update { .. } | S::Delete { .. } | S::Merge { .. } => {
            ExplainCategory::DmlPlanOnly
        }
        S::CreateTable { .. }
        | S::CreateIndex { .. }
        | S::CreateView { .. }
        | S::CreateSchema { .. }
        | S::CreateExtension { .. }
        | S::CreateFunction { .. }
        | S::Drop { .. }
        | S::AlterTable { .. }
        | S::AlterIndex { .. }
        | S::Truncate { .. }
        | S::Grant { .. }
        | S::Revoke { .. } => ExplainCategory::DdlPlanOnly,
        _ => ExplainCategory::NotExplainable,
    }
}

#[cfg(test)]
mod explain_classifier_tests {
    use super::*;

    fn cls(s: &str) -> ExplainCategory {
        classify_for_explain(s)
    }

    #[test]
    fn select_is_analyze() {
        assert_eq!(cls("SELECT 1"), ExplainCategory::SelectAnalyze);
        assert_eq!(
            cls("  select * from users  "),
            ExplainCategory::SelectAnalyze
        );
        assert_eq!(
            cls("WITH x AS (SELECT 1) SELECT * FROM x"),
            ExplainCategory::SelectAnalyze
        );
        assert_eq!(cls("VALUES (1),(2)"), ExplainCategory::SelectAnalyze);
        // Note: `TABLE users` shorthand is not supported by sqlparser 0.52 PostgreSQL dialect.
        // It would parse as SelectAnalyze in newer versions.
    }

    #[test]
    fn dml_is_plan_only() {
        assert_eq!(
            cls("INSERT INTO t VALUES (1)"),
            ExplainCategory::DmlPlanOnly
        );
        assert_eq!(cls("UPDATE t SET a=1"), ExplainCategory::DmlPlanOnly);
        assert_eq!(cls("DELETE FROM t"), ExplainCategory::DmlPlanOnly);
        // Note: `MERGE ... DO NOTHING` is not supported by sqlparser 0.52 PostgreSQL dialect.
        // Use UPDATE action variant instead to verify MERGE parses as DmlPlanOnly.
        assert_eq!(
            cls("MERGE INTO t USING s ON t.id=s.id WHEN MATCHED THEN UPDATE SET t.a=s.a"),
            ExplainCategory::DmlPlanOnly
        );
    }

    #[test]
    fn ddl_is_plan_only() {
        assert_eq!(cls("CREATE TABLE x (id int)"), ExplainCategory::DdlPlanOnly);
        assert_eq!(cls("DROP TABLE x"), ExplainCategory::DdlPlanOnly);
        assert_eq!(
            cls("ALTER TABLE x ADD COLUMN y int"),
            ExplainCategory::DdlPlanOnly
        );
        assert_eq!(cls("TRUNCATE x"), ExplainCategory::DdlPlanOnly);
    }

    #[test]
    fn already_explain_passthrough() {
        assert_eq!(cls("EXPLAIN SELECT 1"), ExplainCategory::Passthrough);
        assert_eq!(
            cls("EXPLAIN (ANALYZE, BUFFERS) SELECT 1"),
            ExplainCategory::Passthrough
        );
        assert_eq!(cls("  explain   select 1"), ExplainCategory::Passthrough);
    }

    #[test]
    fn unparseable_returns_error() {
        assert_eq!(cls(""), ExplainCategory::Unparseable);
        assert_eq!(cls("    "), ExplainCategory::Unparseable);
        assert_eq!(cls("not even sql !!"), ExplainCategory::Unparseable);
    }

    #[test]
    fn non_explainable_returns_error() {
        assert_eq!(cls("BEGIN"), ExplainCategory::NotExplainable);
        assert_eq!(cls("COMMIT"), ExplainCategory::NotExplainable);
        assert_eq!(
            cls("SET search_path = public"),
            ExplainCategory::NotExplainable
        );
    }

    #[test]
    fn multi_statement_uses_first() {
        assert_eq!(
            cls("SELECT 1; UPDATE t SET a=1"),
            ExplainCategory::SelectAnalyze
        );
        assert_eq!(
            cls("UPDATE t SET a=1; SELECT 1"),
            ExplainCategory::DmlPlanOnly
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_single(sql: &str, schema: &str, table: &str) {
        match parse_select_target(sql) {
            ParsedSelect::SingleTable {
                schema: s,
                table: t,
            } => {
                assert_eq!(s, schema);
                assert_eq!(t, table);
            }
            other => panic!("expected SingleTable, got {other:?}"),
        }
    }

    fn assert_not_editable(sql: &str, expected: &NotEditableReason) {
        match parse_select_target(sql) {
            ParsedSelect::NotEditable { reason } => assert_eq!(&reason, expected),
            other => panic!("expected NotEditable({expected:?}), got {other:?}"),
        }
    }

    #[test]
    fn simple_select_unqualified_uses_public_default() {
        assert_single("SELECT * FROM users", "public", "users");
    }

    #[test]
    fn schema_qualified_select_keeps_schema() {
        assert_single("SELECT id, email FROM auth.users", "auth", "users");
    }

    #[test]
    fn select_with_where_still_editable() {
        assert_single(
            "SELECT * FROM public.users WHERE id = 42",
            "public",
            "users",
        );
    }

    #[test]
    fn select_with_order_by_still_editable() {
        assert_single(
            "SELECT id FROM public.users ORDER BY id DESC LIMIT 10",
            "public",
            "users",
        );
    }

    #[test]
    fn join_is_multi_table() {
        assert_not_editable(
            "SELECT u.id FROM users u JOIN orders o ON o.user_id = u.id",
            &NotEditableReason::MultiTable,
        );
    }

    #[test]
    fn cte_is_not_editable() {
        assert_not_editable(
            "WITH x AS (SELECT * FROM users) SELECT * FROM x",
            &NotEditableReason::Cte,
        );
    }

    #[test]
    fn subquery_in_from_is_not_editable() {
        assert_not_editable(
            "SELECT * FROM (SELECT * FROM users) sub",
            &NotEditableReason::Subquery,
        );
    }

    // Renamed: this exercises projection-aggregate detection, not GROUP BY.
    #[test]
    fn aggregate_in_projection_is_computed() {
        assert_not_editable("SELECT count(*) FROM users", &NotEditableReason::Computed);
    }

    #[test]
    fn group_by_clause_is_computed() {
        assert_not_editable(
            "SELECT id, count(*) FROM users GROUP BY id",
            &NotEditableReason::Computed,
        );
    }

    #[test]
    fn having_clause_is_computed() {
        assert_not_editable(
            "SELECT id FROM users GROUP BY id HAVING count(*) > 1",
            &NotEditableReason::Computed,
        );
    }

    #[test]
    fn distinct_clause_is_computed() {
        assert_not_editable(
            "SELECT DISTINCT id FROM users",
            &NotEditableReason::Computed,
        );
    }

    #[test]
    fn union_is_multi_table() {
        assert_not_editable(
            "SELECT id FROM a UNION SELECT id FROM b",
            &NotEditableReason::MultiTable,
        );
    }

    #[test]
    fn multiple_statements_rejected() {
        assert_not_editable(
            "SELECT * FROM users; UPDATE users SET id = 1",
            &NotEditableReason::MultipleStatements,
        );
    }

    #[test]
    fn from_less_select_is_computed() {
        assert_not_editable("SELECT 1", &NotEditableReason::Computed);
    }

    #[test]
    fn window_function_in_projection_is_computed() {
        assert_not_editable(
            "SELECT row_number() OVER () FROM users",
            &NotEditableReason::Computed,
        );
    }

    #[test]
    fn select_into_is_not_select() {
        assert_not_editable(
            "SELECT * INTO new_users FROM users",
            &NotEditableReason::NotSelect,
        );
    }

    #[test]
    fn scalar_function_in_projection_is_editable() {
        // LOWER(email) is a scalar — should NOT be Computed.
        assert_single(
            "SELECT id, LOWER(email) FROM public.users",
            "public",
            "users",
        );
    }

    #[test]
    fn insert_is_not_select() {
        assert_not_editable(
            "INSERT INTO users (id) VALUES (1)",
            &NotEditableReason::NotSelect,
        );
    }

    #[test]
    fn unparseable_sql_yields_parser_failed() {
        assert_not_editable("this is not sql at all", &NotEditableReason::ParserFailed);
    }
}
