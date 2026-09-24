use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelPlacementState {
    /// Cold on NVMe/SSD storage
    Dormant,
    /// Warm in host RAM / zero-copy mmap staging
    Staged,
    /// Loaded in active compute plane (GPU VRAM or NPU memory)
    Resident,
    /// Dedicated, unevictable emergency slice for systemd-sentry
    PinnedTriage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDescriptor {
    pub id: String,
    pub format: String,
    pub path: PathBuf,
    pub estimated_memory_bytes: u64,
    pub placement: ModelPlacementState,
    pub resident_plane_id: Option<String>,
    pub last_accessed: DateTime<Utc>,
    pub access_count: u64,
    pub preferred_plane: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelRegistry {
    models: HashMap<String, ModelDescriptor>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
        }
    }

    pub fn register(&mut self, descriptor: ModelDescriptor) {
        self.models.insert(descriptor.id.clone(), descriptor);
    }

    pub fn get(&self, id: &str) -> Option<&ModelDescriptor> {
        self.models.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut ModelDescriptor> {
        self.models.get_mut(id)
    }

    pub fn list(&self) -> Vec<ModelDescriptor> {
        self.models.values().cloned().collect()
    }

    pub fn remove(&mut self, id: &str) -> Option<ModelDescriptor> {
        self.models.remove(id)
    }

    pub fn pin_for_triage(&mut self, id: &str, plane_id: String) -> bool {
        if let Some(m) = self.models.get_mut(id) {
            m.placement = ModelPlacementState::PinnedTriage;
            m.resident_plane_id = Some(plane_id);
            true
        } else {
            false
        }
    }
}
