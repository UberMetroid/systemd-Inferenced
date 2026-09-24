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
        if !self.models.contains_key(&descriptor.id) && self.models.len() >= 256 {
            if let Some(evict_id) = self
                .models
                .iter()
                .filter(|(_, m)| m.placement != ModelPlacementState::PinnedTriage)
                .min_by_key(|(_, m)| (m.access_count, m.last_accessed))
                .map(|(k, _)| k.clone())
            {
                self.models.remove(&evict_id);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_model(id: &str, accesses: u64) -> ModelDescriptor {
        ModelDescriptor {
            id: id.into(),
            format: "GGUF".into(),
            path: PathBuf::from(format!("/models/{}.gguf", id)),
            estimated_memory_bytes: 1024 * 1024,
            placement: ModelPlacementState::Dormant,
            resident_plane_id: None,
            last_accessed: Utc::now(),
            access_count: accesses,
            preferred_plane: None,
        }
    }

    #[test]
    fn test_model_registry_eviction_and_update() {
        let mut reg = ModelRegistry::new();
        for i in 0..256 {
            reg.register(dummy_model(&format!("model_{}", i), i as u64 + 1));
        }
        assert_eq!(reg.list().len(), 256);

        // Updating an existing model must NOT evict any other model
        reg.register(dummy_model("model_10", 999));
        assert_eq!(reg.list().len(), 256);
        assert!(reg.get("model_0").is_some());

        // Registering a 257th model should evict the LRU (model_0 with access_count 1)
        reg.register(dummy_model("model_256", 500));
        assert_eq!(reg.list().len(), 256);
        assert!(reg.get("model_0").is_none());
        assert!(reg.get("model_256").is_some());
    }

    #[test]
    fn test_model_registry_pinned_immunity() {
        let mut reg = ModelRegistry::new();
        let mut pinned = dummy_model("pinned_triage", 0);
        pinned.placement = ModelPlacementState::PinnedTriage;
        reg.register(pinned);

        for i in 1..256 {
            reg.register(dummy_model(&format!("m_{}", i), i as u64 + 10));
        }
        assert_eq!(reg.list().len(), 256);

        // Register new model: pinned model must NOT be evicted despite having lowest access count
        reg.register(dummy_model("m_256", 50));
        assert_eq!(reg.list().len(), 256);
        assert!(reg.get("pinned_triage").is_some());
    }
}
