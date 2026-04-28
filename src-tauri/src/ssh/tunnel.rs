// src-tauri/src/ssh/tunnel.rs
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Runtime};
use tokio::net::TcpStream;
use tokio::time::sleep;

use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Clone)]
pub enum SshTarget {
    Alias(String),
    Manual {
        host: String,
        port: u16,
        user: String,
        key_path: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct TunnelSpec {
    pub target: SshTarget,
    pub remote_host: String, // Postgres host as seen from the bastion
    pub remote_port: u16,
}

#[derive(Debug)]
pub struct TunnelHandle {
    pub child: Child,
    pub local_port: u16,
}

impl Drop for TunnelHandle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn pick_free_port() -> TuskResult<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| TuskError::Tunnel(format!("bind 127.0.0.1:0 failed: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| TuskError::Tunnel(e.to_string()))?
        .port();
    drop(listener);
    Ok(port)
}

pub async fn open_tunnel<R: Runtime>(
    app: AppHandle<R>,
    connection_id: String,
    spec: TunnelSpec,
) -> TuskResult<TunnelHandle> {
    let local_port = pick_free_port()?;

    let mut cmd = Command::new("ssh");
    cmd.args([
        "-N",
        "-o",
        "ServerAliveInterval=30",
        "-o",
        "ServerAliveCountMax=3",
        "-o",
        "ExitOnForwardFailure=yes",
        "-o",
        "BatchMode=no",
        "-L",
        &format!(
            "127.0.0.1:{local_port}:{}:{}",
            spec.remote_host, spec.remote_port
        ),
    ]);

    match &spec.target {
        SshTarget::Alias(alias) => {
            cmd.arg(alias);
        }
        SshTarget::Manual {
            host,
            port,
            user,
            key_path,
        } => {
            cmd.args(["-p", &port.to_string()]);
            if let Some(path) = key_path {
                cmd.args(["-i", path]);
            }
            cmd.arg(format!("{user}@{host}"));
        }
    }

    let child = cmd
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| TuskError::Tunnel(format!("ssh spawn failed: {e}")))?;

    let mut handle = TunnelHandle { child, local_port };

    // Poll until the forwarded port accepts TCP, or we time out.
    let started = Instant::now();
    let timeout = Duration::from_secs(5);
    loop {
        if TcpStream::connect(("127.0.0.1", local_port)).await.is_ok() {
            let pid = handle.child.id();

            #[cfg(unix)]
            {
                let app_for_task = app.clone();
                let id_for_task = connection_id.clone();
                tokio::spawn(async move {
                    use nix::sys::signal::kill;
                    use nix::unistd::Pid;
                    loop {
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        let alive = kill(Pid::from_raw(pid as i32), None).is_ok();
                        if !alive {
                            let _ = app_for_task.emit("connection:lost", &id_for_task);
                            break;
                        }
                    }
                });
            }

            #[cfg(not(unix))]
            {
                // Windows path: tunnel-death detection is a v1.5 follow-up.
                let _ = (&app, &connection_id, pid);
            }

            return Ok(handle);
        }
        if started.elapsed() >= timeout {
            let stderr_msg = handle
                .child
                .stderr
                .take()
                .and_then(|mut r| {
                    use std::io::Read;
                    let mut s = String::new();
                    r.read_to_string(&mut s).ok().map(|_| s)
                })
                .unwrap_or_default();
            let trimmed = stderr_msg.trim();
            return Err(TuskError::Tunnel(if trimmed.is_empty() {
                format!("tunnel readiness timed out after {}s", timeout.as_secs())
            } else {
                format!(
                    "tunnel readiness timed out after {}s: {trimmed}",
                    timeout.as_secs(),
                )
            }));
        }
        sleep(Duration::from_millis(50)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unknown_alias_times_out() {
        let app = tauri::test::mock_app();
        let spec = TunnelSpec {
            target: SshTarget::Alias("definitely-not-a-real-host-tusk".into()),
            remote_host: "127.0.0.1".into(),
            remote_port: 5432,
        };
        let result = open_tunnel(app.handle().clone(), "test-id".into(), spec).await;
        assert!(result.is_err(), "expected tunnel error, got {result:?}");
    }
}
