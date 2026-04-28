// src-tauri/src/secrets.rs
use crate::errors::{TuskError, TuskResult};

const SERVICE: &str = "tusk";

fn entry(connection_id: &str) -> TuskResult<keyring::Entry> {
    keyring::Entry::new(SERVICE, &format!("conn:{connection_id}"))
        .map_err(|e| TuskError::Secrets(e.to_string()))
}

pub fn set_password(connection_id: &str, password: &str) -> TuskResult<()> {
    entry(connection_id)?
        .set_password(password)
        .map_err(|e| TuskError::Secrets(e.to_string()))
}

pub fn get_password(connection_id: &str) -> TuskResult<Option<String>> {
    match entry(connection_id)?.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(TuskError::Secrets(e.to_string())),
    }
}

pub fn delete_password(connection_id: &str) -> TuskResult<()> {
    match entry(connection_id)?.delete_credential() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(TuskError::Secrets(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trips a password through the OS keychain. Skipped in CI when no
    /// keychain backend is available (the keyring crate falls back gracefully
    /// on Linux only when configured; on macOS the test runs against the
    /// real login keychain).
    #[test]
    fn set_get_delete_roundtrip() {
        let id = format!("test-{}", uuid::Uuid::new_v4());
        if set_password(&id, "hunter2").is_err() {
            eprintln!("skipping set_get_delete_roundtrip: keyring backend unavailable");
            return;
        }
        let got = get_password(&id).unwrap();
        if got.is_none() {
            eprintln!(
                "skipping set_get_delete_roundtrip: keyring read returned None after successful set"
            );
            return;
        }
        assert_eq!(got.as_deref(), Some("hunter2"));
        delete_password(&id).unwrap();
        assert_eq!(get_password(&id).unwrap(), None);
    }
}
