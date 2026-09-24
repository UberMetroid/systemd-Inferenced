use inferenced_core::{arbiter::Arbiter, lease::LeasePriority};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::UnixListener;
use tracing::{error, info, warn};

pub async fn run_sentry_triage_listener(
    socket_path: PathBuf,
    arbiter: Arc<Arbiter>,
) -> anyhow::Result<()> {
    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }
    if let Some(parent) = socket_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let listener = match UnixListener::bind(&socket_path) {
        Ok(l) => l,
        Err(e) => {
            warn!("Could not bind Sentry socket at {:?}: {}. Running without triage socket.", socket_path, e);
            return Ok(());
        }
    };

    info!("Sentry emergency triage enclave listening on {:?}", socket_path);

    loop {
        match listener.accept().await {
            Ok((_stream, _addr)) => {
                info!("Received emergency triage request from systemd-sentry!");
                let lease = arbiter
                    .acquire_lease(
                        LeasePriority::EmergencyTriage,
                        1024 * 1024 * 1024, // 1GB dedicated slice
                        None,
                        Some("systemd-sentry.service".into()),
                        None,
                    )
                    .await;
                if let Err(e) = lease {
                    error!("Failed to grant emergency triage lease: {}", e);
                }
            }
            Err(e) => {
                warn!("Error accepting triage connection: {}", e);
            }
        }
    }
}
