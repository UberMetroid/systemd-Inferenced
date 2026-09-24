use inferenced_core::freezer::{freeze_cgroup, is_cgroup_frozen, thaw_cgroup};
use std::fs::{self, File};
use tempfile::tempdir;

#[test]
fn test_cgroup_freeze_and_thaw_file_operations() {
    let dir = tempdir().expect("tempdir");
    let freeze_file = dir.path().join("cgroup.freeze");
    File::create(&freeze_file).expect("create mock cgroup.freeze");

    // Freeze
    freeze_cgroup(dir.path()).expect("freeze should succeed");
    let frozen_state = is_cgroup_frozen(dir.path()).expect("read freeze state");
    assert!(frozen_state, "cgroup should be reported as frozen");
    let content = fs::read_to_string(&freeze_file).expect("read content");
    assert_eq!(content.trim(), "1");

    // Thaw
    thaw_cgroup(dir.path()).expect("thaw should succeed");
    let thawed_state = is_cgroup_frozen(dir.path()).expect("read thawed state");
    assert!(!thawed_state, "cgroup should be reported as thawed");
    let content = fs::read_to_string(&freeze_file).expect("read content");
    assert_eq!(content.trim(), "0");
}

#[test]
fn test_freezer_nonexistent_path_returns_error() {
    let result = freeze_cgroup("/nonexistent/sys/fs/cgroup/missing.slice");
    assert!(result.is_err(), "Nonexistent path should return an Error");
}
