use rustix::fs::{fcntl_getfd, fcntl_setfd, FdFlags};
use rustix::net::{
    sendto_unix, socket, AddressFamily, SendFlags, SocketAddrUnix, SocketType,
};
use std::env;
use std::io;
use std::path::Path;
use tracing::{info, warn};

/// Notify systemd that initialization has completed and service is ready
pub fn notify_systemd_ready() {
    if let Err(e) = send_notification("READY=1\nSTATUS=Hardware planes active\n") {
        warn!("Failed to send SD_NOTIFY READY=1: {}", e);
    } else {
        info!("Sent SD_NOTIFY READY=1 to systemd");
    }
}

/// Update systemd service status text shown in systemctl status
#[allow(dead_code)]
pub fn notify_systemd_status(status: &str) {
    let msg = format!("STATUS={}\n", status);
    if let Err(e) = send_notification(&msg) {
        warn!("Failed to send SD_NOTIFY status: {}", e);
    }
}

/// Keepalive ping for systemd WatchdogSec
#[allow(dead_code)]
pub fn notify_systemd_watchdog() {
    if let Err(e) = send_notification("WATCHDOG=1\n") {
        warn!("Failed to send SD_NOTIFY WATCHDOG=1: {}", e);
    }
}

/// Notify systemd that service is stopping
pub fn notify_systemd_stopping() {
    if let Err(e) = send_notification("STOPPING=1\nSTATUS=Shutting down cleanly\n") {
        warn!("Failed to send SD_NOTIFY STOPPING=1: {}", e);
    } else {
        info!("Sent SD_NOTIFY STOPPING=1 to systemd");
    }
}

/// Send a notification string to the socket defined in $NOTIFY_SOCKET
pub fn send_notification(state: &str) -> io::Result<usize> {
    let socket_path = match env::var("NOTIFY_SOCKET") {
        Ok(path) if !path.is_empty() => path,
        _ => return Ok(0),
    };
    send_notification_to(&socket_path, state)
}

/// Send notification payload directly to specified socket (abstract or filesystem)
pub fn send_notification_to(socket_path: &str, state: &str) -> io::Result<usize> {
    let addr = if let Some(stripped) = socket_path.strip_prefix('@') {
        SocketAddrUnix::new_abstract_name(stripped.as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?
    } else {
        SocketAddrUnix::new(Path::new(socket_path))
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?
    };

    let sock = socket(AddressFamily::UNIX, SocketType::DGRAM, None)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    if let Ok(flags) = fcntl_getfd(&sock) {
        let _ = fcntl_setfd(&sock, flags | FdFlags::CLOEXEC);
    }

    sendto_unix(&sock, state.as_bytes(), SendFlags::empty(), &addr)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::net::{SocketAddr, UnixDatagram};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(100);

    fn create_test_abstract_socket() -> (UnixDatagram, String) {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let name = format!("inferenced_notify_mod_test_{}_{}", std::process::id(), id);
        let addr = SocketAddr::from_abstract_name(name.as_bytes()).expect("abstract addr");
        let receiver = UnixDatagram::bind_addr(&addr).expect("bind abstract");
        (receiver, format!("@{}", name))
    }

    #[test]
    fn test_abstract_notify_all_messages() {
        let (receiver, addr_str) = create_test_abstract_socket();

        // 1. Ready
        let ready_msg = "READY=1\nSTATUS=Hardware planes active\n";
        let sent = send_notification_to(&addr_str, ready_msg).expect("send ready");
        assert_eq!(sent, ready_msg.len());

        let mut buf = [0u8; 512];
        let (n, _) = receiver.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..n], ready_msg.as_bytes());

        // 2. Status
        let status_msg = "STATUS=Planes active: 3\n";
        send_notification_to(&addr_str, status_msg).unwrap();
        let (n, _) = receiver.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..n], status_msg.as_bytes());

        // 3. Watchdog
        let wd_msg = "WATCHDOG=1\n";
        send_notification_to(&addr_str, wd_msg).unwrap();
        let (n, _) = receiver.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..n], wd_msg.as_bytes());

        // 4. Stopping
        let stop_msg = "STOPPING=1\nSTATUS=Shutting down cleanly\n";
        send_notification_to(&addr_str, stop_msg).unwrap();
        let (n, _) = receiver.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..n], stop_msg.as_bytes());
    }

    #[test]
    fn test_notify_missing_env_noop() {
        env::remove_var("NOTIFY_SOCKET");
        let res = send_notification("READY=1\n");
        assert_eq!(res.unwrap(), 0);
    }
}
