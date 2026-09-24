use crate::fd_lease::FdLease;
use crate::paging::{MemfdPaging, ZswapMetrics};
use rustix::fs::{fcntl_get_seals, seek, SeekFrom};
use rustix::io::read;
use rustix::mm::{mmap_anonymous, munmap, MapFlags, ProtFlags};
use std::os::unix::net::UnixStream;
use std::ptr::null_mut;
use tempfile::tempdir;

#[test]
fn test_create_sealed_model_and_seals() {
    let data = b"weights-tensor-model-test-data-12345";
    let fd = MemfdPaging::create_sealed_model("model_weights", data).unwrap();

    let seals = fcntl_get_seals(&fd).unwrap();
    assert!(seals.contains(rustix::fs::SealFlags::SEAL));
    assert!(seals.contains(rustix::fs::SealFlags::GROW));
    assert!(seals.contains(rustix::fs::SealFlags::SHRINK));
    assert!(seals.contains(rustix::fs::SealFlags::WRITE));

    seek(&fd, SeekFrom::Start(0)).unwrap();
    let mut buf = vec![0u8; data.len()];
    let n = read(&fd, &mut buf).unwrap();
    assert_eq!(n, data.len());
    assert_eq!(&buf, data);
}

#[test]
fn test_send_and_recv_model_fd() {
    let (s1, s2) = UnixStream::pair().unwrap();
    let model_data = b"neural-network-layer-weights";
    let model_id = "test-model-v1";

    let memfd = MemfdPaging::create_sealed_model("shared_model", model_data).unwrap();

    let sent = MemfdPaging::send_model_fd(&s1, &memfd, model_id).unwrap();
    assert_eq!(sent, model_id.len());

    let (recv_id, recv_fd) = MemfdPaging::recv_model_fd(&s2).unwrap();
    assert_eq!(recv_id, model_id);

    seek(&recv_fd, SeekFrom::Start(0)).unwrap();
    let mut buf = vec![0u8; model_data.len()];
    let n = read(&recv_fd, &mut buf).unwrap();
    assert_eq!(n, model_data.len());
    assert_eq!(&buf, model_data);
}

#[test]
fn test_reclaim_and_prefetch_pages() {
    let len = 4096 * 8; // 32KB
    let addr = unsafe {
        mmap_anonymous(
            null_mut(),
            len,
            ProtFlags::READ | ProtFlags::WRITE,
            MapFlags::PRIVATE,
        )
    }
    .unwrap();

    unsafe {
        std::ptr::write_bytes(addr, 0xAA, len);
    }

    assert!(MemfdPaging::advise_sequential(addr, len).is_ok());
    assert!(MemfdPaging::prefetch_pages(addr, len).is_ok());
    let _ = MemfdPaging::advise_hugepages(addr, len);
    assert!(MemfdPaging::advise_random(addr, len).is_ok());
    assert!(MemfdPaging::reclaim_pages(addr, len).is_ok());

    unsafe {
        munmap(addr, len).unwrap();
    }
}

#[test]
fn test_zswap_metrics_parse_and_read() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("enabled"), "Y\n").unwrap();
    std::fs::write(dir.path().join("compressor"), "zstd\n").unwrap();
    std::fs::write(dir.path().join("max_pool_percent"), "25\n").unwrap();
    std::fs::write(dir.path().join("accept_threshold_percent"), "85\n").unwrap();

    let mock_meminfo = "MemTotal:       32768 kB\nZswap:           1024 kB\nZswapped:        4096 kB\n";
    let metrics = ZswapMetrics::parse_metrics(mock_meminfo, dir.path().to_str().unwrap());

    assert!(metrics.enabled);
    assert_eq!(metrics.compressor, "zstd");
    assert_eq!(metrics.zswap_bytes, 1024 * 1024);
    assert_eq!(metrics.zswapped_bytes, 4096 * 1024);
    assert_eq!(metrics.max_pool_percent, 25);
    assert_eq!(metrics.accept_threshold_percent, 85);

    // Also verify reading live current metrics does not panic
    let current = MemfdPaging::read_zswap_metrics();
    assert!(!current.compressor.is_empty());
}

#[test]
fn test_fd_lease_wrapper_methods() {
    let data = b"fd-lease-wrapper-test";
    let fd = FdLease::create_sealed("wrapper_test", data.len() as u64, Some(data)).unwrap();

    let (s1, s2) = UnixStream::pair().unwrap();
    let sent = FdLease::send_fd(&s1, &fd, b"header").unwrap();
    assert_eq!(sent, 6);

    let mut buf = vec![0u8; 16];
    let (n, opt_fd) = FdLease::recv_fd(&s2, &mut buf).unwrap();
    assert_eq!(n, 6);
    assert!(opt_fd.is_some());
}
