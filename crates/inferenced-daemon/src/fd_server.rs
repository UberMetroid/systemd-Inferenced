use inferenced_core::fd_lease::{create_sealed_memfd, send_fd_over_unix};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
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
        let (mut stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            let n = match stream.read(&mut buf).await {
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
                    if let Ok(bytes) = serde_json::to_vec(&resp) {
                        let _ = stream.write_all(&bytes).await;
                    }
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
                        let _ = stream.writable().await;
                        if let Err(e) = send_fd_over_unix(&stream, &memfd, &payload) {
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
                        if let Ok(bytes) = serde_json::to_vec(&resp) {
                            let _ = stream.write_all(&bytes).await;
                        }
                    }
                }
            } else {
                let resp = FdResponse {
                    status: "error".into(),
                    message: Some(format!(
                        "Unknown action '{}'. Supported actions: 'create', 'lease'",
                        req.action
                    )),
                };
                if let Ok(bytes) = serde_json::to_vec(&resp) {
                    let _ = stream.write_all(&bytes).await;
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

#[cfg(test)]
mod tests {
    use super::*;
    use inferenced_core::fd_lease::recv_fd_from_unix;
    use tempfile::tempdir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;

    #[tokio::test]
    async fn test_fd_server_create_and_unknown_action() {
        let dir = tempdir().unwrap();
        let sock_path = dir.path().join("fd_test.sock");
        let listener = UnixListener::bind(&sock_path).unwrap();

        tokio::spawn(async move {
            let _ = run_fd_server(listener).await;
        });

        // 1. Unknown action receives error response
        let mut client1 = UnixStream::connect(&sock_path).await.unwrap();
        let bad_req = serde_json::json!({ "action": "invalid_cmd" });
        client1.write_all(&serde_json::to_vec(&bad_req).unwrap()).await.unwrap();

        let mut resp_buf = vec![0u8; 512];
        let n = client1.read(&mut resp_buf).await.unwrap();
        assert!(n > 0);
        let resp: FdResponse = serde_json::from_slice(&resp_buf[..n]).unwrap();
        assert_eq!(resp.status, "error");
        assert!(resp.message.unwrap().contains("Unknown action"));

        // 2. Create action passes sealed memfd over SCM_RIGHTS
        let mut client2 = UnixStream::connect(&sock_path).await.unwrap();
        let create_req = serde_json::json!({ "action": "create", "name": "test_tensor", "size_bytes": 4096 });
        client2.write_all(&serde_json::to_vec(&create_req).unwrap()).await.unwrap();
        client2.flush().await.unwrap();

        client2.readable().await.unwrap();
        let mut buf = [0u8; 512];
        let (bytes, fd_opt) = recv_fd_from_unix(&client2, &mut buf).unwrap();
        assert!(bytes > 0);
        assert!(fd_opt.is_some(), "Client must receive sealed memfd via SCM_RIGHTS");
    }
}
