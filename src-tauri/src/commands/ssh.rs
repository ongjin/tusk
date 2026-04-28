// src-tauri/src/commands/ssh.rs
use crate::errors::TuskResult;
use crate::ssh::config::{list_known_hosts, SshHost};

#[tauri::command]
pub fn list_known_ssh_hosts() -> TuskResult<Vec<SshHost>> {
    list_known_hosts()
}
