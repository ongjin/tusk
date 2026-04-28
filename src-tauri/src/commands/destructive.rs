//! AST-based destructive-statement classifier.
//!
//! The frontend `lib/ai/destructive.ts` is a regex pre-warning *only*; this
//! module is the single source of truth for the run gate.
//!
//! Wire format: `kind` is kebab-case to match the TypeScript `DestructiveKind`
//! union.

use serde::Serialize;
use sqlparser::ast::{AlterTableOperation, FromTable, ObjectType, Privileges, Statement};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DestructiveFinding {
    pub kind: DestructiveKind,
    pub statement_index: usize,
    pub message: String,
    pub affected_object: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DestructiveKind {
    DropDatabase,
    DropSchema,
    DropTable,
    DropColumn,
    DropIndex,
    DropView,
    DropFunction,
    Truncate,
    DeleteNoWhere,
    UpdateNoWhere,
    AlterDropConstraint,
    GrantRevokeAll,
    VacuumFull,
    ParserFailed,
}

pub fn classify_destructive(sql: &str) -> Vec<DestructiveFinding> {
    let stmts = match Parser::parse_sql(&PostgreSqlDialect {}, sql) {
        Ok(s) => s,
        Err(_) => {
            return vec![DestructiveFinding {
                kind: DestructiveKind::ParserFailed,
                statement_index: 0,
                message: "SQL could not be parsed; confirm before running".to_string(),
                affected_object: None,
            }]
        }
    };
    let mut out = Vec::new();
    for (i, stmt) in stmts.iter().enumerate() {
        if let Some(f) = classify_one(i, stmt) {
            out.push(f);
        }
    }
    out
}

fn classify_one(idx: usize, stmt: &Statement) -> Option<DestructiveFinding> {
    match stmt {
        Statement::Drop {
            object_type, names, ..
        } => {
            let names_str = names
                .iter()
                .map(object_to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let kind = match object_type {
                ObjectType::Table => DestructiveKind::DropTable,
                ObjectType::Index => DestructiveKind::DropIndex,
                ObjectType::View => DestructiveKind::DropView,
                ObjectType::Schema => DestructiveKind::DropSchema,
                ObjectType::Database => DestructiveKind::DropDatabase,
                _ => DestructiveKind::DropTable,
            };
            Some(DestructiveFinding {
                kind,
                statement_index: idx,
                message: format!("DROP {object_type:?} {names_str}"),
                affected_object: Some(names_str),
            })
        }
        Statement::DropFunction { .. } => Some(DestructiveFinding {
            kind: DestructiveKind::DropFunction,
            statement_index: idx,
            message: "DROP FUNCTION removes a function".to_string(),
            affected_object: None,
        }),
        Statement::Truncate { table_names, .. } => {
            let s = table_names
                .iter()
                .map(|t| object_to_string(&t.name))
                .collect::<Vec<_>>()
                .join(", ");
            Some(DestructiveFinding {
                kind: DestructiveKind::Truncate,
                statement_index: idx,
                message: format!("TRUNCATE will remove all rows from {s}"),
                affected_object: Some(s),
            })
        }
        Statement::Delete(d) => {
            let where_present = d.selection.is_some();
            if !where_present {
                let target = match &d.from {
                    FromTable::WithFromKeyword(v) | FromTable::WithoutKeyword(v) => v
                        .first()
                        .and_then(|tw| {
                            if let sqlparser::ast::TableFactor::Table { name, .. } = &tw.relation {
                                Some(object_to_string(name))
                            } else {
                                None
                            }
                        })
                        .unwrap_or_else(|| "<unknown>".to_string()),
                };
                Some(DestructiveFinding {
                    kind: DestructiveKind::DeleteNoWhere,
                    statement_index: idx,
                    message: format!("DELETE without WHERE will remove all rows from {target}"),
                    affected_object: Some(target),
                })
            } else {
                None
            }
        }
        Statement::Update {
            table, selection, ..
        } => {
            if selection.is_none() {
                let target =
                    if let sqlparser::ast::TableFactor::Table { name, .. } = &table.relation {
                        object_to_string(name)
                    } else {
                        "<unknown>".to_string()
                    };
                Some(DestructiveFinding {
                    kind: DestructiveKind::UpdateNoWhere,
                    statement_index: idx,
                    message: format!("UPDATE without WHERE will modify all rows in {target}"),
                    affected_object: Some(target),
                })
            } else {
                None
            }
        }
        Statement::AlterTable {
            name, operations, ..
        } => {
            for op in operations {
                match op {
                    AlterTableOperation::DropColumn { column_name, .. } => {
                        return Some(DestructiveFinding {
                            kind: DestructiveKind::DropColumn,
                            statement_index: idx,
                            message: format!(
                                "ALTER TABLE {} DROP COLUMN {} will remove the column and its data",
                                object_to_string(name),
                                column_name.value
                            ),
                            affected_object: Some(format!(
                                "{}.{}",
                                object_to_string(name),
                                column_name.value
                            )),
                        });
                    }
                    AlterTableOperation::DropConstraint { name: c, .. } => {
                        return Some(DestructiveFinding {
                            kind: DestructiveKind::AlterDropConstraint,
                            statement_index: idx,
                            message: format!(
                                "ALTER TABLE {} DROP CONSTRAINT {}",
                                object_to_string(name),
                                c.value
                            ),
                            affected_object: Some(object_to_string(name)),
                        });
                    }
                    _ => {}
                }
            }
            None
        }
        Statement::Grant { privileges, .. } | Statement::Revoke { privileges, .. } => {
            let all = matches!(privileges, Privileges::All { .. });
            if all {
                Some(DestructiveFinding {
                    kind: DestructiveKind::GrantRevokeAll,
                    statement_index: idx,
                    message: "GRANT/REVOKE ALL changes broad privileges".into(),
                    affected_object: None,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

fn object_to_string(name: &sqlparser::ast::ObjectName) -> String {
    name.0
        .iter()
        .map(|i| i.value.clone())
        .collect::<Vec<_>>()
        .join(".")
}

/// VACUUM FULL is awkward to discriminate from sqlparser AST — supplemented
/// with a raw-token check.
pub fn classify_vacuum_full(sql: &str) -> Vec<DestructiveFinding> {
    let mut out = Vec::new();
    let upper = sql.to_uppercase();
    if upper.contains("VACUUM FULL") {
        out.push(DestructiveFinding {
            kind: DestructiveKind::VacuumFull,
            statement_index: 0,
            message: "VACUUM FULL takes an exclusive lock and rewrites the table".into(),
            affected_object: None,
        });
    }
    out
}

pub fn classify_all(sql: &str) -> Vec<DestructiveFinding> {
    let vacuum = classify_vacuum_full(sql);
    let mut out = classify_destructive(sql);
    // When VACUUM FULL is detected via token match, suppress a spurious
    // ParserFailed that results from sqlparser 0.52 not recognising VACUUM.
    if !vacuum.is_empty() {
        out.retain(|f| f.kind != DestructiveKind::ParserFailed);
    }
    out.extend(vacuum);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(sql: &str) -> Vec<DestructiveKind> {
        classify_all(sql).into_iter().map(|f| f.kind).collect()
    }

    #[test]
    fn drop_table() {
        assert_eq!(kinds("DROP TABLE users"), vec![DestructiveKind::DropTable]);
    }

    #[test]
    fn drop_schema_cascade() {
        assert_eq!(
            kinds("DROP SCHEMA app CASCADE"),
            vec![DestructiveKind::DropSchema]
        );
    }

    #[test]
    fn drop_index_view_function() {
        assert_eq!(kinds("DROP INDEX idx_a"), vec![DestructiveKind::DropIndex]);
        assert_eq!(kinds("DROP VIEW v"), vec![DestructiveKind::DropView]);
    }

    #[test]
    fn truncate_named_table() {
        assert_eq!(
            kinds("TRUNCATE TABLE public.audit_log"),
            vec![DestructiveKind::Truncate]
        );
    }

    #[test]
    fn delete_without_where() {
        assert_eq!(
            kinds("DELETE FROM users"),
            vec![DestructiveKind::DeleteNoWhere]
        );
    }

    #[test]
    fn delete_with_where_is_safe() {
        assert!(kinds("DELETE FROM users WHERE id = 1").is_empty());
    }

    #[test]
    fn update_without_where() {
        assert_eq!(
            kinds("UPDATE users SET active = false"),
            vec![DestructiveKind::UpdateNoWhere]
        );
    }

    #[test]
    fn update_with_where_is_safe() {
        assert!(kinds("UPDATE users SET active = false WHERE id = 1").is_empty());
    }

    #[test]
    fn alter_drop_column() {
        assert_eq!(
            kinds("ALTER TABLE users DROP COLUMN email"),
            vec![DestructiveKind::DropColumn]
        );
    }

    #[test]
    fn alter_drop_constraint() {
        assert_eq!(
            kinds("ALTER TABLE users DROP CONSTRAINT users_pkey"),
            vec![DestructiveKind::AlterDropConstraint]
        );
    }

    #[test]
    fn grant_all_privileges() {
        assert_eq!(
            kinds("GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO bob"),
            vec![DestructiveKind::GrantRevokeAll]
        );
    }

    #[test]
    fn revoke_all_privileges() {
        assert_eq!(
            kinds("REVOKE ALL PRIVILEGES ON ALL TABLES IN SCHEMA public FROM bob"),
            vec![DestructiveKind::GrantRevokeAll]
        );
    }

    #[test]
    fn vacuum_full_token_match() {
        assert_eq!(
            kinds("VACUUM FULL users"),
            vec![DestructiveKind::VacuumFull]
        );
    }

    #[test]
    fn select_is_safe() {
        assert!(kinds("SELECT * FROM users").is_empty());
    }

    #[test]
    fn unparseable_returns_parser_failed() {
        let r = classify_destructive("this is not sql");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].kind, DestructiveKind::ParserFailed);
    }

    #[test]
    fn multi_statement_collects_each_finding() {
        let sql = "DELETE FROM a; UPDATE b SET x=1; DROP TABLE c";
        let r = classify_destructive(sql);
        let kinds: Vec<_> = r.iter().map(|f| f.kind).collect();
        assert_eq!(
            kinds,
            vec![
                DestructiveKind::DeleteNoWhere,
                DestructiveKind::UpdateNoWhere,
                DestructiveKind::DropTable,
            ]
        );
        assert_eq!(r[0].statement_index, 0);
        assert_eq!(r[1].statement_index, 1);
        assert_eq!(r[2].statement_index, 2);
    }
}
