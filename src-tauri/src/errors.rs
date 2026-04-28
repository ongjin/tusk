// src-tauri/src/errors.rs
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug, Serialize)]
#[serde(tag = "kind", content = "message")]
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
        assert_eq!(json, r#"{"kind":"Connection","message":"nope"}"#);
    }

    #[test]
    fn from_anyhow_becomes_internal() {
        let any = anyhow::anyhow!("boom");
        let err: TuskError = any.into();
        assert!(matches!(err, TuskError::Internal(_)));
    }
}
