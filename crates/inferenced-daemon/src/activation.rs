use anyhow::Result;
use rustix::fd::{FromRawFd, IntoRawFd, OwnedFd};
use rustix::fs::{fcntl_getfd, fcntl_setfd, FdFlags};
use rustix::net::{getsockname, SocketAddrAny};
use std::env;
use std::fs;
use std::net::TcpListener as StdTcpListener;
use std::os::unix::net::UnixListener as StdUnixListener;
use std::path::Path;
use tokio::net::{TcpListener, UnixListener};
use tracing::{info, warn};

pub const SD_LISTEN_FDS_START: i32 = 3;

pub enum GatewayListener {
    Tcp(TcpListener),
    Unix(UnixListener),
}

#[derive(Default)]
pub struct ActivatedSockets {
    pub varlink: Option<UnixListener>,
    pub sentry: Option<UnixListener>,
    pub fd_server: Option<UnixListener>,
    pub gateway: Option<GatewayListener>,
}

/// Checks whether the current process was activated by systemd
pub fn is_socket_activated() -> bool {
    let pid_matches = env::var("LISTEN_PID")
        .ok()
        .and_then(|p| p.parse::<u32>().ok())
        .map(|pid| pid == std::process::id())
        .unwrap_or(false);

    let has_fds = env::var("LISTEN_FDS")
        .ok()
        .and_then(|f| f.parse::<i32>().ok())
        .map(|count| count > 0)
        .unwrap_or(false);

    pid_matches && has_fds
}

/// Pure Rust systemd socket activation parser adopting $LISTEN_FDS descriptors
pub fn check_and_adopt_sockets() -> Result<ActivatedSockets> {
    let mut sockets = ActivatedSockets::default();

    if !is_socket_activated() {
        return Ok(sockets);
    }

    let fds_count: i32 = env::var("LISTEN_FDS")
        .unwrap_or_default()
        .parse()
        .unwrap_or(0);

    info!(
        "Adopting {} socket activation descriptor(s) from systemd",
        fds_count
    );

    for i in 0..fds_count {
        let fd_raw = SD_LISTEN_FDS_START + i;
        let owned_fd = unsafe { OwnedFd::from_raw_fd(fd_raw) };

        if let Ok(flags) = fcntl_getfd(&owned_fd) {
            let _ = fcntl_setfd(&owned_fd, flags | FdFlags::CLOEXEC);
        }

        let sock_addr = match getsockname(&owned_fd) {
            Ok(addr) => addr,
            Err(e) => {
                warn!("Failed getsockname on fd {}: {}", fd_raw, e);
                continue;
            }
        };

        match sock_addr {
            SocketAddrAny::Unix(addr) => {
                let raw = owned_fd.into_raw_fd();
                let std_unix = unsafe { StdUnixListener::from_raw_fd(raw) };
                std_unix.set_nonblocking(true)?;
                let tokio_unix = UnixListener::from_std(std_unix)?;
                let path_str = addr.path().map(|p| p.to_string_lossy().into_owned());

                match path_str.as_deref() {
                    Some(p) if p.contains("inferenced1") || p.contains("varlink") => {
                        info!("Adopted Varlink socket on fd {} ({})", fd_raw, p);
                        sockets.varlink = Some(tokio_unix);
                    }
                    Some(p) if p.contains("sentry") => {
                        info!("Adopted Sentry socket on fd {} ({})", fd_raw, p);
                        sockets.sentry = Some(tokio_unix);
                    }
                    Some(p) if p.contains("fd") => {
                        info!("Adopted FD server socket on fd {} ({})", fd_raw, p);
                        sockets.fd_server = Some(tokio_unix);
                    }
                    Some(p) if p.contains("io.sock") || p.contains("gateway") => {
                        info!("Adopted Gateway Unix socket on fd {} ({})", fd_raw, p);
                        sockets.gateway = Some(GatewayListener::Unix(tokio_unix));
                    }
                    _ => {
                        // Positional fallback: FD 3 = Varlink, FD 4 = Sentry, FD 5 = Gateway
                        match i {
                            0 if sockets.varlink.is_none() => {
                                info!("Adopted Varlink socket by index 0 (fd {})", fd_raw);
                                sockets.varlink = Some(tokio_unix);
                            }
                            1 if sockets.sentry.is_none() => {
                                info!("Adopted Sentry socket by index 1 (fd {})", fd_raw);
                                sockets.sentry = Some(tokio_unix);
                            }
                            2 if sockets.gateway.is_none() => {
                                info!("Adopted Gateway Unix socket by index 2 (fd {})", fd_raw);
                                sockets.gateway = Some(GatewayListener::Unix(tokio_unix));
                            }
                            3 if sockets.fd_server.is_none() => {
                                info!("Adopted FD server socket by index 3 (fd {})", fd_raw);
                                sockets.fd_server = Some(tokio_unix);
                            }
                            _ => warn!("Unassigned Unix socket on fd {}", fd_raw),
                        }
                    }
                }
            }
            SocketAddrAny::V4(addr) => {
                let raw = owned_fd.into_raw_fd();
                let std_tcp = unsafe { StdTcpListener::from_raw_fd(raw) };
                std_tcp.set_nonblocking(true)?;
                let tokio_tcp = TcpListener::from_std(std_tcp)?;
                info!("Adopted IPv4 Gateway socket on fd {} ({})", fd_raw, addr);
                sockets.gateway = Some(GatewayListener::Tcp(tokio_tcp));
            }
            SocketAddrAny::V6(addr) => {
                let raw = owned_fd.into_raw_fd();
                let std_tcp = unsafe { StdTcpListener::from_raw_fd(raw) };
                std_tcp.set_nonblocking(true)?;
                let tokio_tcp = TcpListener::from_std(std_tcp)?;
                info!("Adopted IPv6 Gateway socket on fd {} ({})", fd_raw, addr);
                sockets.gateway = Some(GatewayListener::Tcp(tokio_tcp));
            }
            _ => warn!("Unrecognized socket family on fd {}", fd_raw),
        }
    }

    Ok(sockets)
}

/// Fallback helper to bind standalone Unix socket cleanly
#[allow(dead_code)]
pub fn bind_standalone_unix(path: &Path) -> Result<UnixListener> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    let listener = UnixListener::bind(path)?;
    info!("Bound standalone Unix socket on {:?}", path);
    Ok(listener)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener as StdUnixListener;
    use tempfile::tempdir;

    #[test]
    fn test_is_socket_activated_lifecycle() {
        env::remove_var("LISTEN_PID");
        env::remove_var("LISTEN_FDS");
        assert!(!is_socket_activated());

        env::set_var("LISTEN_PID", (std::process::id() + 9999).to_string());
        env::set_var("LISTEN_FDS", "3");
        assert!(!is_socket_activated());

        env::set_var("LISTEN_PID", std::process::id().to_string());
        env::set_var("LISTEN_FDS", "3");
        assert!(is_socket_activated());

        env::remove_var("LISTEN_PID");
        env::remove_var("LISTEN_FDS");
    }

    #[tokio::test]
    async fn test_bind_standalone_unix_cleans_existing() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test_standalone.sock");
        let _l1 = StdUnixListener::bind(&sock).unwrap();
        assert!(sock.exists());

        let l2 = bind_standalone_unix(&sock);
        assert!(l2.is_ok());
    }
}
