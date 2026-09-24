mod activation;
mod creds;
mod fd_server;
mod gateway;
mod notify;
mod sentry;
mod varlink;

use activation::{check_and_adopt_sockets, GatewayListener};
use clap::Parser;
use gateway::{build_gateway_router, serve_gateway, AppState};
use inferenced_core::{
    arbiter::Arbiter, paging::MemfdPaging, preempt::PreemptCoordinator, topology::HardwareTopology,
};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
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

    #[arg(long, default_value = "/run/systemd-inferenced/io.systemd.inferenced1")]
    varlink_socket: PathBuf,

    #[arg(long, default_value = "/run/systemd-inferenced/sentry.sock")]
    sentry_socket: PathBuf,

    #[arg(long, default_value = "/run/systemd-inferenced/fd.sock")]
    fd_socket: PathBuf,
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
    info!("Starting systemd-inferenced daemon in pure Rust...");

    let mut activated = check_and_adopt_sockets()?;

    let topo = HardwareTopology::discover()?;
    info!(
        "Discovered {} compute planes ({} GB RAM, {} cores)",
        topo.planes.len(),
        topo.total_system_ram_bytes / (1024 * 1024 * 1024),
        topo.cpu_cores_total
    );

    let arbiter = Arc::new(Arbiter::new(topo));
    let preempt = Arc::new(PreemptCoordinator::new(arbiter.clone()));

    // Wire zero-copy paging manager
    let _ = MemfdPaging;

    // 1. Varlink IPC Server (FD 3)
    let varlink_listener = match activated.varlink.take() {
        Some(l) => l,
        None => varlink::bind_or_create_listener(
            cli.varlink_socket
                .to_str()
                .unwrap_or(varlink::DEFAULT_VARLINK_PATH),
        )?,
    };
    let varlink_arbiter = arbiter.clone();
    tokio::spawn(async move {
        if let Err(e) = varlink::run_varlink_listener(varlink_listener, varlink_arbiter).await {
            error!("Varlink IPC server encountered an error: {}", e);
        }
    });

    // 2. Sentry Emergency Enclave (FD 4)
    let sentry_listener = match activated.sentry.take() {
        Some(l) => l,
        None => sentry::bind_or_create_sentry_listener(&cli.sentry_socket)?,
    };
    let sentry_arbiter = arbiter.clone();
    tokio::spawn(async move {
        if let Err(e) = sentry::run_sentry_triage_listener(sentry_listener, sentry_arbiter).await {
            error!("Sentry emergency triage socket failed: {}", e);
        }
    });

    // 3. SCM_RIGHTS Zero-Copy FD Server (FD 6)
    let fd_listener = match activated.fd_server.take() {
        Some(l) => l,
        None => fd_server::bind_or_create_fd_listener(
            cli.fd_socket
                .to_str()
                .unwrap_or(fd_server::DEFAULT_FD_SOCKET_PATH),
        )?,
    };
    tokio::spawn(async move {
        if let Err(e) = fd_server::run_fd_server(fd_listener).await {
            error!("SCM_RIGHTS FD server encountered an error: {}", e);
        }
    });

    // 4. HTTP Gateway Router & Server (FD 5 or standalone bind)
    let api_token = creds::resolve_secret("INFERENCED_API_TOKEN", "gateway_api_token");
    if api_token.is_some() {
        info!("Gateway API token configured via systemd-creds ($CREDENTIALS_DIRECTORY)");
    }
    let state = Arc::new(AppState {
        arbiter: arbiter.clone(),
        preempt: preempt.clone(),
        api_token,
    });
    let app = build_gateway_router(state);

    let gateway_listener = match activated.gateway.take() {
        Some(l) => l,
        None => {
            info!("Binding HTTP gateway to {}", cli.bind);
            GatewayListener::Tcp(TcpListener::bind(cli.bind).await?)
        }
    };

    notify::notify_systemd_ready();

    // Systemd Watchdog keepalive loop (pings every 10s for WatchdogSec=30s)
    let watchdog_task = tokio::spawn(async {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            notify::notify_systemd_watchdog();
        }
    });

    serve_gateway(gateway_listener, app, shutdown_signal()).await?;

    watchdog_task.abort();
    notify::notify_systemd_stopping();
    info!("systemd-inferenced daemon terminated cleanly.");
    Ok(())
}

async fn shutdown_signal() {
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("failed to install SIGTERM handler");

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Termination signal SIGINT received, shutting down gracefully...");
        }
        _ = sigterm.recv() => {
            info!("Termination signal SIGTERM from systemd received, shutting down gracefully...");
        }
    }
}
