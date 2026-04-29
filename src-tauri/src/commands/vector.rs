use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct VectorColumn {
    pub schema: String,
    pub table: String,
    pub column: String,
    pub dim: i32,
    pub has_index: bool,
}

#[derive(Debug, Serialize, Default, Clone)]
pub struct VectorIndexParams {
    pub m: Option<i32>,
    pub ef_construction: Option<i32>,
    pub lists: Option<i32>,
    pub ops: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct VectorIndex {
    pub name: String,
    pub schema: String,
    pub table: String,
    pub column: String,
    pub method: String,
    pub params: VectorIndexParams,
    pub size_bytes: i64,
    pub definition: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct SampledVectorRow {
    pub pk_json: serde_json::Value,
    pub vec: Vec<f32>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SampledVectors {
    pub rows: Vec<SampledVectorRow>,
    pub total_rows: i64,
}

pub(crate) fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_ident_doubles_embedded_quotes() {
        assert_eq!(quote_ident("plain"), "\"plain\"");
        assert_eq!(quote_ident("we\"ird"), "\"we\"\"ird\"");
    }
}
