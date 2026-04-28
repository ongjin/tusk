// src-tauri/src/ssh/config.rs
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde::Serialize;

use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SshHost {
    pub alias: String,
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub proxy_jump: Option<String>,
}

pub fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".ssh")
        .join("config")
}

/// Extracts non-wildcard `Host` entries from a config string.
pub fn extract_aliases(config_text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in config_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("host ") || lower.starts_with("host\t") {
            let original = trimmed["host".len()..].trim();
            for token in original.split_whitespace() {
                if token.contains(['*', '?', '!']) {
                    continue;
                }
                out.push(token.to_string());
            }
        }
    }
    out
}

/// Calls `ssh -G <alias>` and parses the `key value` lines into an SshHost.
/// Returns None if the binary fails or exits non-zero.
pub fn resolve_via_ssh_g(alias: &str) -> TuskResult<Option<SshHost>> {
    let output = Command::new("ssh")
        .args(["-G", alias])
        .output()
        .map_err(|e| TuskError::Ssh(format!("ssh -G failed to spawn: {e}")))?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut host = SshHost {
        alias: alias.to_string(),
        hostname: None,
        user: None,
        port: None,
        identity_file: None,
        proxy_jump: None,
    };

    for line in stdout.lines() {
        let mut parts = line.splitn(2, ' ');
        let key = parts.next().unwrap_or("").to_ascii_lowercase();
        let value = parts.next().unwrap_or("").trim();
        if value.is_empty() {
            continue;
        }
        match key.as_str() {
            "hostname" => host.hostname = Some(value.to_string()),
            "user" => host.user = Some(value.to_string()),
            "port" => host.port = value.parse().ok(),
            "identityfile" if host.identity_file.is_none() => {
                host.identity_file = Some(value.to_string())
            }
            "proxyjump" if value != "none" => host.proxy_jump = Some(value.to_string()),
            _ => {}
        }
    }

    Ok(Some(host))
}

pub fn list_known_hosts() -> TuskResult<Vec<SshHost>> {
    let path = config_path();
    let aliases = match fs::read_to_string(&path) {
        Ok(text) => extract_aliases(&text),
        Err(_) => return Ok(Vec::new()),
    };

    let mut out = Vec::new();
    for alias in aliases {
        if let Ok(Some(host)) = resolve_via_ssh_g(&alias) {
            out.push(host);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_non_wildcard_hosts() {
        let cfg = r#"
# preamble
Host *
    User defaults

Host oci-db oci-util
    HostName 10.0.0.4
    ProxyJump app-cf

Host app-cf
    HostName cf.example.com
"#;
        let aliases = extract_aliases(cfg);
        assert_eq!(aliases, vec!["oci-db", "oci-util", "app-cf"]);
    }
}
