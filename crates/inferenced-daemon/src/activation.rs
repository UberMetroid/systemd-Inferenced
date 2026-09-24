use anyhow::Result;
use rustix::fd::{FromRawFd, IntoRawFd, OwnedFd};
use rustix::fs::{fcntl_getfd, fcntl_setfd, FdFlags};
use rustix::net::{getsockname, SocketAddrAny};
use std::env;
use std::net::TcpListener as StdTcpListener;
use std::os::unix::net::UnixListener as StdUnixListener;
use tokio::net::{TcpListener, UnixListener};
use tracing::{info, warn};

pub const SD_LISTEN_FDS_START: i32 = 3;

#[derive(Default)]
pub struct ActivatedSockets {
    pub varlink: Option<UnixListener>,
    pub sentry: Option<UnixListener>,
    pub fd_server: Option<UnixListener>,
    pub gateway: Option<TcpListener>,
}

/// Pure Rust systemd socket activation parser adopting $LISTEN_FDS descriptors
pub fn check_and_adopt_sockets() -> Result<ActivatedSockets> {
    let mut sockets = ActivatedSockets::default();

    let listen_pid = env::var("LISTEN_PID").ok();
    let listen_fds = env::var("LISTEN_FDS").ok();

    let (Some(pid_str), Some(fds_str)) = (listen_pid, listen_fds) else {
        return Ok(sockets);
    };

    let pid: u32 = pid_str.parse().unwrap_or(0);
    if pid != std::process::id() {
        return Ok(sockets);
    }

    let fds_count: i32 = fds_str.parse().unwrap_or(0);
    if fds_count <= 0 {
        return Ok(sockets);
    }

    info!(
        "Adopting {} socket activation file descriptor(s) from systemd",
        fds_count
    );

    for i in 0..fds_count {
        let fd_raw = SD_LISTEN_FDS_START + i;
        let owned_fd = unsafe { OwnedFd::from_raw_fd(fd_raw) };

        // Ensure FD_CLOEXEC
        if let Ok(flags) = fcntl_getfd(&owned_fd) {
            let _ = fcntl_setfd(&owned_fd, flags | FdFlags::CLOEXEC);
        }

        let sock_addr = match getsockname(&owned_fd) {
            Ok(addr) => addr,
            Err(e) => {
                warn!("Failed to getsockname on fd {}: {}", fd_raw, e);
                continue;
            }
        };

        match sock_addr {
            SocketAddrAny::Unix(addr) => {
                let raw = owned_fd.into_raw_fd();
                let std_unix = unsafe { StdUnixListener::from_raw_fd(raw) };
                std_unix.set_nonblocking(true)?;
                let tokio_unix = UnixListener::from_std(std_unix)?;

                if let Some(path) = addr.path() {
                    let path_str = path.to_string_lossy();
                    if path_str.contains("inferenced1") || path_str.contains("varlink") {
                        info!("Adopted Varlink socket on fd {} ({})", fd_raw, path_str);
                        sockets.varlink = Some(tokio_unix);
                    } else if path_str.contains("sentry") {
                        info!("Adopted Sentry socket on fd {} ({})", fd_raw, path_str);
                        sockets.sentry = Some(tokio_unix);
                    } else if path_str.contains("fd") {
                        info!("Adopted FD passing socket on fd {} ({})", fd_raw, path_str);
                        sockets.fd_server = Some(tokio_unix);
                    } else if sockets.varlink.is_none() {
                        sockets.varlink = Some(tokio_unix);
                    } else if sockets.sentry.is_none() {
                        sockets.sentry = Some(tokio_unix);
                    }
                } else if sockets.varlink.is_none() {
                    sockets.varlink = Some(tokio_unix);
                } else if sockets.sentry.is_none() {
                    sockets.sentry = Some(tokio_unix);
                }
            }
            SocketAddrAny::V4(addr) => {
                let raw = owned_fd.into_raw_fd();
                let std_tcp = unsafe { StdTcpListener::from_raw_fd(raw) };
                std_tcp.set_nonblocking(true)?;
                let tokio_tcp = TcpListener::from_std(std_tcp)?;
                info!("Adopted IPv4 Gateway socket on fd {} ({})", fd_raw, addr);
                sockets.gateway = Some(tokio_tcp);
            }
            SocketAddrAny::V6(addr) => {
                let raw = owned_fd.into_raw_fd();
                let std_tcp = unsafe { StdTcpListener::from_raw_fd(raw) };
                std_tcp.set_nonblocking(true)?;
                let tokio_tcp = TcpListener::from_std(std_tcp)?;
                info!("Adopted IPv6 Gateway socket on fd {} ({})", fd_raw, addr);
                sockets.gateway = Some(tokio_tcp);
            }
            _ => {
                warn!("Unrecognized socket family on fd {}", fd_raw);
            }
        }
    }

    Ok(sockets)
}
