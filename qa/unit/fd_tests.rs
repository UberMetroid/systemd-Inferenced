use inferenced_core::fd_lease::{create_sealed_memfd, recv_fd_from_unix, send_fd_over_unix};
use rustix::fs::{fcntl_get_seals, seek, SealFlags, SeekFrom};
use rustix::io::{read, write};
use std::os::unix::net::UnixStream;

#[test]
fn test_create_sealed_memfd_success() {
    let data = b"heterogeneous-tensor-weights-0123456789";
    let fd = create_sealed_memfd("test_weights", data.len() as u64, Some(data))
        .expect("memfd_create should succeed");

    let seals = fcntl_get_seals(&fd).expect("get seals should succeed");
    assert!(seals.contains(SealFlags::SEAL));
    assert!(seals.contains(SealFlags::GROW));
    assert!(seals.contains(SealFlags::SHRINK));
    assert!(seals.contains(SealFlags::WRITE));
}

#[test]
fn test_send_and_recv_fd_over_unix() {
    let (s1, s2) = UnixStream::pair().expect("UnixStream pair");
    let test_payload = b"weight-descriptor-json";
    let test_data = b"zero-copy-neural-tensor-data";

    let memfd = create_sealed_memfd("shm_weights", test_data.len() as u64, Some(test_data))
        .expect("memfd should be created");

    let sent_bytes =
        send_fd_over_unix(&s1, &memfd, test_payload).expect("send_fd_over_unix should succeed");
    assert_eq!(sent_bytes, test_payload.len());

    let mut buf = vec![0u8; 128];
    let (recv_bytes, received_fd) =
        recv_fd_from_unix(&s2, &mut buf).expect("recv_fd_from_unix should succeed");
    assert_eq!(&buf[..recv_bytes], test_payload);

    let recv_fd = received_fd.expect("Should receive passed fd via SCM_RIGHTS");
    seek(&recv_fd, SeekFrom::Start(0)).expect("seek to start");
    let mut file_buf = vec![0u8; test_data.len()];
    let n = read(&recv_fd, &mut file_buf).expect("reading received memfd should succeed");
    assert_eq!(n, test_data.len());
    assert_eq!(&file_buf, test_data);
}

#[test]
fn test_memfd_seals_prevent_modification() {
    let data = b"immutable-constant-weights";
    let fd = create_sealed_memfd("immutable", data.len() as u64, Some(data)).unwrap();

    // Any attempt to write to the sealed memfd must fail with EPERM
    let write_res = write(&fd, b"corrupted-overwrite");
    assert!(write_res.is_err(), "Writing to sealed memfd must be prevented by kernel seals");
}

#[test]
fn test_memfd_zero_copy_multiple_receivers() {
    let (s1, r1) = UnixStream::pair().unwrap();
    let (s2, r2) = UnixStream::pair().unwrap();
    let data = b"shared-transformer-layer-0";

    let memfd = create_sealed_memfd("shared_layer", data.len() as u64, Some(data)).unwrap();

    // Send to worker 1
    send_fd_over_unix(&s1, &memfd, b"meta1").unwrap();
    // Send to worker 2
    send_fd_over_unix(&s2, &memfd, b"meta2").unwrap();

    let mut buf1 = [0u8; 16];
    let (_, fd1) = recv_fd_from_unix(&r1, &mut buf1).unwrap();
    let mut buf2 = [0u8; 16];
    let (_, fd2) = recv_fd_from_unix(&r2, &mut buf2).unwrap();

    let f1 = fd1.unwrap();
    let f2 = fd2.unwrap();

    let mut read1 = vec![0u8; data.len()];
    seek(&f1, SeekFrom::Start(0)).unwrap();
    read(&f1, &mut read1).unwrap();
    assert_eq!(&read1, data);

    let mut read2 = vec![0u8; data.len()];
    seek(&f2, SeekFrom::Start(0)).unwrap();
    read(&f2, &mut read2).unwrap();
    assert_eq!(&read2, data);
}

#[test]
fn test_memfd_large_tensor_transfer() {
    let (s1, s2) = UnixStream::pair().unwrap();
    let large_tensor = vec![0x42u8; 256 * 1024]; // 256 KB

    let memfd = create_sealed_memfd("large_tensor", large_tensor.len() as u64, Some(&large_tensor)).unwrap();
    send_fd_over_unix(&s1, &memfd, b"header-large").unwrap();

    let mut buf = [0u8; 32];
    let (_, recv_fd) = recv_fd_from_unix(&s2, &mut buf).unwrap();
    let fd = recv_fd.unwrap();

    seek(&fd, SeekFrom::Start(0)).unwrap();
    let mut read_buf = vec![0u8; large_tensor.len()];
    let mut total_read = 0;
    while total_read < large_tensor.len() {
        let n = read(&fd, &mut read_buf[total_read..]).unwrap();
        if n == 0 {
            break;
        }
        total_read += n;
    }
    assert_eq!(total_read, large_tensor.len());
    assert_eq!(read_buf[0], 0x42);
    assert_eq!(read_buf[large_tensor.len() - 1], 0x42);
}
