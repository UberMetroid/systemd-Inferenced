use crate::error::{Error, Result};
use rustix::fd::{AsFd, OwnedFd};
use rustix::fs::{fcntl_add_seals, ftruncate, memfd_create, seek, MemfdFlags, SealFlags, SeekFrom};
use rustix::io::write;
use rustix::net::{
    recvmsg, sendmsg, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags,
    SendAncillaryBuffer, SendAncillaryMessage, SendFlags,
};
use std::io::{IoSlice, IoSliceMut};

/// Creates an in-memory anonymous file descriptor sealed against
/// modification, suitable for zero-copy weight sharing via SCM_RIGHTS.
///
/// The full seal set `WRITE | SHRINK | GROW | SEAL` is applied in every
/// case. The previous version omitted `WRITE` when `initial_data` was
/// `None`, which gave an empty buffer asymmetric immutability versus a
/// populated buffer; callers can no longer rely on the seal contract
/// being uniform.
pub fn create_sealed_memfd(
    name: &str,
    size_bytes: u64,
    initial_data: Option<&[u8]>,
) -> Result<OwnedFd> {
    let fd = memfd_create(name, MemfdFlags::ALLOW_SEALING | MemfdFlags::CLOEXEC)
        .map_err(Error::SystemCall)?;

    ftruncate(&fd, size_bytes).map_err(Error::SystemCall)?;

    if let Some(data) = initial_data {
        let mut written = 0;
        while written < data.len() {
            let n = write(&fd, &data[written..]).map_err(Error::SystemCall)?;
            if n == 0 {
                break;
            }
            written += n;
        }
    }

    // Rewind file offset to start so readers sharing file description
    // do not encounter EOF.
    seek(&fd, SeekFrom::Start(0)).map_err(Error::SystemCall)?;

    let seals = SealFlags::SEAL | SealFlags::GROW | SealFlags::SHRINK | SealFlags::WRITE;
    fcntl_add_seals(&fd, seals).map_err(Error::SystemCall)?;

    Ok(fd)
}

/// Send a file descriptor and payload bytes over a Unix stream socket using SCM_RIGHTS.
pub fn send_fd_over_unix<S: AsFd, F: AsFd>(
    socket: S,
    fd_to_send: F,
    payload: &[u8],
) -> Result<usize> {
    let mut space = [0u8; rustix::cmsg_space!(ScmRights(1))];
    let mut ancillary_buf = SendAncillaryBuffer::new(&mut space);
    let fds = [fd_to_send.as_fd()];
    let pushed = ancillary_buf.push(SendAncillaryMessage::ScmRights(&fds));
    if !pushed {
        return Err(Error::Fd("Failed to push fd to ancillary buffer".into()));
    }

    let iov = [IoSlice::new(payload)];
    let sent = sendmsg(
        socket.as_fd(),
        &iov,
        &mut ancillary_buf,
        SendFlags::empty(),
    )
    .map_err(Error::SystemCall)?;

    Ok(sent)
}

/// Receive a file descriptor and payload bytes from a Unix stream
/// socket using SCM_RIGHTS.
///
/// Returns `Err(Error::Fd(...))` when the payload is exactly
/// `buf.len()` — the signal that `recvmsg` truncated the JSON header
/// and the FD attached to it is unreliable. Callers who want to
/// tolerate this should grow `buf` and retry, but the default is to
/// surface the truncation so the consumer never silently gets a
/// sealed FD paired with a malformed metadata blob.
pub fn recv_fd_from_unix<S: AsFd>(
    socket: S,
    buf: &mut [u8],
) -> Result<(usize, Option<OwnedFd>)> {
    let mut space = [0u8; rustix::cmsg_space!(ScmRights(1))];
    let mut ancillary_buf = RecvAncillaryBuffer::new(&mut space);
    let mut iov = [IoSliceMut::new(buf)];

    let msg = recvmsg(
        socket.as_fd(),
        &mut iov,
        &mut ancillary_buf,
        RecvFlags::CMSG_CLOEXEC,
    )
    .map_err(Error::SystemCall)?;

    if msg.bytes == buf.len() {
        // recvmsg would have written more if it had room; treat the
        // payload as truncated and drop the FD rather than handing a
        // sealed descriptor to the caller with broken metadata.
        let mut received_fd = None;
        for cmsg in ancillary_buf.drain() {
            if let RecvAncillaryMessage::ScmRights(fds) = cmsg {
                for owned in fds {
                    if received_fd.is_none() {
                        received_fd = Some(owned);
                    }
                }
            }
        }
        return Err(Error::Fd(format!(
            "recvmsg filled {} bytes; caller-supplied buffer too small",
            buf.len()
        )));
    }

    let mut received_fd = None;
    for cmsg in ancillary_buf.drain() {
        if let RecvAncillaryMessage::ScmRights(fds) = cmsg {
            for owned in fds {
                if received_fd.is_none() {
                    received_fd = Some(owned);
                }
            }
        }
    }

    Ok((msg.bytes, received_fd))
}

/// High-level wrapper for SCM_RIGHTS file descriptor leasing.
pub struct FdLease;

impl FdLease {
    /// Creates an in-memory anonymous file descriptor sealed against modification.
    pub fn create_sealed(name: &str, size_bytes: u64, initial_data: Option<&[u8]>) -> Result<OwnedFd> {
        create_sealed_memfd(name, size_bytes, initial_data)
    }

    /// Send a file descriptor over a Unix socket using SCM_RIGHTS.
    pub fn send_fd<S: AsFd, F: AsFd>(socket: S, fd: F, payload: &[u8]) -> Result<usize> {
        send_fd_over_unix(socket, fd, payload)
    }

    /// Receive a file descriptor over a Unix socket using SCM_RIGHTS.
    pub fn recv_fd<S: AsFd>(socket: S, buf: &mut [u8]) -> Result<(usize, Option<OwnedFd>)> {
        recv_fd_from_unix(socket, buf)
    }
}
