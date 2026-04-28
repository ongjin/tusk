// src-tauri/src/commands/export.rs
//
// Export the rows of a query result to CSV, JSON, or SQL INSERT statements.
// Writes the file directly from the backend at a user-chosen path; the frontend
// shows the OS save dialog and forwards the selected path here.
//
// CSV: optional UTF-8 BOM, header row, RFC-4180 escaping for fields that
//      contain comma / quote / newline.
// JSON: array of objects keyed by column name. NULL becomes JSON null.
// SQL INSERT: requires a fully-qualified, already-quoted table identifier;
//             reuses `pg_literals::to_literal` so the output matches what
//             the parameterized executor would have rendered.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use serde::Deserialize;

use crate::db::decoder::Cell;
use crate::db::pg_literals::to_literal;
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    Csv,
    Json,
    SqlInsert,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub format: ExportFormat,
    pub path: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
    #[serde(default)]
    pub include_bom: bool,
    /// For SQL INSERT: a quoted, qualified table identifier such as
    /// `"public"."users"`. Required when `format == SqlInsert`.
    #[serde(default)]
    pub table: Option<String>,
}

#[tauri::command]
pub fn export_result(req: ExportRequest) -> TuskResult<()> {
    let ExportRequest {
        format,
        path,
        columns,
        rows,
        include_bom,
        table,
    } = req;
    let path = PathBuf::from(&path);
    let file = File::create(&path)
        .map_err(|e| TuskError::Internal(format!("create {}: {e}", path.display())))?;
    let mut out = BufWriter::new(file);

    match format {
        ExportFormat::Csv => write_csv(&mut out, &columns, &rows, include_bom)?,
        ExportFormat::Json => write_json(&mut out, &columns, &rows)?,
        ExportFormat::SqlInsert => {
            let table = table.ok_or_else(|| {
                TuskError::Internal("SQL INSERT export requires a table identifier".into())
            })?;
            write_sql_inserts(&mut out, &table, &columns, &rows)?;
        }
    }

    out.flush()
        .map_err(|e| TuskError::Internal(format!("flush: {e}")))?;
    Ok(())
}

fn write_csv<W: Write>(
    out: &mut W,
    columns: &[String],
    rows: &[Vec<Cell>],
    include_bom: bool,
) -> TuskResult<()> {
    if include_bom {
        out.write_all(&[0xEF, 0xBB, 0xBF])
            .map_err(|e| TuskError::Internal(format!("write bom: {e}")))?;
    }
    // Header.
    let header = columns
        .iter()
        .map(|c| csv_escape(c))
        .collect::<Vec<_>>()
        .join(",");
    writeln!(out, "{header}").map_err(|e| TuskError::Internal(format!("write header: {e}")))?;
    // Rows.
    for row in rows {
        let line = row
            .iter()
            .map(|c| csv_escape(&cell_to_csv(c)))
            .collect::<Vec<_>>()
            .join(",");
        writeln!(out, "{line}").map_err(|e| TuskError::Internal(format!("write row: {e}")))?;
    }
    Ok(())
}

fn write_json<W: Write>(out: &mut W, columns: &[String], rows: &[Vec<Cell>]) -> TuskResult<()> {
    let mut arr: Vec<serde_json::Map<String, serde_json::Value>> = Vec::with_capacity(rows.len());
    for row in rows {
        let mut obj = serde_json::Map::with_capacity(columns.len());
        for (i, name) in columns.iter().enumerate() {
            let cell = row.get(i).cloned().unwrap_or(Cell::Null);
            obj.insert(name.clone(), cell_to_json(&cell));
        }
        arr.push(obj);
    }
    let buf = serde_json::to_vec_pretty(&arr)
        .map_err(|e| TuskError::Internal(format!("encode json: {e}")))?;
    out.write_all(&buf)
        .map_err(|e| TuskError::Internal(format!("write json: {e}")))?;
    Ok(())
}

fn write_sql_inserts<W: Write>(
    out: &mut W,
    table: &str,
    columns: &[String],
    rows: &[Vec<Cell>],
) -> TuskResult<()> {
    let col_list = columns
        .iter()
        .map(|c| quote_ident(c))
        .collect::<Vec<_>>()
        .join(", ");
    for row in rows {
        let values = row.iter().map(to_literal).collect::<Vec<_>>().join(", ");
        writeln!(out, "INSERT INTO {table} ({col_list}) VALUES ({values});")
            .map_err(|e| TuskError::Internal(format!("write insert: {e}")))?;
    }
    Ok(())
}

/// Render a `Cell` as the textual form used in a CSV field (before escaping).
/// NULL becomes the empty string. Bytea is hex-prefixed (`\x...`) so it
/// round-trips when imported into a `bytea` column.
fn cell_to_csv(cell: &Cell) -> String {
    match cell {
        Cell::Null => String::new(),
        Cell::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        Cell::Int(v) => v.to_string(),
        Cell::Bigint(s) => s.clone(),
        Cell::Float(v) => {
            if v.is_nan() {
                "NaN".to_string()
            } else if v.is_infinite() {
                if v.is_sign_negative() {
                    "-Infinity".to_string()
                } else {
                    "Infinity".to_string()
                }
            } else {
                v.to_string()
            }
        }
        Cell::Numeric(s) => s.clone(),
        Cell::Text(s) => s.clone(),
        Cell::Bytea { b64 } => {
            use base64::{engine::general_purpose::STANDARD, Engine};
            let bytes = STANDARD.decode(b64).unwrap_or_default();
            let mut out = String::with_capacity(bytes.len() * 2 + 2);
            out.push_str("\\x");
            for b in bytes {
                use std::fmt::Write as _;
                let _ = write!(out, "{b:02x}");
            }
            out
        }
        Cell::Uuid(s) => s.clone(),
        Cell::Inet(s) => s.clone(),
        Cell::Date(s) => s.clone(),
        Cell::Time(s) => s.clone(),
        Cell::Timetz(s) => s.clone(),
        Cell::Timestamp(s) => s.clone(),
        Cell::Timestamptz(s) => s.clone(),
        Cell::Interval { iso } => iso.clone(),
        Cell::Json(v) => serde_json::to_string(v).unwrap_or_default(),
        Cell::Array { values, .. } => {
            let inner = values.iter().map(cell_to_csv).collect::<Vec<_>>().join(",");
            format!("{{{inner}}}")
        }
        Cell::Enum { value, .. } => value.clone(),
        Cell::Vector { values, .. } => {
            let inner = values
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(",");
            format!("[{inner}]")
        }
        Cell::Unknown { text, .. } => text.clone(),
    }
}

/// Render a `Cell` as a JSON value. Strings are JSON strings; numbers become
/// JSON numbers when they are exactly representable. Anything ambiguous
/// (bigint, numeric, bytea, etc.) becomes a string so we don't lose precision.
fn cell_to_json(cell: &Cell) -> serde_json::Value {
    use serde_json::Value;
    match cell {
        Cell::Null => Value::Null,
        Cell::Bool(b) => Value::Bool(*b),
        Cell::Int(v) => Value::from(*v),
        Cell::Bigint(s) => Value::String(s.clone()),
        Cell::Float(v) => serde_json::Number::from_f64(*v)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(v.to_string())),
        Cell::Numeric(s) => Value::String(s.clone()),
        Cell::Text(s) => Value::String(s.clone()),
        Cell::Bytea { b64 } => Value::String(b64.clone()),
        Cell::Uuid(s) => Value::String(s.clone()),
        Cell::Inet(s) => Value::String(s.clone()),
        Cell::Date(s) => Value::String(s.clone()),
        Cell::Time(s) => Value::String(s.clone()),
        Cell::Timetz(s) => Value::String(s.clone()),
        Cell::Timestamp(s) => Value::String(s.clone()),
        Cell::Timestamptz(s) => Value::String(s.clone()),
        Cell::Interval { iso } => Value::String(iso.clone()),
        Cell::Json(v) => v.clone(),
        Cell::Array { values, .. } => Value::Array(values.iter().map(cell_to_json).collect()),
        Cell::Enum { value, .. } => Value::String(value.clone()),
        Cell::Vector { values, .. } => Value::Array(
            values
                .iter()
                .filter_map(|f| serde_json::Number::from_f64(f64::from(*f)).map(Value::Number))
                .collect(),
        ),
        Cell::Unknown { text, .. } => Value::String(text.clone()),
    }
}

/// RFC-4180 quoting: wrap in double-quotes if the field contains a comma,
/// quote, or newline; double up embedded quotes.
fn csv_escape(s: &str) -> String {
    let needs_quotes = s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r');
    if !needs_quotes {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        if c == '"' {
            out.push('"');
            out.push('"');
        } else {
            out.push(c);
        }
    }
    out.push('"');
    out
}

fn quote_ident(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 2);
    out.push('"');
    for c in name.chars() {
        if c == '"' {
            out.push('"');
            out.push('"');
        } else {
            out.push(c);
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn csv_escape_passes_through_simple_text() {
        assert_eq!(csv_escape("hello"), "hello");
        assert_eq!(csv_escape(""), "");
    }

    #[test]
    fn csv_escape_quotes_when_field_has_comma() {
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
    }

    #[test]
    fn csv_escape_doubles_embedded_quotes() {
        assert_eq!(csv_escape("she said \"hi\""), "\"she said \"\"hi\"\"\"");
    }

    #[test]
    fn csv_escape_quotes_on_newline() {
        assert_eq!(csv_escape("a\nb"), "\"a\nb\"");
        assert_eq!(csv_escape("a\r\nb"), "\"a\r\nb\"");
    }

    #[test]
    fn cell_to_csv_renders_null_as_empty_string() {
        assert_eq!(cell_to_csv(&Cell::Null), "");
    }

    #[test]
    fn cell_to_csv_renders_primitive_variants() {
        assert_eq!(cell_to_csv(&Cell::Bool(true)), "true");
        assert_eq!(cell_to_csv(&Cell::Bool(false)), "false");
        assert_eq!(cell_to_csv(&Cell::Int(42)), "42");
        assert_eq!(cell_to_csv(&Cell::Bigint("9".into())), "9");
        assert_eq!(cell_to_csv(&Cell::Numeric("1.5".into())), "1.5");
        assert_eq!(cell_to_csv(&Cell::Text("hi".into())), "hi");
    }

    #[test]
    fn cell_to_csv_renders_bytea_as_hex_prefix() {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let cell = Cell::Bytea {
            b64: STANDARD.encode([0xDE_u8, 0xAD]),
        };
        assert_eq!(cell_to_csv(&cell), "\\xdead");
    }

    #[test]
    fn cell_to_csv_renders_json_inline() {
        let cell = Cell::Json(json!({ "k": "v" }));
        assert_eq!(cell_to_csv(&cell), r#"{"k":"v"}"#);
    }

    #[test]
    fn cell_to_csv_renders_array_with_braces() {
        let cell = Cell::Array {
            elem: "int4".into(),
            values: vec![Cell::Int(1), Cell::Null, Cell::Int(3)],
        };
        assert_eq!(cell_to_csv(&cell), "{1,,3}");
    }

    #[test]
    fn cell_to_csv_handles_non_finite_floats() {
        assert_eq!(cell_to_csv(&Cell::Float(f64::NAN)), "NaN");
        assert_eq!(cell_to_csv(&Cell::Float(f64::INFINITY)), "Infinity");
        assert_eq!(cell_to_csv(&Cell::Float(f64::NEG_INFINITY)), "-Infinity");
    }

    #[test]
    fn cell_to_json_preserves_null_and_primitives() {
        assert_eq!(cell_to_json(&Cell::Null), serde_json::Value::Null);
        assert_eq!(cell_to_json(&Cell::Bool(true)), json!(true));
        assert_eq!(cell_to_json(&Cell::Int(42)), json!(42));
        assert_eq!(
            cell_to_json(&Cell::Bigint("9999999999999999".into())),
            json!("9999999999999999")
        );
    }

    #[test]
    fn cell_to_json_passes_through_json_value() {
        let inner = json!({ "k": [1, 2, 3] });
        assert_eq!(cell_to_json(&Cell::Json(inner.clone())), inner);
    }

    #[test]
    fn quote_ident_doubles_embedded_quotes() {
        assert_eq!(quote_ident("plain"), "\"plain\"");
        assert_eq!(quote_ident("we\"ird"), "\"we\"\"ird\"");
    }

    #[test]
    fn write_csv_emits_header_and_quoted_fields() {
        let mut buf: Vec<u8> = Vec::new();
        let cols = vec!["id".to_string(), "note".to_string()];
        let rows = vec![
            vec![Cell::Int(1), Cell::Text("a,b".into())],
            vec![Cell::Int(2), Cell::Null],
        ];
        write_csv(&mut buf, &cols, &rows, false).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert_eq!(s, "id,note\n1,\"a,b\"\n2,\n");
    }

    #[test]
    fn write_csv_with_bom_prefixes_three_bytes() {
        let mut buf: Vec<u8> = Vec::new();
        let cols = vec!["x".to_string()];
        let rows = vec![vec![Cell::Int(1)]];
        write_csv(&mut buf, &cols, &rows, true).unwrap();
        assert_eq!(&buf[..3], &[0xEF, 0xBB, 0xBF]);
    }

    #[test]
    fn write_json_emits_array_of_objects() {
        let mut buf: Vec<u8> = Vec::new();
        let cols = vec!["id".to_string(), "name".to_string()];
        let rows = vec![
            vec![Cell::Int(1), Cell::Text("alice".into())],
            vec![Cell::Int(2), Cell::Null],
        ];
        write_json(&mut buf, &cols, &rows).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(
            v,
            json!([{ "id": 1, "name": "alice" }, { "id": 2, "name": null }])
        );
    }

    #[test]
    fn write_sql_inserts_uses_pg_literals_and_quoted_columns() {
        let mut buf: Vec<u8> = Vec::new();
        let cols = vec!["id".to_string(), "note".to_string()];
        let rows = vec![vec![Cell::Int(1), Cell::Text("o'r".into())]];
        write_sql_inserts(&mut buf, "\"public\".\"t\"", &cols, &rows).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert_eq!(
            s,
            "INSERT INTO \"public\".\"t\" (\"id\", \"note\") VALUES (1, 'o''r');\n"
        );
    }

    #[test]
    fn export_result_writes_csv_to_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.csv");
        let req = ExportRequest {
            format: ExportFormat::Csv,
            path: path.to_string_lossy().into_owned(),
            columns: vec!["id".to_string()],
            rows: vec![vec![Cell::Int(1)], vec![Cell::Int(2)]],
            include_bom: false,
            table: None,
        };
        export_result(req).unwrap();
        let s = std::fs::read_to_string(&path).unwrap();
        assert_eq!(s, "id\n1\n2\n");
    }

    #[test]
    fn export_result_sql_insert_requires_table() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.sql");
        let req = ExportRequest {
            format: ExportFormat::SqlInsert,
            path: path.to_string_lossy().into_owned(),
            columns: vec!["id".to_string()],
            rows: vec![vec![Cell::Int(1)]],
            include_bom: false,
            table: None,
        };
        let err = export_result(req).unwrap_err();
        assert!(matches!(err, TuskError::Internal(_)));
    }
}
