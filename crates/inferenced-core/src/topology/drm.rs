use super::types::{ComputePlane, ComputePlaneKind};
use std::fs;
use std::path::Path;

pub fn discover_drm_planes(dri_dir: &str, sysfs_drm_dir: &str, total_ram: u64, avail_ram: u64) -> Vec<ComputePlane> {
    let mut planes = Vec::new();
    let dri_path = Path::new(dri_dir);
    if !dri_path.exists() {
        return planes;
    }

    let Ok(entries) = fs::read_dir(dri_path) else {
        return planes;
    };

    for entry in entries.flatten() {
        let fname = entry.file_name().to_string_lossy().to_string();
        if fname.starts_with("renderD") {
            let dev_path = entry.path();
            let sysfs_card = Path::new(sysfs_drm_dir).join(&fname);
            let (vendor_name, is_integrated, vram_total) = inspect_drm_sysfs(&sysfs_card, total_ram);

            let kind = if is_integrated {
                ComputePlaneKind::IntegratedUma
            } else {
                ComputePlaneKind::DiscreteGpu
            };

            let avail_vram = if is_integrated {
                avail_ram.min(vram_total)
            } else {
                vram_total
            };

            planes.push(ComputePlane {
                id: format!("drm-{}", fname),
                name: format!("{} ({})", vendor_name, fname),
                kind,
                device_path: Some(dev_path),
                total_memory_bytes: vram_total,
                available_memory_bytes: avail_vram,
                numa_node: None,
                supported_formats: vec!["FP16".into(), "BF16".into(), "FP8".into(), "INT4".into(), "GGUF".into()],
                is_triage_reserved: false,
                is_quarantined: false,
                hardware_features: vec!["drm-gem".into(), "vram-managed".into()],
            });
        }
    }
    planes
}

pub fn inspect_drm_sysfs(sysfs_card: &Path, total_system_ram: u64) -> (String, bool, u64) {
    let device_dir = sysfs_card.join("device");
    let vendor = fs::read_to_string(device_dir.join("vendor"))
        .map(|v| v.trim().to_lowercase())
        .unwrap_or_default();

    let vendor_name = match vendor.as_str() {
        "0x10de" => "NVIDIA Corporation",
        "0x1002" => "Advanced Micro Devices [AMD/ATI]",
        "0x8086" => "Intel Corporation",
        "0x17cb" => "Qualcomm Technologies",
        _ => "Direct Rendering Accelerator",
    };

    let vram_path = device_dir.join("mem_info_vram_total");
    if let Ok(vram_str) = fs::read_to_string(vram_path) {
        if let Ok(vram_bytes) = vram_str.trim().parse::<u64>() {
            if vram_bytes > 0 {
                return (vendor_name.into(), false, vram_bytes);
            }
        }
    }

    let is_integrated = vendor == "0x8086" || vendor.is_empty();
    let uma_slice = (total_system_ram / 2).max(1024 * 1024 * 1024);
    (vendor_name.into(), is_integrated, uma_slice)
}
