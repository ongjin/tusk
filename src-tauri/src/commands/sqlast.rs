// src-tauri/src/commands/sqlast.rs
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
        _ => {
            return ParsedSelect::NotEditable {
                reason: NotEditableReason::Computed,
            }
        }
    };
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
    let has_function_in_projection = select.projection.iter().any(|item| match item {
        SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => {
            matches!(e, Expr::Function(_))
        }
        _ => false,
    });
    if has_function_in_projection {
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

    #[test]
    fn group_by_is_computed() {
        assert_not_editable("SELECT count(*) FROM users", &NotEditableReason::Computed);
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
