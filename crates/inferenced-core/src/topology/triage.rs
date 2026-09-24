use super::types::{ComputePlane, ComputePlaneKind};

/// Assign the out-of-band emergency triage enclave for systemd-sentry.
/// Prefers NPU first to survive complete GPU driver panics, falling back to CPU.
pub fn assign_triage_enclave(planes: &mut [ComputePlane]) -> Option<String> {
    // 1. Prefer NPU first
    if let Some(npu) = planes.iter_mut().find(|p| p.kind == ComputePlaneKind::NpuAccelerator) {
        npu.is_triage_reserved = true;
        return Some(npu.id.clone());
    }

    // 2. Fall back to dedicated CPU-AMX / Host CPU slice
    if let Some(cpu) = planes.iter_mut().find(|p| p.kind == ComputePlaneKind::CpuMatrixExtension) {
        cpu.is_triage_reserved = true;
        return Some(cpu.id.clone());
    }

    None
}
