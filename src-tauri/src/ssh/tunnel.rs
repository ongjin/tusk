// src-tauri/src/ssh/tunnel.rs
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

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

pub async fn open_tunnel(spec: TunnelSpec) -> TuskResult<TunnelHandle> {
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

    let handle = TunnelHandle { child, local_port };

    // Poll until the forwarded port accepts TCP, or we time out.
    let started = Instant::now();
    let timeout = Duration::from_secs(5);
    loop {
        if TcpStream::connect(("127.0.0.1", local_port)).await.is_ok() {
            return Ok(handle);
        }
        if started.elapsed() >= timeout {
            return Err(TuskError::Tunnel(format!(
                "tunnel readiness timed out after {}s",
                timeout.as_secs()
            )));
        }
        sleep(Duration::from_millis(50)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unknown_alias_times_out() {
        let spec = TunnelSpec {
            target: SshTarget::Alias("definitely-not-a-real-host-tusk".into()),
            remote_host: "127.0.0.1".into(),
            remote_port: 5432,
        };
        let result = open_tunnel(spec).await;
        assert!(result.is_err(), "expected tunnel error, got {result:?}");
    }
}
