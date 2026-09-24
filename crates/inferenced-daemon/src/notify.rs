use std::os::unix::net::UnixDatagram;
use tracing::info;

pub fn notify_systemd_ready() {
    if let Ok(socket_path) = std::env::var("NOTIFY_SOCKET") {
        if let Ok(sock) = UnixDatagram::unbound() {
            let _ = sock.send_to(b"READY=1\nSTATUS=Hardware planes active\n", socket_path);
            info!("Sent SD_NOTIFY READY=1 to systemd");
        }
    }
}
