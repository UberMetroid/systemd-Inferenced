use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PressureLevel {
    Normal,
    Elevated,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressureMetrics {
    pub memory_some_avg10: f32,
    pub memory_full_avg10: f32,
    pub cpu_some_avg10: f32,
    pub io_some_avg10: f32,
    pub level: PressureLevel,
}

impl Default for PressureMetrics {
    fn default() -> Self {
        Self {
            memory_some_avg10: 0.0,
            memory_full_avg10: 0.0,
            cpu_some_avg10: 0.0,
            io_some_avg10: 0.0,
            level: PressureLevel::Normal,
        }
    }
}

impl PressureMetrics {
    /// Read real-time Linux kernel Pressure Stall Information (PSI).
    pub fn read_current() -> Self {
        let mem_some = Self::parse_psi_avg10("/proc/pressure/memory", "some").unwrap_or(0.0);
        let mem_full = Self::parse_psi_avg10("/proc/pressure/memory", "full").unwrap_or(0.0);
        let cpu_some = Self::parse_psi_avg10("/proc/pressure/cpu", "some").unwrap_or(0.0);
        let io_some = Self::parse_psi_avg10("/proc/pressure/io", "some").unwrap_or(0.0);

        let level = if mem_full > 10.0 || mem_some > 40.0 || io_some > 50.0 {
            PressureLevel::Critical
        } else if mem_some > 15.0 || io_some > 20.0 || cpu_some > 60.0 {
            PressureLevel::Elevated
        } else {
            PressureLevel::Normal
        };

        Self {
            memory_some_avg10: mem_some,
            memory_full_avg10: mem_full,
            cpu_some_avg10: cpu_some,
            io_some_avg10: io_some,
            level,
        }
    }

    fn parse_psi_avg10(path: &str, line_prefix: &str) -> Option<f32> {
        let p = Path::new(path);
        if !p.exists() {
            return None;
        }
        let content = fs::read_to_string(p).ok()?;
        for line in content.lines() {
            if line.starts_with(line_prefix) {
                // Format: some avg10=0.00 avg60=0.00 avg300=0.00 total=29103914
                for part in line.split_whitespace() {
                    if let Some(val_str) = part.strip_prefix("avg10=") {
                        return val_str.parse::<f32>().ok();
                    }
                }
            }
        }
        None
    }
}
