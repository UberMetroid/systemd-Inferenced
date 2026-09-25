use anyhow::Result;
use rustix::fd::{FromRawFd, IntoRawFd, OwnedFd};
use rustix::fs::{fcntl_getfd, fcntl_setfd, FdFlags};
use rustix::net::{getsockname, SocketAddrAny};
use std::env;
use std::fs;
use std::net::TcpListener as StdTcpListener;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::UnixListener as StdUnixListener;
use std::path::Path;
use tokio::net::{TcpListener, UnixListener};
use tracing::{info, warn};

pub const SD_LISTEN_FDS_START: i32 = 3;

/// Canonical socket basenames produced by `systemd-inferenced.socket`.
/// Used to classify adopted FDs by exact match rather than substring
/// containment, which previously mis-routed paths like
/// `/run/systemd-inferenced/gateway-fd.sock` into the FD handoff slot.
const SOCKET_VARLINK: &str = "io.syntrop.Inference1";
const SOCKET_SENTRY: &str = "sentry.sock";
const SOCKET_FD: &str = "fd.sock";
const SOCKET_GATEWAY_UNIX: &str = "gateway.sock";

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

/// Classify a Unix socket path into a target slot. Returns `None` when
/// the basename does not match any known socket name.
fn classify_unix_socket(path: &Path) -> Option<&'static str> {
    let name = path.file_name().and_then(|n| n.to_str())?;
    match name {
        SOCKET_VARLINK => Some("varlink"),
        SOCKET_SENTRY => Some("sentry"),
        SOCKET_FD => Some("fd"),
        SOCKET_GATEWAY_UNIX => Some("gateway-unix"),
        // Legacy aliases kept for backward compatibility with v0.1 socket
        // units; new units should use the canonical names above.
        "io.systemd.inferenced1" => Some("varlink"),
        _ => None,
    }
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
                let path = addr
                    .path()
                    .map(|p| Path::new(std::ffi::OsStr::from_bytes(p.to_bytes())).to_path_buf());
                let kind = path
                    .as_deref()
                    .and_then(classify_unix_socket);

                match kind {
                    Some("varlink") => {
                        info!("Adopted Varlink socket on fd {}", fd_raw);
                        sockets.varlink = Some(tokio_unix);
                    }
                    Some("sentry") => {
                        info!("Adopted Sentry socket on fd {}", fd_raw);
                        sockets.sentry = Some(tokio_unix);
                    }
                    Some("fd") => {
                        info!("Adopted FD server socket on fd {}", fd_raw);
                        sockets.fd_server = Some(tokio_unix);
                    }
                    Some("gateway-unix") => {
                        info!("Adopted Gateway Unix socket on fd {}", fd_raw);
                        sockets.gateway = Some(GatewayListener::Unix(tokio_unix));
                    }
                    // Positional fallback for unclassified sockets.
                    // Order matches `systemd-inferenced.socket`: FD 3 =
                    // varlink, FD 4 = sentry, FD 5 = gateway (TCP via the
                    // V4 branch above; if a Unix gateway is used it lands
                    // here), FD 6 = fd_server.
                    // Fails closed: leaves the slot empty and warns so
                    // the operator notices the misconfiguration.
                    _ => {
                        let slot = match i {
                            0 => "varlink",
                            1 => "sentry",
                            2 => "gateway",
                            3 => "fd",
                            _ => "",
                        };
                        if path.is_some() {
                            warn!(
                                "Unclassified Unix socket on fd {} (path {:?}); expected one of {:?}",
                                fd_raw, path, [SOCKET_VARLINK, SOCKET_SENTRY, SOCKET_FD, SOCKET_GATEWAY_UNIX]
                            );
                        }
                        match slot {
                            "varlink" if sockets.varlink.is_none() => {
                                info!("Adopted Varlink socket by index 0 (fd {})", fd_raw);
                                sockets.varlink = Some(tokio_unix);
                            }
                            "sentry" if sockets.sentry.is_none() => {
                                info!("Adopted Sentry socket by index 1 (fd {})", fd_raw);
                                sockets.sentry = Some(tokio_unix);
                            }
                            "gateway" if sockets.gateway.is_none() => {
                                info!("Adopted Gateway Unix socket by index 2 (fd {})", fd_raw);
                                sockets.gateway = Some(GatewayListener::Unix(tokio_unix));
                            }
                            "fd" if sockets.fd_server.is_none() => {
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
    #[cfg(unix)]
    let _ = fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o660));
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
        #[cfg(unix)]
        assert_eq!(std::os::unix::fs::PermissionsExt::mode(&fs::metadata(&sock).unwrap().permissions()) & 0o777, 0o660);
    }
}
