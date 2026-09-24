use super::types::{ComputePlane, ComputePlaneKind};

pub const CPU_TRIAGE_RESERVED_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2GB dedicated quota

/// Assign the out-of-band emergency triage enclave for systemd-sentry.
/// Prefers NPU first to survive complete GPU driver panics, falling back to CPU.
/// On CPU fallback, reserves 2GB memory quota without locking out non-emergency workloads.
pub fn assign_triage_enclave(planes: &mut [ComputePlane]) -> Option<String> {
    // 1. Prefer NPU first (fully reserved)
    if let Some(npu) = planes.iter_mut().find(|p| p.kind == ComputePlaneKind::NpuAccelerator) {
        npu.is_triage_reserved = true;
        return Some(npu.id.clone());
    }

    // 2. Fall back to dedicated CPU-AMX / Host CPU slice
    // Reserves up to 2GB memory quota for emergency triage (or at most 25% of available RAM on constrained hosts),
    // leaving remaining RAM accessible for general workloads.
    if let Some(cpu) = planes.iter_mut().find(|p| p.kind == ComputePlaneKind::CpuMatrixExtension) {
        cpu.is_triage_reserved = false;
        let triage_quota = CPU_TRIAGE_RESERVED_BYTES.min(cpu.available_memory_bytes / 4);
        cpu.available_memory_bytes = cpu
            .available_memory_bytes
            .saturating_sub(triage_quota);
        cpu.total_memory_bytes = cpu
            .total_memory_bytes
            .saturating_sub(triage_quota);
        return Some(cpu.id.clone());
    }

    None
}

