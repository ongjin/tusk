// src-tauri/src/db/decoder.rs
//
// OID-dispatch typed decoder. Replaces the best-effort `decode_cell` in
// commands/query.rs (Week 2). Implementation lands in Task 3.

use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum Cell {
    Null,
    Bool(bool),
    Int(i32),
    Bigint(String),
    Float(f64),
    Numeric(String),
    Text(String),
    Bytea { b64: String },
    Uuid(String),
    Inet(String),
    Date(String),
    Time(String),
    Timetz(String),
    Timestamp(String),
    Timestamptz(String),
    Interval { iso: String },
    Json(serde_json::Value),
    Array { elem: String, values: Vec<Cell> },
    Enum { type_name: String, value: String },
    Vector { dim: u32, values: Vec<f32> },
    Unknown { oid: u32, text: String },
}
