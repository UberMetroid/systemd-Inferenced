mod gateway;
mod notify;
mod sentry;

use clap::Parser;
use gateway::{build_gateway_router, AppState};
use inferenced_core::{arbiter::Arbiter, topology::HardwareTopology};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(name = "systemd-inferenced")]
#[command(about = "Heterogeneous AI Hardware Arbiter and Model Lifecycle Broker")]
#[command(version)]
struct Cli {
    #[arg(short, long, default_value = "/etc/systemd/inferenced.conf")]
    config: PathBuf,

    #[arg(short, long, default_value = "127.0.0.1:11434")]
    bind: SocketAddr,

    #[arg(long, default_value = "/run/systemd-inferenced/sentry.sock")]
    sentry_socket: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "systemd_inferenced=info,inferenced_core=info".into()),
        )
        .init();

    let cli = Cli::parse();
    info!("Starting systemd-inferenced daemon...");

    let topo = HardwareTopology::discover()?;
    info!(
        "Discovered {} compute planes ({} RAM, {} cores)",
        topo.planes.len(),
        topo.total_system_ram_bytes / (1024 * 1024 * 1024),
        topo.cpu_cores_total
    );

    let arbiter = Arc::new(Arbiter::new(topo));
    let state = Arc::new(AppState {
        arbiter: arbiter.clone(),
    });

    notify::notify_systemd_ready();

    // Spawn Sentry emergency triage listener
    let sentry_socket = cli.sentry_socket.clone();
    let sentry_arbiter = arbiter.clone();
    tokio::spawn(async move {
        if let Err(e) = sentry::run_sentry_triage_listener(sentry_socket, sentry_arbiter).await {
            error!("Sentry emergency triage socket failed: {}", e);
        }
    });

    // Start HTTP Gateway
    let app = build_gateway_router(state);
    info!("Gateway listening on http://{}", cli.bind);
    let listener = tokio::net::TcpListener::bind(cli.bind).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("systemd-inferenced daemon terminated cleanly.");
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C signal handler");
    info!("Termination signal received, shutting down gracefully...");
}
