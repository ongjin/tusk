// src-tauri/src/db/pg_literals.rs
//
// Renders typed values as PG literal SQL fragments (single-quoted strings,
// hex bytea, NULL, etc.) — used to build human-readable preview SQL that
// matches what the parameterized executor will actually run. Implementation
// lands in Task 2.

use crate::db::decoder::Cell;
use std::fmt::Write;

/// Renders a typed Cell as a PG literal SQL fragment.
///
/// Examples:
///   Null              → "NULL"
///   Bool(true)        → "TRUE"
///   Int(42)           → "42"
///   Numeric("1.234")  → "1.234"
///   Text("o'r")       → "'o''r'"
///   Bytea(b64="...")  → "'\\x<hex>'"
///   Uuid              → "'<uuid>'::uuid"
///   Timestamptz(iso)  → "'<iso>'::timestamptz"
///   Json              → "'{...}'::jsonb"
///   Array(elem, vals) → "ARRAY[...]::<elem>[]"
pub fn to_literal(cell: &Cell) -> String {
    match cell {
        Cell::Null => "NULL".to_string(),
        Cell::Bool(true) => "TRUE".to_string(),
        Cell::Bool(false) => "FALSE".to_string(),
        Cell::Int(v) => v.to_string(),
        Cell::Bigint(s) => s.clone(),
        Cell::Float(v) => format_float(*v),
        Cell::Numeric(s) => s.clone(),
        Cell::Text(s) => quote_string(s),
        Cell::Bytea { b64 } => quote_bytea(b64),
        Cell::Uuid(s) => format!("{}::uuid", quote_string(s)),
        Cell::Inet(s) => format!("{}::inet", quote_string(s)),
        Cell::Date(s) => format!("{}::date", quote_string(s)),
        Cell::Time(s) => format!("{}::time", quote_string(s)),
        Cell::Timetz(s) => format!("{}::timetz", quote_string(s)),
        Cell::Timestamp(s) => format!("{}::timestamp", quote_string(s)),
        Cell::Timestamptz(s) => format!("{}::timestamptz", quote_string(s)),
        Cell::Interval { iso } => format!("{}::interval", quote_string(iso)),
        Cell::Json(v) => {
            let raw = serde_json::to_string(v).expect("json round-trip");
            format!("{}::jsonb", quote_string(&raw))
        }
        Cell::Array { elem, values } => {
            let mut out = String::from("ARRAY[");
            for (i, v) in values.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&to_literal(v));
            }
            let _ = write!(out, "]::{}[]", elem);
            out
        }
        Cell::Enum { type_name, value } => format!("{}::{}", quote_string(value), type_name),
        Cell::Vector { values, .. } => {
            let mut inner = String::from("[");
            for (i, v) in values.iter().enumerate() {
                if i > 0 {
                    inner.push(',');
                }
                inner.push_str(&format_float(*v as f64));
            }
            inner.push(']');
            format!("{}::vector", quote_string(&inner))
        }
        Cell::Unknown { text, .. } => format!("{}::text", quote_string(text)),
    }
}

fn quote_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push('\'');
            out.push('\'');
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

fn quote_bytea(b64: &str) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let bytes = STANDARD.decode(b64).unwrap_or_default();
    let mut hex = String::with_capacity(bytes.len() * 2 + 4);
    hex.push_str("'\\x");
    for b in bytes {
        let _ = write!(hex, "{b:02x}");
    }
    hex.push('\'');
    hex.push_str("::bytea");
    hex
}

fn format_float(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() && v.abs() < 1e16 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn null_renders_uppercase_keyword() {
        assert_eq!(to_literal(&Cell::Null), "NULL");
    }

    #[test]
    fn bool_renders_uppercase_keyword() {
        assert_eq!(to_literal(&Cell::Bool(true)), "TRUE");
        assert_eq!(to_literal(&Cell::Bool(false)), "FALSE");
    }

    #[test]
    fn int_renders_decimal() {
        assert_eq!(to_literal(&Cell::Int(42)), "42");
        assert_eq!(to_literal(&Cell::Int(-7)), "-7");
    }

    #[test]
    fn bigint_preserves_string() {
        assert_eq!(
            to_literal(&Cell::Bigint("9223372036854775807".into())),
            "9223372036854775807"
        );
    }

    #[test]
    fn float_uses_pg_compatible_repr() {
        assert_eq!(to_literal(&Cell::Float(1.5)), "1.5");
        assert_eq!(to_literal(&Cell::Float(-0.25)), "-0.25");
    }

    #[test]
    fn numeric_passes_through_string() {
        assert_eq!(to_literal(&Cell::Numeric("1.234".into())), "1.234");
    }

    #[test]
    fn text_quotes_and_doubles_single_quotes() {
        assert_eq!(to_literal(&Cell::Text("o'reilly".into())), "'o''reilly'");
        assert_eq!(to_literal(&Cell::Text("plain".into())), "'plain'");
    }

    #[test]
    fn text_with_backslash_uses_e_string() {
        assert_eq!(to_literal(&Cell::Text("a\\b".into())), "'a\\b'");
    }

    #[test]
    fn bytea_emits_hex_form() {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let b64 = STANDARD.encode([0xDE_u8, 0xAD, 0xBE, 0xEF]);
        let cell = Cell::Bytea { b64 };
        assert_eq!(to_literal(&cell), "'\\xdeadbeef'::bytea");
    }

    #[test]
    fn uuid_appends_cast() {
        assert_eq!(
            to_literal(&Cell::Uuid("550e8400-e29b-41d4-a716-446655440000".into())),
            "'550e8400-e29b-41d4-a716-446655440000'::uuid"
        );
    }

    #[test]
    fn inet_appends_cast() {
        assert_eq!(
            to_literal(&Cell::Inet("10.0.0.1/32".into())),
            "'10.0.0.1/32'::inet"
        );
    }

    #[test]
    fn timestamps_append_typed_cast() {
        assert_eq!(
            to_literal(&Cell::Timestamptz("2026-04-28T12:00:00+00:00".into())),
            "'2026-04-28T12:00:00+00:00'::timestamptz"
        );
        assert_eq!(
            to_literal(&Cell::Timestamp("2026-04-28T12:00:00".into())),
            "'2026-04-28T12:00:00'::timestamp"
        );
        assert_eq!(
            to_literal(&Cell::Date("2026-04-28".into())),
            "'2026-04-28'::date"
        );
        assert_eq!(
            to_literal(&Cell::Time("12:00:00".into())),
            "'12:00:00'::time"
        );
        assert_eq!(
            to_literal(&Cell::Timetz("12:00:00+00".into())),
            "'12:00:00+00'::timetz"
        );
    }

    #[test]
    fn interval_appends_cast() {
        let cell = Cell::Interval {
            iso: "PT1H30M".into(),
        };
        assert_eq!(to_literal(&cell), "'PT1H30M'::interval");
    }

    #[test]
    fn json_quotes_and_escapes() {
        let cell = Cell::Json(json!({ "k": "v's" }));
        assert_eq!(to_literal(&cell), r#"'{"k":"v''s"}'::jsonb"#);
    }

    #[test]
    fn enum_uses_text_cast_to_typename() {
        let cell = Cell::Enum {
            type_name: "mood".into(),
            value: "happy".into(),
        };
        assert_eq!(to_literal(&cell), "'happy'::mood");
    }

    #[test]
    fn vector_renders_brackets() {
        let cell = Cell::Vector {
            dim: 3,
            values: vec![1.0, 2.5, -3.0],
        };
        assert_eq!(to_literal(&cell), "'[1,2.5,-3]'::vector");
    }

    #[test]
    fn array_of_ints_renders_as_array_literal() {
        let cell = Cell::Array {
            elem: "int4".into(),
            values: vec![Cell::Int(1), Cell::Int(2), Cell::Null, Cell::Int(4)],
        };
        assert_eq!(to_literal(&cell), "ARRAY[1,2,NULL,4]::int4[]");
    }

    #[test]
    fn array_of_text_quotes_each_element() {
        let cell = Cell::Array {
            elem: "text".into(),
            values: vec![Cell::Text("a".into()), Cell::Text("o'r".into())],
        };
        assert_eq!(to_literal(&cell), "ARRAY['a','o''r']::text[]");
    }

    #[test]
    fn unknown_uses_text_repr_quoted() {
        let cell = Cell::Unknown {
            oid: 9999,
            text: "raw".into(),
        };
        assert_eq!(to_literal(&cell), "'raw'::text");
    }
}
