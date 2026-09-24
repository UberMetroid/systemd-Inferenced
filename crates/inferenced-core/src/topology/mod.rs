pub mod cpu;
pub mod drm;
pub mod npu;
pub mod triage;
pub mod types;

pub use types::{ComputePlane, ComputePlaneKind, HardwareTopology};

use crate::error::Result;

impl HardwareTopology {
    /// Discover heterogeneous hardware topology across the host Linux system.
    pub fn discover() -> Result<Self> {
        let (total_ram, avail_ram) = cpu::read_meminfo_from("/proc/meminfo");
        let cpu_cores = cpu::count_cpu_cores("/proc/cpuinfo");
        let numa_nodes = cpu::count_numa_nodes("/sys/devices/system/node");

        let mut planes = Vec::new();

        // 1. Discover NPUs (/dev/accel/*, Hailo)
        let mut npus = npu::discover_npu_planes("/dev/accel", "/dev/hailo0");
        planes.append(&mut npus);

        // 2. Discover DRM Graphics & Compute Devices (/dev/dri/renderD*)
        let mut drms = drm::discover_drm_planes("/dev/dri", "/sys/class/drm", total_ram, avail_ram);
        planes.append(&mut drms);

        // 3. Discover Host CPU Matrix Plane
        let cpu_plane = cpu::build_cpu_plane("/proc/cpuinfo", avail_ram);
        planes.push(cpu_plane);

        // 4. Assign Sentry Emergency Triage Enclave
        triage::assign_triage_enclave(&mut planes);

        Ok(Self {
            planes,
            total_system_ram_bytes: total_ram,
            available_system_ram_bytes: avail_ram,
            cpu_cores_total: cpu_cores,
            numa_nodes,
        })
    }
}
