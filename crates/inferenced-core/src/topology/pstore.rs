use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;
use tracing::warn;

/// Post-mortem audit result extracted from Linux kernel pstore.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PstoreAudit {
    pub quarantined_drivers: Vec<String>,
    pub panic_count: usize,
    pub has_unarchived_pstore: bool,
}

impl PstoreAudit {
    /// Ingest and analyze crash logs from standard systemd-pstore locations.
    pub fn read_default() -> Self {
        let pstore_sys = Path::new("/sys/fs/pstore");
        let pstore_var = Path::new("/var/lib/systemd/pstore");
        Self::analyze_directories(&[pstore_sys, pstore_var])
    }

    /// Analyze provided directory paths for kernel crash signatures.
    pub fn analyze_directories(paths: &[&Path]) -> Self {
        let mut quarantined = HashSet::new();
        let mut panic_count = 0;
        let mut has_unarchived = false;

        for path in paths {
            if !path.exists() || !path.is_dir() {
                continue;
            }

            let entries = match fs::read_dir(path) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.starts_with("dmesg-") || file_name.starts_with("console-") {
                    if path.to_string_lossy().contains("/sys/fs/pstore") {
                        has_unarchived = true;
                    }
                    if let Ok(file) = File::open(entry.path()) {
                        panic_count += 1;
                        Self::scan_file_for_gpu_hangs(file, &mut quarantined);
                    }
                }
            }
        }

        if has_unarchived {
            warn!("systemd-pstore: unarchived crash logs in /sys/fs/pstore; EFI NVRAM space at risk");
        }

        let mut drivers: Vec<String> = quarantined.into_iter().collect();
        drivers.sort();

        Self {
            quarantined_drivers: drivers,
            panic_count,
            has_unarchived_pstore: has_unarchived,
        }
    }

    /// Stream-scan crash log line-by-line to prevent heap spikes on low-memory systems.
    fn scan_file_for_gpu_hangs(file: File, quarantined: &mut HashSet<String>) {
        let reader = BufReader::with_capacity(4096, file);
        for line in reader.lines().map_while(Result::ok) {
            if line.contains("amdgpu_job_timedout") || line.contains("amdgpu: GPU reset") {
                quarantined.insert("amdgpu".to_string());
            } else if line.contains("xe: GPU reset") || line.contains("xe 0000:") {
                quarantined.insert("xe".to_string());
            } else if line.contains("drm/i915: GPU HANG") || line.contains("i915: Resetting chip") {
                quarantined.insert("i915".to_string());
            } else if line.contains("NVRM: Xid") || line.contains("fallen off the bus") {
                quarantined.insert("nvidia".to_string());
            } else if line.contains("hailo: device reset failed") || line.contains("hailo0: timeout") {
                quarantined.insert("hailo".to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_pstore_empty_dir() {
        let dir = tempdir().unwrap();
        let audit = PstoreAudit::analyze_directories(&[dir.path()]);
        assert_eq!(audit.panic_count, 0);
        assert!(audit.quarantined_drivers.is_empty());
        assert!(!audit.has_unarchived_pstore);
    }

    #[test]
    fn test_pstore_detect_amdgpu_ring_timeout() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("dmesg-efi-101");
        let mut file = File::create(&log_path).unwrap();
        writeln!(file, "[  124.551020] [drm:amdgpu_job_timedout] *ERROR* ring gfx_0.0.0 timeout").unwrap();
        writeln!(file, "[  124.551030] amdgpu 0000:03:00.0: GPU reset begin!").unwrap();

        let audit = PstoreAudit::analyze_directories(&[dir.path()]);
        assert_eq!(audit.panic_count, 1);
        assert_eq!(audit.quarantined_drivers, vec!["amdgpu".to_string()]);
    }

    #[test]
    fn test_pstore_detect_hailo_timeout() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("dmesg-efi-202");
        let mut file = File::create(&log_path).unwrap();
        writeln!(file, "[   45.100] hailo: device reset failed for hailo0").unwrap();

        let audit = PstoreAudit::analyze_directories(&[dir.path()]);
        assert_eq!(audit.panic_count, 1);
        assert_eq!(audit.quarantined_drivers, vec!["hailo".to_string()]);
    }
}
