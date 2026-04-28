// src-tauri/src/errors.rs
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug, Serialize)]
#[serde(tag = "kind", content = "data")]
pub enum TuskError {
    #[error("Connection failed: {0}")]
    Connection(String),
    #[error("Query failed: {0}")]
    Query(String),
    #[error("SSH tunnel failed: {0}")]
    Tunnel(String),
    #[error("SSH config error: {0}")]
    Ssh(String),
    #[error("State error: {0}")]
    State(String),
    #[error("Secrets error: {0}")]
    Secrets(String),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Editing failed: {0}")]
    Editing(String),
    #[error("Conflict on batch")]
    Conflict {
        #[serde(rename = "batchId")]
        batch_id: String,
        #[serde(rename = "executedSql")]
        executed_sql: String,
        current: serde_json::Value,
    },
    #[error("Transaction error: {0}")]
    Tx(String),
    #[error("Transaction aborted — only ROLLBACK is allowed")]
    TxAborted,
    #[error("Query cancelled")]
    QueryCancelled,
    #[error("History error: {0}")]
    History(String),
    #[error("Unsupported column type for editing: oid={oid}, name={name}")]
    UnsupportedEditType { oid: u32, name: String },

    #[error("AI provider error: {0}")]
    Ai(String),

    #[error("AI provider not configured: {0}")]
    AiNotConfigured(String),

    #[error("Schema index error: {0}")]
    SchemaIndex(String),

    #[error("Embedding HTTP error: {0}")]
    EmbeddingHttp(String),

    #[error("Destructive guard: parser failed")]
    DestructiveParserFailed,

    #[error("Destructive guard: confirmation required")]
    DestructiveConfirmRequired,
}

impl From<anyhow::Error> for TuskError {
    fn from(e: anyhow::Error) -> Self {
        TuskError::Internal(format!("{e:#}"))
    }
}

pub type TuskResult<T> = Result<T, TuskError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_uses_tagged_repr() {
        let err = TuskError::Connection("nope".into());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, r#"{"kind":"Connection","data":"nope"}"#);
    }

    #[test]
    fn from_anyhow_becomes_internal() {
        let any = anyhow::anyhow!("boom");
        let err: TuskError = any.into();
        assert!(matches!(err, TuskError::Internal(_)));
    }

    #[test]
    fn serialize_conflict_carries_payload() {
        let err = TuskError::Conflict {
            batch_id: "b1".into(),
            executed_sql: "UPDATE t ...".into(),
            current: serde_json::json!({ "id": 1 }),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"kind\":\"Conflict\""));
        assert!(json.contains("\"batchId\":\"b1\""));
        assert!(json.contains("\"executedSql\":\"UPDATE t ...\""));
    }

    #[test]
    fn tx_aborted_serializes_as_tag_only() {
        let err = TuskError::TxAborted;
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"kind\":\"TxAborted\""));
    }
}
