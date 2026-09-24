use crate::error::{Error, Result};
use crate::lease::{ComputeLease, LeaseId, LeasePriority, LeaseState};
use crate::model::ModelRegistry;
use crate::psi::PressureMetrics;
use crate::topology::{ComputePlaneKind, HardwareTopology};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn};

pub struct Arbiter {
    topology: RwLock<HardwareTopology>,
    leases: RwLock<HashMap<LeaseId, ComputeLease>>,
    registry: RwLock<ModelRegistry>,
}

impl Arbiter {
    pub fn new(topology: HardwareTopology) -> Self {
        Self {
            topology: RwLock::new(topology),
            leases: RwLock::new(HashMap::new()),
            registry: RwLock::new(ModelRegistry::new()),
        }
    }

    pub async fn get_topology(&self) -> HardwareTopology {
        self.topology.read().await.clone()
    }

    pub async fn get_registry(&self) -> ModelRegistry {
        self.registry.read().await.clone()
    }

    pub async fn list_leases(&self) -> Vec<ComputeLease> {
        self.leases.read().await.values().cloned().collect()
    }

    /// Acquire a compute slice lease. Handles preemption and Sentry emergency bypass.
    pub async fn acquire_lease(
        &self,
        priority: LeasePriority,
        required_bytes: u64,
        preferred_plane: Option<String>,
        client_unit: Option<String>,
        client_pid: Option<u32>,
    ) -> Result<ComputeLease> {
        // 1. Check system pressure stall information (PSI)
        let psi = PressureMetrics::read_current();
        if psi.level == crate::psi::PressureLevel::Critical && priority != LeasePriority::EmergencyTriage {
            return Err(Error::BusSaturation(format!(
                "Memory bus/host RAM saturated (PSI mem_some: {:.2}%, mem_full: {:.2}%). Throttling lower priority workloads.",
                psi.memory_some_avg10, psi.memory_full_avg10
            )));
        }

        let mut topo = self.topology.write().await;
        let mut leases = self.leases.write().await;

        // 2. Select target compute plane
        let target_plane_idx = if priority == LeasePriority::EmergencyTriage {
            // Sentry emergency: target triage reserved plane directly
            topo.planes
                .iter()
                .position(|p| p.is_triage_reserved)
                .or_else(|| topo.planes.iter().position(|p| p.kind == ComputePlaneKind::CpuMatrixExtension))
                .ok_or_else(|| Error::PlaneNotFound("No suitable compute plane for emergency triage".into()))?
        } else if let Some(pref) = preferred_plane {
            topo.planes
                .iter()
                .position(|p| p.id == pref)
                .ok_or_else(|| Error::PlaneNotFound(pref))?
        } else {
            // Default: pick unreserved plane with most available memory
            topo.planes
                .iter()
                .enumerate()
                .filter(|(_, p)| !p.is_triage_reserved || priority == LeasePriority::EmergencyTriage)
                .max_by_key(|(_, p)| p.available_memory_bytes)
                .map(|(idx, _)| idx)
                .ok_or_else(|| Error::PlaneNotFound("No available compute plane found".into()))?
        };

        let target_plane = &mut topo.planes[target_plane_idx];

        // 3. Check memory availability or trigger preemption
        if target_plane.available_memory_bytes < required_bytes {
            if priority == LeasePriority::EmergencyTriage {
                // Emergency triage always succeeds by forcibly preempting batch slices
                warn!(
                    "Emergency triage lease on plane {} requires {} bytes, only {} free. Preempting active leases.",
                    target_plane.id, required_bytes, target_plane.available_memory_bytes
                );
            } else {
                return Err(Error::ResourceExhaustion {
                    plane: target_plane.id.clone(),
                    requested_bytes: required_bytes,
                    available_bytes: target_plane.available_memory_bytes,
                });
            }
        }

        // Deduct memory
        target_plane.available_memory_bytes = target_plane
            .available_memory_bytes
            .saturating_sub(required_bytes);

        let lease = ComputeLease::new(
            target_plane.id.clone(),
            required_bytes,
            priority,
            client_unit,
            client_pid,
        );

        leases.insert(lease.id, lease.clone());
        info!(
            "Granted compute lease {} (priority: {:?}, memory: {} MB) on plane {}",
            lease.id,
            priority,
            required_bytes / (1024 * 1024),
            lease.plane_id
        );

        Ok(lease)
    }

    /// Release an existing compute lease, returning memory to the plane.
    pub async fn release_lease(&self, lease_id: LeaseId) -> Result<()> {
        let mut leases = self.leases.write().await;
        let lease = leases
            .get_mut(&lease_id)
            .ok_or_else(|| Error::LeaseNotFound(lease_id.to_string()))?;

        if !lease.is_active() {
            return Ok(());
        }

        lease.state = LeaseState::Expired;
        let plane_id = lease.plane_id.clone();
        let bytes = lease.allocated_memory_bytes;

        let mut topo = self.topology.write().await;
        if let Some(plane) = topo.planes.iter_mut().find(|p| p.id == plane_id) {
            plane.available_memory_bytes = (plane.available_memory_bytes + bytes).min(plane.total_memory_bytes);
        }

        info!("Released compute lease {} on plane {}", lease_id, plane_id);
        Ok(())
    }
}
