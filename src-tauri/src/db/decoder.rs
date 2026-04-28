// src-tauri/src/db/decoder.rs
//
// OID-dispatch typed decoder. Replaces the best-effort `decode_cell` in
// commands/query.rs (Week 2). Implementation lands in Task 3.

use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "value")]
pub enum Cell {
    Null,
    Bool(bool),
    Int(i32),
    Bigint(String),
    Float(f64),
    Numeric(String),
    Text(String),
    Bytea {
        b64: String,
    },
    Uuid(String),
    Inet(String),
    Date(String),
    Time(String),
    Timetz(String),
    Timestamp(String),
    Timestamptz(String),
    Interval {
        iso: String,
    },
    Json(serde_json::Value),
    Array {
        elem: String,
        values: Vec<Cell>,
    },
    Enum {
        #[serde(rename = "typeName")]
        type_name: String,
        value: String,
    },
    Vector {
        dim: u32,
        values: Vec<f32>,
    },
    Unknown {
        oid: u32,
        text: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_serializes_with_pascal_case_kind() {
        // Tuple variant
        let c = Cell::Bool(true);
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(json, r#"{"kind":"Bool","value":true}"#);

        // Tuple variant with String
        let c = Cell::Bigint("123".into());
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(json, r#"{"kind":"Bigint","value":"123"}"#);

        // Unit variant
        let c = Cell::Null;
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(json, r#"{"kind":"Null"}"#);

        // Struct variant
        let c = Cell::Bytea { b64: "AA==".into() };
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(json, r#"{"kind":"Bytea","value":{"b64":"AA=="}}"#);

        // Struct variant with snake_case field — serde keeps field name as-is
        // since we removed rename_all on the enum.
        let c = Cell::Enum {
            type_name: "mood".into(),
            value: "happy".into(),
        };
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(
            json,
            r#"{"kind":"Enum","value":{"typeName":"mood","value":"happy"}}"#
        );
    }
}
