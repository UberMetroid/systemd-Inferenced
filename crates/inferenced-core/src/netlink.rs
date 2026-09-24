use crate::error::{Error, Result};
use rustix::fd::{FromRawFd, OwnedFd};
use std::collections::HashMap;
use std::os::raw::c_int;

/// Parsed kernel device event from Linux Netlink KOBJECT_UEVENT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uevent {
    pub action: String,
    pub devpath: String,
    pub subsystem: String,
    pub devname: Option<String>,
    pub properties: HashMap<String, String>,
}

impl Uevent {
    /// Parse a raw netlink uevent buffer (NUL-delimited strings).
    pub fn parse(buf: &[u8]) -> Option<Self> {
        let text = std::str::from_utf8(buf).ok()?;
        let mut lines = text.split('\0').filter(|s| !s.is_empty());
        let header = lines.next()?; // e.g. "add@/devices/.../drm/renderD128"
        let (action_str, devpath_str) = header.split_once('@')?;

        let mut properties = HashMap::new();
        let mut subsystem = String::new();
        let mut devname = None;

        for prop in lines {
            if let Some((k, v)) = prop.split_once('=') {
                match k {
                    "SUBSYSTEM" => subsystem = v.to_string(),
                    "DEVNAME" => devname = Some(v.to_string()),
                    _ => {}
                }
                properties.insert(k.to_string(), v.to_string());
            }
        }

        if subsystem.is_empty() {
            if let Some(sub) = properties.get("SUBSYSTEM") {
                subsystem = sub.clone();
            }
        }

        Some(Self {
            action: action_str.to_string(),
            devpath: devpath_str.to_string(),
            subsystem,
            devname,
            properties,
        })
    }

    /// Returns true if this event corresponds to an AI compute plane (DRM GPU, NPU, Hailo).
    pub fn is_compute_device(&self) -> bool {
        self.subsystem == "drm"
            || self.subsystem == "accel"
            || (self.subsystem == "misc" && self.devname.as_deref().map_or(false, |n| n.contains("hailo")))
            || self.subsystem == "kfd"
    }

    /// Returns true if the event represents an ASIC reset or driver timeout event.
    pub fn is_reset_event(&self) -> bool {
        self.action == "change"
            && self.properties.get("RESET").map_or(false, |v| v == "1")
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SockAddrNl {
    pub nl_family: u16,
    pub nl_pad: u16,
    pub nl_pid: u32,
    pub nl_groups: u32,
}

/// Linux kernel socket constants: AF_NETLINK = 16, SOCK_RAW = 3, NETLINK_KOBJECT_UEVENT = 15.
const AF_NETLINK: c_int = 16;
const SOCK_RAW: c_int = 3;
const SOCK_NONBLOCK: c_int = 0x800;
const SOCK_CLOEXEC: c_int = 0x80000;
const NETLINK_KOBJECT_UEVENT: c_int = 15;

/// Open and bind a non-blocking Netlink KOBJECT_UEVENT multicast listener socket.
pub fn open_uevent_socket() -> Result<OwnedFd> {
    unsafe {
        // Syscall socket(AF_NETLINK, SOCK_RAW | SOCK_NONBLOCK | SOCK_CLOEXEC, NETLINK_KOBJECT_UEVENT)
        let fd_raw = libc_syscall_socket(
            AF_NETLINK,
            SOCK_RAW | SOCK_NONBLOCK | SOCK_CLOEXEC,
            NETLINK_KOBJECT_UEVENT,
        );
        if fd_raw < 0 {
            return Err(Error::Systemd("Failed to create AF_NETLINK socket".into()));
        }

        let addr = SockAddrNl {
            nl_family: AF_NETLINK as u16,
            nl_pad: 0,
            nl_pid: 0,       // Let kernel assign port ID
            nl_groups: 1,    // Multicast group 1: kernel uevents
        };

        let res = libc_syscall_bind(
            fd_raw,
            &addr as *const _ as *const _,
            std::mem::size_of::<SockAddrNl>() as u32,
        );

        if res < 0 {
            let _ = libc_syscall_close(fd_raw);
            return Err(Error::Systemd("Failed to bind netlink socket to group 1".into()));
        }

        Ok(OwnedFd::from_raw_fd(fd_raw))
    }
}

/// Direct Linux syscall wrappers (pure Rust, zero C runtime dependencies).
#[inline(always)]
unsafe fn libc_syscall_socket(domain: c_int, type_: c_int, protocol: c_int) -> c_int {
    std::arch::asm!(
        "syscall",
        inlateout("rax") 41isize => _, // SYS_socket = 41 on x86_64
        in("rdi") domain,
        in("rsi") type_,
        in("rdx") protocol,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    let ret: c_int;
    std::arch::asm!("mov {:e}, eax", out(reg) ret);
    ret
}

#[inline(always)]
unsafe fn libc_syscall_bind(sockfd: c_int, addr: *const std::ffi::c_void, addrlen: u32) -> c_int {
    std::arch::asm!(
        "syscall",
        inlateout("rax") 49isize => _, // SYS_bind = 49 on x86_64
        in("rdi") sockfd,
        in("rsi") addr,
        in("rdx") addrlen,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    let ret: c_int;
    std::arch::asm!("mov {:e}, eax", out(reg) ret);
    ret
}

#[inline(always)]
unsafe fn libc_syscall_close(fd: c_int) -> c_int {
    std::arch::asm!(
        "syscall",
        inlateout("rax") 3isize => _, // SYS_close = 3 on x86_64
        in("rdi") fd,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    let ret: c_int;
    std::arch::asm!("mov {:e}, eax", out(reg) ret);
    ret
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustix::fd::AsRawFd;

    #[test]
    fn test_parse_drm_add_uevent() {
        let raw = b"add@/devices/pci0000:00/0000:00:01.0/drm/renderD128\0ACTION=add\0DEVPATH=/devices/pci0000:00/0000:00:01.0/drm/renderD128\0SUBSYSTEM=drm\0DEVNAME=/dev/dri/renderD128\0MAJOR=226\0MINOR=128\0";
        let uevent = Uevent::parse(raw).unwrap();
        assert_eq!(uevent.action, "add");
        assert_eq!(uevent.subsystem, "drm");
        assert_eq!(uevent.devname.as_deref(), Some("/dev/dri/renderD128"));
        assert!(uevent.is_compute_device());
        assert!(!uevent.is_reset_event());
    }

    #[test]
    fn test_parse_reset_event() {
        let raw = b"change@/devices/pci0000:00/0000:00:01.0/drm/card0\0ACTION=change\0SUBSYSTEM=drm\0RESET=1\0";
        let uevent = Uevent::parse(raw).unwrap();
        assert_eq!(uevent.action, "change");
        assert!(uevent.is_reset_event());
    }

    #[test]
    fn test_open_uevent_socket_lifecycle() {
        // Will succeed if user has network/netlink permissions (or CAP_NET_ADMIN / standard user)
        if let Ok(fd) = open_uevent_socket() {
            assert!(fd.as_raw_fd() >= 0);
        }
    }
}
