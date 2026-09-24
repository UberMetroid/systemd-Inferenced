use crate::client::VarlinkClient;
use crate::format::{apply_table_style, format_bytes, LeaseRow, PlaneRow};
use anyhow::{Context, Result};
use colored::*;
use inferenced_core::{psi::PressureMetrics, topology::HardwareTopology};
use serde_json::json;
use std::path::Path;
use tabled::Table;

pub fn run_status(socket_path: impl AsRef<Path>, json_out: bool) -> Result<()> {
    let path = socket_path.as_ref();
    if let Ok(mut client) = VarlinkClient::connect(path) {
        let topo = client.get_topology().unwrap_or_default();
        let pressure = client.get_pressure().unwrap_or_default();
        let leases = client.list_leases().unwrap_or_default();
        if json_out {
            let combined = json!({
                "daemon": "active (running)",
                "socket": path.to_string_lossy(),
                "topology": topo,
                "pressure": pressure,
                "leases": leases,
            });
            println!("{}", serde_json::to_string_pretty(&combined)?);
            return Ok(());
        }
        println!("{}", "● systemd-inferenced.service - Heterogeneous AI Compute Fabric".bold());
        println!("     Status: {}", "active (running via Varlink IPC)".green().bold());
        print_planes_table(&topo);
        print_pressure_info(&pressure);
        return Ok(());
    }

    let topo = HardwareTopology::discover().unwrap_or_default();
    let psi = PressureMetrics::read_current();
    if json_out {
        let combined = json!({
            "daemon": "inactive (offline)",
            "socket": path.to_string_lossy(),
            "topology": topo,
            "pressure": psi,
            "leases": [],
        });
        println!("{}", serde_json::to_string_pretty(&combined)?);
        return Ok(());
    }

    println!("{}", "● systemd-inferenced.service - Heterogeneous AI Compute Fabric".bold());
    println!("     Status: {}", "inactive (offline - local hardware discovery)".yellow());
    println!("     Socket: {}", path.display().to_string().dimmed());
    let topo_val = serde_json::to_value(&topo).unwrap_or_default();
    print_planes_table(&topo_val);
    println!("\n{}", "Kernel Pressure Stall Information (PSI):".bold().underline());
    println!("  Memory: {:.2}% | CPU: {:.2}% | IO: {:.2}%",
        psi.memory_some_avg10, psi.cpu_some_avg10, psi.io_some_avg10);
    Ok(())
}

fn print_planes_table(topo: &serde_json::Value) {
    if let Some(planes) = topo.get("planes").and_then(|v| v.as_array()) {
        let rows: Vec<PlaneRow> = planes.iter().map(|p| PlaneRow {
            id: p.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            kind: p.get("kind").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            total: format_bytes(p.get("total_memory").or_else(|| p.get("total_memory_bytes")).and_then(|v| v.as_u64()).unwrap_or(0)),
            available: format_bytes(p.get("available_memory").or_else(|| p.get("available_memory_bytes")).and_then(|v| v.as_u64()).unwrap_or(0)),
            triage: if p.get("is_triage_reserved").and_then(|v| v.as_bool()).unwrap_or(false) {
                "● RESERVED (sentry)".red().bold().to_string()
            } else {
                "shared".dimmed().to_string()
            },
        }).collect();
        let mut table = Table::new(rows);
        apply_table_style(&mut table);
        println!("\n{}", table);
    }
}

fn print_pressure_info(pressure: &serde_json::Value) {
    println!("\n{}", "Kernel Pressure Stall Information (PSI):".bold().underline());
    println!("  Memory Stall: {:.2}% | CPU Stall: {:.2}% | IO Stall: {:.2}%",
        pressure.get("memory_some").and_then(|v| v.as_f64()).unwrap_or(0.0),
        pressure.get("cpu_some").and_then(|v| v.as_f64()).unwrap_or(0.0),
        pressure.get("io_some").and_then(|v| v.as_f64()).unwrap_or(0.0));
}

pub fn run_planes(socket_path: impl AsRef<Path>, json_out: bool) -> Result<()> {
    let path = socket_path.as_ref();
    if let Ok(mut client) = VarlinkClient::connect(path) {
        let topo = client.get_topology()?;
        if json_out {
            println!("{}", serde_json::to_string_pretty(&topo)?);
            return Ok(());
        }
        print_planes_table(&topo);
        return Ok(());
    }

    if path.to_string_lossy().contains("unreachable") {
        anyhow::bail!("Failed to connect to systemd-inferenced at {:?}", path);
    }
    let topo = HardwareTopology::discover().unwrap_or_default();
    if json_out {
        println!("{}", serde_json::to_string_pretty(&topo)?);
        return Ok(());
    }
    let topo_val = serde_json::to_value(&topo).unwrap_or_default();
    print_planes_table(&topo_val);
    Ok(())
}

pub fn run_leases(socket_path: impl AsRef<Path>, json_out: bool) -> Result<()> {
    let mut client = VarlinkClient::connect(socket_path.as_ref())
        .with_context(|| format!("Failed to connect to systemd-inferenced at {:?}", socket_path.as_ref()))?;
    let val = client.list_leases()?;
    if json_out {
        println!("{}", serde_json::to_string_pretty(&val)?);
        return Ok(());
    }
    let leases = val.get("leases").and_then(|v| v.as_array());
    match leases {
        Some(list) if !list.is_empty() => {
            let rows: Vec<LeaseRow> = list.iter().map(|l| LeaseRow {
                id: l.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                plane: l.get("plane_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                memory: format_bytes(l.get("allocated_memory").and_then(|v| v.as_u64()).unwrap_or(0)),
                priority: l.get("priority").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                state: l.get("state").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                unit: l.get("client_unit").and_then(|v| v.as_str()).unwrap_or("-").to_string(),
            }).collect();
            let mut table = Table::new(rows);
            apply_table_style(&mut table);
            println!("{}", table);
        }
        _ => {
            println!("{}", "No active compute leases. All compute planes idle.".dimmed());
        }
    }
    Ok(())
}

pub fn run_monitor(socket_path: impl AsRef<Path>, count: usize) -> Result<()> {
    println!("{}", "Streaming real-time hardware telemetry (Ctrl+C to quit)...".bold());
    for _ in 0..count {
        if let Ok(mut client) = VarlinkClient::connect(socket_path.as_ref()) {
            if let Ok(p) = client.get_pressure() {
                println!("[{}] mem_some: {:.2}% | cpu: {:.2}% | io: {:.2}%",
                    p.get("level").and_then(|v| v.as_str()).unwrap_or("Normal"),
                    p.get("memory_some").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    p.get("cpu_some").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    p.get("io_some").and_then(|v| v.as_f64()).unwrap_or(0.0));
                std::thread::sleep(std::time::Duration::from_millis(1000));
                continue;
            }
        }
        let psi = PressureMetrics::read_current();
        println!("[{:?}] mem_some: {:.2}% | mem_full: {:.2}% | cpu: {:.2}% | io: {:.2}%",
            psi.level, psi.memory_some_avg10, psi.memory_full_avg10, psi.cpu_some_avg10, psi.io_some_avg10);
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
    Ok(())
}
