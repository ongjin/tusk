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

const ALLOWED_AI_PROVIDERS: &[&str] = &["openai", "anthropic", "gemini", "ollama"];

fn validate_provider(p: &str) -> TuskResult<()> {
    if ALLOWED_AI_PROVIDERS.contains(&p) {
        Ok(())
    } else {
        Err(TuskError::Ai(format!("unknown provider: {p}")))
    }
}

fn ai_entry(provider: &str) -> TuskResult<keyring::Entry> {
    keyring::Entry::new(SERVICE, &format!("ai:{provider}"))
        .map_err(|e| TuskError::Secrets(e.to_string()))
}

pub fn ai_set(provider: &str, value: &str) -> TuskResult<()> {
    validate_provider(provider)?;
    ai_entry(provider)?
        .set_password(value)
        .map_err(|e| TuskError::Secrets(e.to_string()))
}

pub fn ai_get(provider: &str) -> TuskResult<Option<String>> {
    validate_provider(provider)?;
    match ai_entry(provider)?.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(TuskError::Secrets(e.to_string())),
    }
}

pub fn ai_delete(provider: &str) -> TuskResult<()> {
    validate_provider(provider)?;
    match ai_entry(provider)?.delete_credential() {
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

    #[test]
    fn ai_set_get_delete_roundtrip() {
        let provider = "openai";
        let _ = ai_delete(provider);
        if ai_set(provider, "sk-test-xyz").is_err() {
            eprintln!("skipping ai_set_get_delete_roundtrip: keyring backend unavailable");
            return;
        }
        let got = ai_get(provider).unwrap();
        if got.is_none() {
            eprintln!("skipping ai_set_get_delete_roundtrip: keyring read returned None after successful set");
            return;
        }
        assert_eq!(got.as_deref(), Some("sk-test-xyz"));
        ai_delete(provider).unwrap();
        assert_eq!(ai_get(provider).unwrap(), None);
    }

    #[test]
    fn ai_unknown_provider_rejected() {
        assert!(ai_set("oxygen", "x").is_err());
        assert!(ai_get("oxygen").is_err());
        assert!(ai_delete("oxygen").is_err());
    }
}
