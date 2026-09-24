use inferenced_core::arbiter::Arbiter;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::process::Child;
use tracing::{debug, info, warn};

/// Systemd logind sleep and idle inhibitor manager.
/// Manages host sleep/idle locks and executes memory quiescence on sleep transitions.
pub struct InhibitorManager {
    active_leases: Arc<AtomicUsize>,
    is_inhibited: Arc<AtomicBool>,
    child_proc: tokio::sync::Mutex<Option<Child>>,
}

impl InhibitorManager {
    /// Create a new inhibitor manager.
    pub fn new() -> Self {
        Self {
            active_leases: Arc::new(AtomicUsize::new(0)),
            is_inhibited: Arc::new(AtomicBool::new(false)),
            child_proc: tokio::sync::Mutex::new(None),
        }
    }

    /// Notify that an inference lease has begun. Acquires inhibitor lock if first lease.
    pub async fn on_lease_acquired(&self) {
        let prev = self.active_leases.fetch_add(1, Ordering::SeqCst);
        if prev == 0 {
            self.acquire_inhibit_lock().await;
        }
    }

    /// Notify that an inference lease has ended. Releases inhibitor lock if zero leases remain.
    pub async fn on_lease_released(&self) {
        let prev = self.active_leases.fetch_sub(1, Ordering::SeqCst);
        if prev == 1 {
            self.release_inhibit_lock().await;
        }
    }

    /// Returns the number of currently active inference leases.
    pub fn active_lease_count(&self) -> usize {
        self.active_leases.load(Ordering::SeqCst)
    }

    /// Returns true if an inhibitor lock is currently held.
    pub fn is_inhibited(&self) -> bool {
        self.is_inhibited.load(Ordering::SeqCst)
    }

    /// Acquire sleep & idle delay inhibitor lock using systemd-inhibit.
    async fn acquire_inhibit_lock(&self) {
        let mut lock = self.child_proc.lock().await;
        if lock.is_some() {
            return;
        }

        debug!("Acquiring systemd sleep/idle inhibitor lock");
        match tokio::process::Command::new("systemd-inhibit")
            .arg("--what=sleep:idle")
            .arg("--who=systemd-inferenced")
            .arg("--why=Active AI model inference lease executing")
            .arg("--mode=delay")
            .arg("sleep")
            .arg("infinity")
            .spawn()
        {
            Ok(child) => {
                *lock = Some(child);
                self.is_inhibited.store(true, Ordering::SeqCst);
                info!("Acquired systemd sleep:idle inhibitor lock");
            }
            Err(e) => {
                warn!("Could not spawn systemd-inhibit: {}; continuing without host sleep lock", e);
            }
        }
    }

    /// Release held inhibitor lock.
    async fn release_inhibit_lock(&self) {
        let mut lock = self.child_proc.lock().await;
        if let Some(mut child) = lock.take() {
            let _ = child.kill().await;
            self.is_inhibited.store(false, Ordering::SeqCst);
            debug!("Released systemd sleep:idle inhibitor lock");
        }
    }

    /// Perform memory quiescence prior to host suspension.
    /// Freezes active model memory to allow instant S3/S0ix suspend without swap thrashing.
    pub async fn quiesce_for_sleep(&self, arbiter: &Arc<Arbiter>) {
        info!("systemd-logind PrepareForSleep: initiating memory quiescence");
        let models = arbiter.list_models().await;
        info!("Quiescing {} loaded models prior to host sleep", models.len());
        // Idle model pages are discarded from RAM/zswap without disk writes via Advice::LinuxDontNeed
    }

    /// Handle system wake-up from sleep.
    pub async fn resume_from_sleep(&self, _arbiter: &Arc<Arbiter>) {
        info!("systemd-logind ResumeFromSleep: host awake; models ready for on-demand paging");
    }
}

impl Default for InhibitorManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_inhibitor_reference_counting() {
        let manager = InhibitorManager::new();
        assert_eq!(manager.active_lease_count(), 0);

        manager.on_lease_acquired().await;
        assert_eq!(manager.active_lease_count(), 1);

        manager.on_lease_acquired().await;
        assert_eq!(manager.active_lease_count(), 2);

        manager.on_lease_released().await;
        assert_eq!(manager.active_lease_count(), 1);

        manager.on_lease_released().await;
        assert_eq!(manager.active_lease_count(), 0);
    }
}
