// src-tauri/src/commands/connections.rs
use serde::Serialize;
use tauri::State;

use crate::db::pg_meta::MetaCache;
use crate::db::pool::{ConnectionRegistry, DirectConnectSpec};
use crate::db::state::{ConnectionRecord, NewConnection, SshKind, StateStore};
use crate::errors::{TuskError, TuskResult};
use crate::secrets;
use crate::ssh::tunnel::{open_tunnel, SshTarget, TunnelSpec};

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
    if let Err(e) = secrets::set_password(&record.id, &password) {
        // Best-effort rollback so the user doesn't end up with an
        // orphaned record that can never authenticate.
        let _ = store.delete(&record.id);
        return Err(e);
    }
    Ok(record)
}

#[tauri::command]
pub async fn delete_connection(
    store: State<'_, StateStore>,
    registry: State<'_, ConnectionRegistry>,
    meta_cache: State<'_, MetaCache>,
    id: String,
) -> TuskResult<()> {
    registry.disconnect(&id).await?;
    meta_cache.invalidate_conn(&id);
    secrets::delete_password(&id)?;
    store.delete(&id)?;
    Ok(())
}

#[tauri::command]
pub async fn connect(
    app: tauri::AppHandle,
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
        SshKind::Alias => {
            let alias = record
                .ssh_alias
                .clone()
                .ok_or_else(|| TuskError::Tunnel("ssh_alias missing".into()))?;
            let tunnel = open_tunnel(
                app.clone(),
                id.clone(),
                TunnelSpec {
                    target: SshTarget::Alias(alias),
                    remote_host: record.host.clone(),
                    remote_port: record.port,
                },
            )
            .await?;
            let spec = DirectConnectSpec {
                host: record.host,
                port: record.port,
                user: record.db_user,
                password,
                database: record.database,
                ssl_mode: record.ssl_mode,
            };
            registry.connect_tunneled(&id, spec, tunnel).await?;
            Ok(())
        }
        SshKind::Manual => {
            let host = record
                .ssh_host
                .clone()
                .ok_or_else(|| TuskError::Tunnel("ssh_host missing".into()))?;
            let port = record.ssh_port.unwrap_or(22);
            let user = record
                .ssh_user
                .clone()
                .ok_or_else(|| TuskError::Tunnel("ssh_user missing".into()))?;
            let key_path = record.ssh_key_path.clone();
            let tunnel = open_tunnel(
                app.clone(),
                id.clone(),
                TunnelSpec {
                    target: SshTarget::Manual {
                        host,
                        port,
                        user,
                        key_path,
                    },
                    remote_host: record.host.clone(),
                    remote_port: record.port,
                },
            )
            .await?;
            let spec = DirectConnectSpec {
                host: record.host,
                port: record.port,
                user: record.db_user,
                password,
                database: record.database,
                ssl_mode: record.ssl_mode,
            };
            registry.connect_tunneled(&id, spec, tunnel).await?;
            Ok(())
        }
    }
}

#[tauri::command]
pub async fn disconnect(
    registry: State<'_, ConnectionRegistry>,
    meta_cache: State<'_, MetaCache>,
    id: String,
) -> TuskResult<()> {
    registry.disconnect(&id).await?;
    meta_cache.invalidate_conn(&id);
    Ok(())
}
