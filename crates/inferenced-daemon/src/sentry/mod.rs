use inferenced_core::{arbiter::Arbiter, lease::LeasePriority};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tracing::{error, info, warn};

#[allow(dead_code)]
pub const DEFAULT_SENTRY_SOCKET_PATH: &str = "/run/systemd-inferenced/sentry.sock";

#[derive(Debug, Serialize, Deserialize)]
pub struct SentryTriageResponse {
    pub status: String,
    pub incident_id: Option<String>,
    pub allocated_plane: String,
    pub analysis: String,
}

pub async fn run_sentry_triage_listener(
    listener: UnixListener,
    arbiter: Arc<Arbiter>,
) -> anyhow::Result<()> {
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let arbiter_clone = arbiter.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_sentry_connection(stream, arbiter_clone).await {
                        error!("Error handling sentry triage connection: {}", e);
                    }
                });
            }
            Err(e) => {
                warn!("Error accepting triage connection: {}", e);
            }
        }
    }
}

async fn handle_sentry_connection(
    mut stream: UnixStream,
    arbiter: Arc<Arbiter>,
) -> anyhow::Result<()> {
    info!("Received emergency triage connection from systemd-sentry!");

    // Read payload
    let mut buf = vec![0u8; 65536];
    let n = stream.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }

    let payload_json: Value = serde_json::from_slice(&buf[..n])
        .unwrap_or_else(|_| serde_json::json!({ "raw": String::from_utf8_lossy(&buf[..n]) }));

    let incident_id = payload_json
        .get("incident_id")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);

    let unit_name = payload_json
        .get("unit_name")
        .and_then(|v| v.as_str())
        .unwrap_or("systemd-sentry.service");

    // Acquire emergency triage slice
    let lease = arbiter
        .acquire_lease(
            LeasePriority::EmergencyTriage,
            1024 * 1024 * 1024, // 1GB dedicated emergency memory
            None,
            Some(unit_name.to_string()),
            None,
        )
        .await?;

    info!(
        "Acquired emergency lease {} on plane {} for sentry incident {:?}",
        lease.id, lease.plane_id, incident_id
    );

    let response = SentryTriageResponse {
        status: "accepted".into(),
        incident_id,
        allocated_plane: lease.plane_id.clone(),
        analysis: format!(
            "Emergency triage isolated on plane {}. AI hardware state preserved.",
            lease.plane_id
        ),
    };

    let reply_bytes = serde_json::to_vec(&response)?;
    stream.write_all(&reply_bytes).await?;
    stream.flush().await?;

    // Release emergency slice
    arbiter.release_lease(lease.id).await?;
    info!("Released emergency triage lease {}", lease.id);
    Ok(())
}

pub fn bind_or_create_sentry_listener(path: &Path) -> anyhow::Result<UnixListener> {
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let listener = UnixListener::bind(path)?;
    info!("Bound Sentry emergency triage listener on {:?}", path);
    Ok(listener)
}
