// src-tauri/src/commands/connections.rs
use serde::Serialize;
use tauri::State;

use crate::db::pool::{ConnectionRegistry, DirectConnectSpec};
use crate::db::state::{ConnectionRecord, NewConnection, SshKind, StateStore};
use crate::errors::{TuskError, TuskResult};
use crate::secrets;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionListItem {
    #[serde(flatten)]
    pub record: ConnectionRecord,
    pub connected: bool,
}

#[tauri::command]
pub async fn list_connections(
    store: State<'_, StateStore>,
    registry: State<'_, ConnectionRegistry>,
) -> TuskResult<Vec<ConnectionListItem>> {
    let records = store.list()?;
    Ok(records
        .into_iter()
        .map(|r| {
            let connected = registry.is_connected(&r.id);
            ConnectionListItem {
                record: r,
                connected,
            }
        })
        .collect())
}

#[tauri::command]
pub async fn add_connection(
    store: State<'_, StateStore>,
    new: NewConnection,
    password: String,
) -> TuskResult<ConnectionRecord> {
    let record = store.insert(new)?;
    secrets::set_password(&record.id, &password)?;
    Ok(record)
}

#[tauri::command]
pub async fn delete_connection(
    store: State<'_, StateStore>,
    registry: State<'_, ConnectionRegistry>,
    id: String,
) -> TuskResult<()> {
    registry.disconnect(&id)?;
    secrets::delete_password(&id)?;
    store.delete(&id)?;
    Ok(())
}

#[tauri::command]
pub async fn connect(
    store: State<'_, StateStore>,
    registry: State<'_, ConnectionRegistry>,
    id: String,
) -> TuskResult<()> {
    let record = store
        .get(&id)?
        .ok_or_else(|| TuskError::Connection(format!("unknown connection {id}")))?;
    let password = secrets::get_password(&record.id)?
        .ok_or_else(|| TuskError::Secrets("no password stored".into()))?;

    match record.ssh_kind {
        SshKind::None => {
            let spec = DirectConnectSpec {
                host: record.host,
                port: record.port,
                user: record.db_user,
                password,
                database: record.database,
                ssl_mode: record.ssl_mode,
            };
            registry.connect_direct(&id, spec).await?;
            Ok(())
        }
        SshKind::Alias | SshKind::Manual => {
            // Wired up in Task 6.
            Err(TuskError::Tunnel("SSH-backed connect not yet wired".into()))
        }
    }
}

#[tauri::command]
pub async fn disconnect(registry: State<'_, ConnectionRegistry>, id: String) -> TuskResult<()> {
    registry.disconnect(&id)
}
