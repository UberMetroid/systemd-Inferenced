use inferenced_core::fd_lease::{create_sealed_memfd, send_fd_over_unix};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use tracing::{error, info, warn};

pub use crate::fd_quota::{FdQuota, MAX_MEMFD_BYTES};

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

/// Runs the FD handoff listener with a half-of-RAM quota.
pub async fn run_fd_server(listener: UnixListener) -> anyhow::Result<()> {
    run_fd_server_with_quota(listener, Arc::new(FdQuota::from_half_of_total_ram())).await
}

/// Variant that accepts an explicit `FdQuota` for tests and embedded
/// users that want to override the default RAM-based ceiling.
pub async fn run_fd_server_with_quota(
    listener: UnixListener,
    quota: Arc<FdQuota>,
) -> anyhow::Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let quota_clone = quota.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, quota_clone).await {
                warn!("FD handoff client error: {}", e);
            }
        });
    }
}

async fn handle_connection(
    mut stream: tokio::net::UnixStream,
    quota: Arc<FdQuota>,
) -> anyhow::Result<()> {
    let mut scratch = Vec::with_capacity(4096);
    let req: FdRequest = match read_framed_request(&mut stream, &mut scratch).await? {
        Some(r) => r,
        None => return Ok(()),
    };

    if req.action != "create" && req.action != "lease" {
        return write_response(
            &mut stream,
            &FdResponse {
                status: "error".into(),
                message: Some(format!(
                    "Unknown action '{}'. Supported actions: 'create', 'lease'",
                    req.action
                )),
            },
        )
        .await;
    }

    let name = req.name.unwrap_or_else(|| "inferenced-shm".into());
    let size = req.size_bytes.unwrap_or(4 * 1024 * 1024);

    if size > MAX_MEMFD_BYTES {
        return write_response(
            &mut stream,
            &FdResponse {
                status: "error".into(),
                message: Some(format!(
                    "Requested size {} exceeds limit of {} bytes",
                    size, MAX_MEMFD_BYTES
                )),
            },
        )
        .await;
    }

    let reserved = match quota.try_reserve(size) {
        Ok(p) => p,
        Err(current) => {
            return write_response(
                &mut stream,
                &FdResponse {
                    status: "error".into(),
                    message: Some(format!(
                        "Quota exceeded: {} bytes already allocated, request {} bytes",
                        current, size
                    )),
                },
            )
            .await;
        }
    };

    let memfd = match create_sealed_memfd(&name, size, None) {
        Ok(m) => m,
        Err(e) => {
            quota.release(reserved);
            return write_response(
                &mut stream,
                &FdResponse {
                    status: "error".into(),
                    message: Some(format!("Failed to create sealed memfd: {}", e)),
                },
            )
            .await;
        }
    };

    let resp = FdResponse {
        status: "ok".into(),
        message: Some(format!(
            "Created sealed memfd '{}' ({} bytes)",
            name, size
        )),
    };
    let payload = serde_json::to_vec(&resp).unwrap_or_default();

    let _ = stream.writable().await;
    match send_fd_over_unix(&stream, &memfd, &payload) {
        Ok(_) => {
            info!(
                "Transferred sealed memfd '{}' ({} bytes) to client",
                name, size
            );
            // The memfd is now owned by the kernel cmsg buffer; the
            // recipient inherits the FD and our `memfd` handle is
            // consumed by send_fd_over_unix. Release the in-flight
            // reservation so the listener's quota tracks concurrent
            // allocations, not lifetime allocations.
            quota.release(reserved);
        }
        Err(e) => {
            error!(
                "SCM_RIGHTS send failed for memfd '{}' ({} bytes): {}",
                name, size, e
            );
            drop(memfd);
            quota.release(reserved);
        }
    }
    Ok(())
}

/// Read a single NUL-framed JSON request from `stream` into `scratch`.
async fn read_framed_request(
    stream: &mut tokio::net::UnixStream,
    scratch: &mut Vec<u8>,
) -> anyhow::Result<Option<FdRequest>> {
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Ok(None);
        }
        scratch.extend_from_slice(&chunk[..n]);
        if let Some(nul_pos) = scratch.iter().position(|&b| b == 0x00) {
            let payload: Vec<u8> = scratch.drain(..=nul_pos).collect();
            let payload = &payload[..payload.len() - 1];
            let req: FdRequest = serde_json::from_slice(payload)?;
            return Ok(Some(req));
        }
        if scratch.len() > 64 * 1024 {
            return Err(anyhow::anyhow!(
                "FD request exceeded 64 KiB without NUL terminator"
            ));
        }
    }
}

async fn write_response(
    stream: &mut tokio::net::UnixStream,
    resp: &FdResponse,
) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec(resp)?;
    let mut framed = bytes;
    framed.push(0x00);
    stream.write_all(&framed).await?;
    stream.flush().await?;
    Ok(())
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
