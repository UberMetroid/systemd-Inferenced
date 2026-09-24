use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixDatagram};
use tracing::info;

/// Notify systemd that initialization has completed and service is ready
pub fn notify_systemd_ready() {
    notify_systemd("READY=1\nSTATUS=Hardware planes active\n");
    info!("Sent SD_NOTIFY READY=1 to systemd");
}

/// Update systemd service status text shown in systemctl status
#[allow(dead_code)]
pub fn notify_systemd_status(status: &str) {
    let msg = format!("STATUS={}\n", status);
    notify_systemd(&msg);
}

/// Notify systemd that service is stopping
pub fn notify_systemd_stopping() {
    notify_systemd("STOPPING=1\nSTATUS=Shutting down cleanly\n");
}

/// Keepalive ping for systemd WatchdogSec
#[allow(dead_code)]
pub fn notify_systemd_watchdog() {
    notify_systemd("WATCHDOG=1\n");
}

fn notify_systemd(state: &str) {
    let socket_path = match std::env::var("NOTIFY_SOCKET") {
        Ok(path) => path,
        Err(_) => return,
    };

    let sock = match UnixDatagram::unbound() {
        Ok(s) => s,
        Err(_) => return,
    };

    // Systemd supports abstract socket namespace indicated by leading '@'
    let addr_res = if let Some(stripped) = socket_path.strip_prefix('@') {
        SocketAddr::from_abstract_name(stripped.as_bytes())
    } else {
        SocketAddr::from_pathname(&socket_path)
    };

    if let Ok(addr) = addr_res {
        let _ = sock.send_to_addr(state.as_bytes(), &addr);
    }
}
