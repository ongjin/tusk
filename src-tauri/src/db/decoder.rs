// src-tauri/src/db/decoder.rs
//
// OID-dispatch typed decoder. Replaces the best-effort `decode_cell` from
// commands/query.rs (Week 2). Every PG type the spec lists maps to an exact
// `Cell` variant; unknown OIDs fall through to `Cell::Unknown { oid, text }`
// with a best-effort utf8 of the raw bytes.

use serde::Serialize;
use sqlx::postgres::PgRow;
use sqlx::{Column, Row, ValueRef};

#[derive(Debug, Serialize, Clone)]
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

/// PG built-in OIDs we recognize. Source: pg_type.h in postgres 16.
pub mod oids {
    pub const BOOL: u32 = 16;
    pub const BYTEA: u32 = 17;
    pub const INT8: u32 = 20;
    pub const INT2: u32 = 21;
    pub const INT4: u32 = 23;
    pub const TEXT: u32 = 25;
    pub const JSON: u32 = 114;
    pub const FLOAT4: u32 = 700;
    pub const FLOAT8: u32 = 701;
    pub const VARCHAR: u32 = 1043;
    pub const BPCHAR: u32 = 1042;
    pub const DATE: u32 = 1082;
    pub const TIME: u32 = 1083;
    pub const TIMESTAMP: u32 = 1114;
    pub const TIMESTAMPTZ: u32 = 1184;
    pub const INTERVAL: u32 = 1186;
    pub const TIMETZ: u32 = 1266;
    pub const NUMERIC: u32 = 1700;
    pub const UUID: u32 = 2950;
    pub const JSONB: u32 = 3802;
    pub const INET: u32 = 869;
    pub const CIDR: u32 = 650;
    pub const _BOOL: u32 = 1000;
    pub const _BYTEA: u32 = 1001;
    pub const _INT2: u32 = 1005;
    pub const _INT4: u32 = 1007;
    pub const _INT8: u32 = 1016;
    pub const _TEXT: u32 = 1009;
    pub const _VARCHAR: u32 = 1015;
    pub const _NUMERIC: u32 = 1231;
    pub const _UUID: u32 = 2951;
    pub const _FLOAT4: u32 = 1021;
    pub const _FLOAT8: u32 = 1022;
    pub const _TIMESTAMP: u32 = 1115;
    pub const _TIMESTAMPTZ: u32 = 1185;
    pub const _DATE: u32 = 1182;
}

pub fn pg_type_name(oid: u32) -> &'static str {
    match oid {
        oids::BOOL => "bool",
        oids::INT2 => "int2",
        oids::INT4 => "int4",
        oids::INT8 => "int8",
        oids::FLOAT4 => "float4",
        oids::FLOAT8 => "float8",
        oids::NUMERIC => "numeric",
        oids::TEXT => "text",
        oids::VARCHAR => "varchar",
        oids::BPCHAR => "bpchar",
        oids::BYTEA => "bytea",
        oids::UUID => "uuid",
        oids::INET => "inet",
        oids::CIDR => "cidr",
        oids::DATE => "date",
        oids::TIME => "time",
        oids::TIMETZ => "timetz",
        oids::TIMESTAMP => "timestamp",
        oids::TIMESTAMPTZ => "timestamptz",
        oids::INTERVAL => "interval",
        oids::JSON => "json",
        oids::JSONB => "jsonb",
        _ => "unknown",
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMeta {
    pub name: String,
    pub oid: u32,
    pub type_name: String,
}

pub fn columns_of(row: &PgRow) -> Vec<ColumnMeta> {
    row.columns()
        .iter()
        .map(|c| {
            let type_info = c.type_info();
            let oid = type_info.oid().map(|o| o.0).unwrap_or(0);
            ColumnMeta {
                name: c.name().to_string(),
                oid,
                type_name: pg_type_name(oid).to_string(),
            }
        })
        .collect()
}

pub fn decode_row(row: &PgRow, columns: &[ColumnMeta]) -> Vec<Cell> {
    (0..columns.len())
        .map(|i| decode_cell(row, i, columns[i].oid))
        .collect()
}

fn decode_cell(row: &PgRow, idx: usize, oid: u32) -> Cell {
    if let Ok(raw) = row.try_get_raw(idx) {
        if raw.is_null() {
            return Cell::Null;
        }
    }
    match oid {
        oids::BOOL => row
            .try_get::<bool, _>(idx)
            .map(Cell::Bool)
            .unwrap_or(Cell::Null),
        oids::INT2 => row
            .try_get::<i16, _>(idx)
            .map(|v| Cell::Int(i32::from(v)))
            .unwrap_or(Cell::Null),
        oids::INT4 => row
            .try_get::<i32, _>(idx)
            .map(Cell::Int)
            .unwrap_or(Cell::Null),
        oids::INT8 => row
            .try_get::<i64, _>(idx)
            .map(|v| Cell::Bigint(v.to_string()))
            .unwrap_or(Cell::Null),
        oids::FLOAT4 => row
            .try_get::<f32, _>(idx)
            .map(|v| Cell::Float(f64::from(v)))
            .unwrap_or(Cell::Null),
        oids::FLOAT8 => row
            .try_get::<f64, _>(idx)
            .map(Cell::Float)
            .unwrap_or(Cell::Null),
        oids::NUMERIC => row
            .try_get::<bigdecimal::BigDecimal, _>(idx)
            .map(|v| Cell::Numeric(v.normalized().to_string()))
            .unwrap_or(Cell::Null),
        oids::TEXT | oids::VARCHAR | oids::BPCHAR => row
            .try_get::<String, _>(idx)
            .map(Cell::Text)
            .unwrap_or(Cell::Null),
        oids::BYTEA => row
            .try_get::<Vec<u8>, _>(idx)
            .map(|bytes| {
                use base64::{engine::general_purpose::STANDARD, Engine};
                Cell::Bytea {
                    b64: STANDARD.encode(bytes),
                }
            })
            .unwrap_or(Cell::Null),
        oids::UUID => row
            .try_get::<uuid::Uuid, _>(idx)
            .map(|u| Cell::Uuid(u.to_string()))
            .unwrap_or(Cell::Null),
        oids::INET | oids::CIDR => row
            .try_get::<ipnetwork::IpNetwork, _>(idx)
            .map(|n| Cell::Inet(n.to_string()))
            .unwrap_or(Cell::Null),
        oids::DATE => row
            .try_get::<chrono::NaiveDate, _>(idx)
            .map(|d| Cell::Date(d.to_string()))
            .unwrap_or(Cell::Null),
        oids::TIME => row
            .try_get::<chrono::NaiveTime, _>(idx)
            .map(|t| Cell::Time(t.to_string()))
            .unwrap_or(Cell::Null),
        oids::TIMESTAMP => row
            .try_get::<chrono::NaiveDateTime, _>(idx)
            .map(|t| Cell::Timestamp(t.to_string()))
            .unwrap_or(Cell::Null),
        oids::TIMESTAMPTZ => row
            .try_get::<chrono::DateTime<chrono::Utc>, _>(idx)
            .map(|t| Cell::Timestamptz(t.to_rfc3339()))
            .unwrap_or(Cell::Null),
        oids::INTERVAL => row
            .try_get::<sqlx::postgres::types::PgInterval, _>(idx)
            .map(|iv| Cell::Interval {
                iso: pg_interval_to_iso(&iv),
            })
            .unwrap_or(Cell::Null),
        oids::TIMETZ => row
            .try_get::<sqlx::postgres::types::PgTimeTz, _>(idx)
            .map(|t| Cell::Timetz(format!("{}", t.time)))
            .unwrap_or(Cell::Null),
        oids::JSON | oids::JSONB => row
            .try_get::<serde_json::Value, _>(idx)
            .map(Cell::Json)
            .unwrap_or(Cell::Null),
        oids::_INT4 => decode_int_array(row, idx, "int4"),
        oids::_INT8 => decode_bigint_array(row, idx, "int8"),
        oids::_TEXT | oids::_VARCHAR => decode_text_array(row, idx, "text"),
        oids::_BOOL => decode_bool_array(row, idx, "bool"),
        _ => unknown_fallback(row, idx, oid),
    }
}

fn unknown_fallback(row: &PgRow, idx: usize, oid: u32) -> Cell {
    if let Ok(raw) = row.try_get_raw(idx) {
        if let Ok(bytes) = raw.as_bytes() {
            if let Ok(text) = std::str::from_utf8(bytes) {
                return Cell::Unknown {
                    oid,
                    text: text.to_string(),
                };
            }
        }
    }
    Cell::Unknown {
        oid,
        text: String::new(),
    }
}

fn pg_interval_to_iso(iv: &sqlx::postgres::types::PgInterval) -> String {
    let years = iv.months / 12;
    let months = iv.months % 12;
    let total_micros = iv.microseconds;
    let hours = total_micros / 3_600_000_000;
    let rem = total_micros % 3_600_000_000;
    let minutes = rem / 60_000_000;
    let secs_micros = rem % 60_000_000;
    let secs = secs_micros / 1_000_000;
    let frac = secs_micros % 1_000_000;
    let mut iso = String::from("P");
    if years != 0 {
        iso.push_str(&format!("{years}Y"));
    }
    if months != 0 {
        iso.push_str(&format!("{months}M"));
    }
    if iv.days != 0 {
        iso.push_str(&format!("{}D", iv.days));
    }
    if hours != 0 || minutes != 0 || secs != 0 || frac != 0 {
        iso.push('T');
        if hours != 0 {
            iso.push_str(&format!("{hours}H"));
        }
        if minutes != 0 {
            iso.push_str(&format!("{minutes}M"));
        }
        if secs != 0 || frac != 0 {
            if frac != 0 {
                iso.push_str(&format!("{secs}.{frac:06}S"));
            } else {
                iso.push_str(&format!("{secs}S"));
            }
        }
    }
    if iso == "P" {
        iso.push_str("T0S");
    }
    iso
}

fn decode_int_array(row: &PgRow, idx: usize, elem: &str) -> Cell {
    match row.try_get::<Vec<Option<i32>>, _>(idx) {
        Ok(vec) => Cell::Array {
            elem: elem.to_string(),
            values: vec
                .into_iter()
                .map(|o| o.map(Cell::Int).unwrap_or(Cell::Null))
                .collect(),
        },
        Err(_) => Cell::Null,
    }
}
fn decode_bigint_array(row: &PgRow, idx: usize, elem: &str) -> Cell {
    match row.try_get::<Vec<Option<i64>>, _>(idx) {
        Ok(vec) => Cell::Array {
            elem: elem.to_string(),
            values: vec
                .into_iter()
                .map(|o| o.map(|v| Cell::Bigint(v.to_string())).unwrap_or(Cell::Null))
                .collect(),
        },
        Err(_) => Cell::Null,
    }
}
fn decode_text_array(row: &PgRow, idx: usize, elem: &str) -> Cell {
    match row.try_get::<Vec<Option<String>>, _>(idx) {
        Ok(vec) => Cell::Array {
            elem: elem.to_string(),
            values: vec
                .into_iter()
                .map(|o| o.map(Cell::Text).unwrap_or(Cell::Null))
                .collect(),
        },
        Err(_) => Cell::Null,
    }
}
fn decode_bool_array(row: &PgRow, idx: usize, elem: &str) -> Cell {
    match row.try_get::<Vec<Option<bool>>, _>(idx) {
        Ok(vec) => Cell::Array {
            elem: elem.to_string(),
            values: vec
                .into_iter()
                .map(|o| o.map(Cell::Bool).unwrap_or(Cell::Null))
                .collect(),
        },
        Err(_) => Cell::Null,
    }
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

    #[test]
    fn known_oids_map_to_canonical_names() {
        assert_eq!(pg_type_name(oids::BOOL), "bool");
        assert_eq!(pg_type_name(oids::INT4), "int4");
        assert_eq!(pg_type_name(oids::TIMESTAMPTZ), "timestamptz");
        assert_eq!(pg_type_name(oids::JSONB), "jsonb");
    }

    #[test]
    fn unknown_oid_returns_unknown() {
        assert_eq!(pg_type_name(99999), "unknown");
    }
}
