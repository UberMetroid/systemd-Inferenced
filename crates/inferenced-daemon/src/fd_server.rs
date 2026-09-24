use inferenced_core::fd_lease::{create_sealed_memfd, send_fd_over_unix};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tokio::net::UnixListener;
use tracing::{error, info};

pub const DEFAULT_FD_SOCKET_PATH: &str = "/run/systemd-inferenced/fd.sock";

#[derive(Debug, Serialize, Deserialize)]
pub struct FdRequest {
    pub action: String,
    pub name: Option<String>,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FdResponse {
    pub status: String,
    pub message: Option<String>,
}

pub async fn run_fd_server(listener: UnixListener) -> anyhow::Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            let std_stream = match stream.into_std() {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to convert Tokio UnixStream to std: {}", e);
                    return;
                }
            };

            let mut reader = std_stream.try_clone().expect("failed to clone socket");
            let mut buf = [0u8; 1024];
            let n = match std::io::Read::read(&mut reader, &mut buf) {
                Ok(n) if n > 0 => n,
                _ => return,
            };

            let req: FdRequest = match serde_json::from_slice(&buf[..n]) {
                Ok(r) => r,
                Err(e) => {
                    let resp = FdResponse {
                        status: "error".into(),
                        message: Some(format!("Invalid JSON request: {}", e)),
                    };
                    let _ = std::io::Write::write_all(
                        &mut reader,
                        &serde_json::to_vec(&resp).unwrap_or_default(),
                    );
                    return;
                }
            };

            if req.action == "create" || req.action == "lease" {
                let name = req.name.unwrap_or_else(|| "inferenced-shm".into());
                let size = req.size_bytes.unwrap_or(4 * 1024 * 1024); // default 4MB

                match create_sealed_memfd(&name, size, None) {
                    Ok(memfd) => {
                        let resp = FdResponse {
                            status: "ok".into(),
                            message: Some(format!("Created sealed memfd '{}' ({} bytes)", name, size)),
                        };
                        let payload = serde_json::to_vec(&resp).unwrap_or_default();
                        if let Err(e) = send_fd_over_unix(&std_stream, &memfd, &payload) {
                            error!("Failed to pass fd over SCM_RIGHTS: {}", e);
                        } else {
                            info!("Successfully passed sealed memfd '{}' to client", name);
                        }
                    }
                    Err(e) => {
                        let resp = FdResponse {
                            status: "error".into(),
                            message: Some(format!("Failed to create sealed memfd: {}", e)),
                        };
                        let _ = std::io::Write::write_all(
                            &mut reader,
                            &serde_json::to_vec(&resp).unwrap_or_default(),
                        );
                    }
                }
            }
        });
    }
}

pub fn bind_or_create_fd_listener(path_str: &str) -> anyhow::Result<UnixListener> {
    let path = Path::new(path_str);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    let listener = UnixListener::bind(path)?;
    info!("Bound SCM_RIGHTS FD server on {}", path_str);
    Ok(listener)
}
