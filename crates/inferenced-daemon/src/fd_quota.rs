//! Per-listener memfd allocation quota for the SCM_RIGHTS handoff server.
//!
//! The quota is per-listener and tracks bytes currently reserved by
//! in-flight memfd allocations. A request that would push the total
//! past the ceiling is rejected with `Quota exceeded` rather than
//! silently OOMing the daemon. Bytes are released when the SCM_RIGHTS
//! send fails; on success the kernel cmsg buffer inherits the FD and
//! the reservation stays consumed.

use std::sync::atomic::{AtomicU64, Ordering};

/// Hard ceiling for any single memfd allocation request. 16 GiB matches
/// the largest reasonable model shard; anything larger is almost
/// certainly a mistake or an attack.
pub const MAX_MEMFD_BYTES: u64 = 16 * 1024 * 1024 * 1024;

pub struct FdQuota {
    allocated_bytes: AtomicU64,
    ceiling_bytes: u64,
}

impl FdQuota {
    pub fn new(ceiling_bytes: u64) -> Self {
        Self {
            allocated_bytes: AtomicU64::new(0),
            ceiling_bytes,
        }
    }

    /// Compute half of the host's total RAM, falling back to a
    /// conservative default if `/proc/meminfo` is unreadable.
    pub fn from_half_of_total_ram() -> Self {
        let total = std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("MemTotal:"))
                    .and_then(|l| l.split_whitespace().nth(1))
                    .and_then(|n| n.parse::<u64>().ok())
            })
            .map(|kb| kb * 1024)
            .unwrap_or(32u64 * 1024 * 1024 * 1024);
        Self::new(total / 2)
    }

    /// Try to reserve `bytes` from the quota. On success returns the
    /// number of bytes reserved (which the caller must later pass to
    /// `release`). On failure returns the current allocation.
    pub fn try_reserve(&self, bytes: u64) -> Result<u64, u64> {
        let mut current = self.allocated_bytes.load(Ordering::Relaxed);
        loop {
            let next = current.saturating_add(bytes);
            if next > self.ceiling_bytes {
                return Err(current);
            }
            match self.allocated_bytes.compare_exchange(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(bytes),
                Err(actual) => current = actual,
            }
        }
    }

    /// Release a previously-reserved reservation. Clamped to the
    /// current allocation so it can never underflow.
    pub fn release(&self, amount: u64) {
        let current = self.allocated_bytes.load(Ordering::Relaxed);
        self.allocated_bytes
            .fetch_sub(amount.min(current), Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quota_reserve_under_ceiling_succeeds() {
        let q = FdQuota::new(1000);
        assert_eq!(q.try_reserve(400).unwrap(), 400);
        assert_eq!(q.try_reserve(500).unwrap(), 500);
    }

    #[test]
    fn test_quota_reserve_over_ceiling_fails() {
        let q = FdQuota::new(1000);
        assert_eq!(q.try_reserve(800).unwrap(), 800);
        // Now 800/1000 used. Adding 300 must fail and report current.
        assert_eq!(q.try_reserve(300).unwrap_err(), 800);
    }

    #[test]
    fn test_quota_release_restores_capacity() {
        let q = FdQuota::new(1000);
        let reserved = q.try_reserve(900).unwrap();
        assert!(q.try_reserve(200).is_err());
        q.release(reserved);
        // After release the quota is back to 0 allocated; reserve
        // succeeds and returns the requested amount.
        assert_eq!(q.try_reserve(200).unwrap(), 200);
    }

    #[test]
    fn test_quota_release_clamps_to_zero() {
        let q = FdQuota::new(1000);
        let reserved = q.try_reserve(500).unwrap();
        q.release(reserved);
        // Clamp: even if amount > allocated, must not underflow.
        q.release(10_000);
        assert_eq!(q.try_reserve(1000).unwrap(), 1000);
    }
}
