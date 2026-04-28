//! Frontend-facing wrappers around `secrets::ai_*`.
//!
//! `ai_secret_get` is the only path that returns the raw key value; it must
//! be called only at the moment of LLM invocation and the value MUST NOT be
//! cached anywhere on the frontend.

use crate::errors::TuskResult;
use crate::secrets;

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn ai_secret_set(provider: String, value: String) -> TuskResult<()> {
    secrets::ai_set(&provider, &value)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn ai_secret_get(provider: String) -> TuskResult<Option<String>> {
    secrets::ai_get(&provider)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn ai_secret_delete(provider: String) -> TuskResult<()> {
    secrets::ai_delete(&provider)
}

/// Returns the providers that currently have a key in the keychain. Used
/// by the Settings UI to render `apiKeyPresent: bool`.
#[tauri::command]
pub fn ai_secret_list_present() -> TuskResult<Vec<String>> {
    let providers = ["openai", "anthropic", "gemini", "ollama"];
    let mut present = Vec::new();
    for p in providers {
        if secrets::ai_get(p)?.is_some() {
            present.push(p.to_string());
        }
    }
    Ok(present)
}
