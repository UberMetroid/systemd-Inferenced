use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComputePlaneKind {
    DiscreteGpu,
    IntegratedUma,
    NpuAccelerator,
    CpuMatrixExtension,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputePlane {
    pub id: String,
    pub name: String,
    pub kind: ComputePlaneKind,
    pub device_path: Option<PathBuf>,
    pub total_memory_bytes: u64,
    pub available_memory_bytes: u64,
    pub numa_node: Option<u32>,
    pub supported_formats: Vec<String>,
    pub is_triage_reserved: bool,
    pub hardware_features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HardwareTopology {
    pub planes: Vec<ComputePlane>,
    pub total_system_ram_bytes: u64,
    pub available_system_ram_bytes: u64,
    pub cpu_cores_total: usize,
    pub numa_nodes: usize,
}
