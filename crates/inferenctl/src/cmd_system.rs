use crate::client::VarlinkClient;
use anyhow::Result;
use colored::*;
use inferenced_core::psi::PressureMetrics;
use inferenced_core::topology::HardwareTopology;
use std::path::Path;
use tabled::{settings::Style, Table, Tabled};

#[derive(Tabled)]
struct PlaneRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Kind")]
    kind: String,
    #[tabled(rename = "Total")]
    total: String,
    #[tabled(rename = "Available")]
    available: String,
    #[tabled(rename = "Triage Enclave")]
    triage: String,
}

#[derive(Tabled)]
struct LeaseRow {
    #[tabled(rename = "Lease ID")]
    id: String,
    #[tabled(rename = "Plane")]
    plane: String,
    #[tabled(rename = "Memory")]
    memory: String,
    #[tabled(rename = "Priority")]
    priority: String,
    #[tabled(rename = "State")]
    state: String,
    #[tabled(rename = "Unit")]
    unit: String,
}

fn format_bytes(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else {
        format!("{:.0} MB", mb)
    }
}

pub fn run_status(socket_path: impl AsRef<Path>, json_out: bool) -> Result<()> {
    if let Ok(mut client) = VarlinkClient::connect(socket_path.as_ref()) {
        let topo = client.get_topology()?;
        let pressure = client.get_pressure()?;
        let leases = client.list_leases()?;

        if json_out {
            let combined = serde_json::json!({
                "daemon": "active (running)",
                "topology": topo,
                "pressure": pressure,
                "leases": leases,
            });
            println!("{}", serde_json::to_string_pretty(&combined)?);
            return Ok(());
        }

        println!("{}", "● systemd-inferenced.service - Heterogeneous AI Compute Fabric".bold());
        println!("     Status: {}", "active (running via Varlink IPC)".green().bold());

        if let Some(planes) = topo.get("planes").and_then(|v| v.as_array()) {
            let rows: Vec<PlaneRow> = planes.iter().map(|p| PlaneRow {
                id: p.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                kind: p.get("kind").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                total: format_bytes(p.get("total_memory").and_then(|v| v.as_u64()).unwrap_or(0)),
                available: format_bytes(p.get("available_memory").and_then(|v| v.as_u64()).unwrap_or(0)),
                triage: if p.get("is_triage_reserved").and_then(|v| v.as_bool()).unwrap_or(false) {
                    "● RESERVED (sentry)".red().bold().to_string()
                } else {
                    "shared".dimmed().to_string()
                },
            }).collect();
            let mut table = Table::new(rows);
            table.with(Style::rounded());
            println!("\n{}", table);
        }

        println!("\n{}", "Kernel Pressure Stall Information (PSI):".bold().underline());
        println!(
            "  Memory Stall: {:.2}% | CPU Stall: {:.2}% | IO Stall: {:.2}%",
            pressure.get("memory_some").and_then(|v| v.as_f64()).unwrap_or(0.0),
            pressure.get("cpu_some").and_then(|v| v.as_f64()).unwrap_or(0.0),
            pressure.get("io_some").and_then(|v| v.as_f64()).unwrap_or(0.0),
        );
        return Ok(());
    }

    // Fallback: daemon not running
    if json_out {
        println!("{}", serde_json::json!({ "daemon": "inactive" }));
        return Ok(());
    }
    println!("{}", "● systemd-inferenced.service - Heterogeneous AI Compute Fabric".bold());
    println!("     Status: {}", "inactive (daemon offline - showing local hardware probe)".yellow().bold());
    let topo = HardwareTopology::discover()?;
    println!("  Discovered {} compute planes (local probe)", topo.planes.len());
    Ok(())
}

pub fn run_planes(socket_path: impl AsRef<Path>, json_out: bool) -> Result<()> {
    if let Ok(mut client) = VarlinkClient::connect(socket_path.as_ref()) {
        let topo = client.get_topology()?;
        if json_out {
            println!("{}", serde_json::to_string_pretty(&topo)?);
            return Ok(());
        }
        if let Some(planes) = topo.get("planes").and_then(|v| v.as_array()) {
            let rows: Vec<PlaneRow> = planes.iter().map(|p| PlaneRow {
                id: p.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                kind: p.get("kind").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                total: format_bytes(p.get("total_memory").and_then(|v| v.as_u64()).unwrap_or(0)),
                available: format_bytes(p.get("available_memory").and_then(|v| v.as_u64()).unwrap_or(0)),
                triage: if p.get("is_triage_reserved").and_then(|v| v.as_bool()).unwrap_or(false) {
                    "YES".into()
                } else {
                    "NO".into()
                },
            }).collect();
            let mut table = Table::new(rows);
            table.with(Style::rounded());
            println!("{}", table);
        }
        return Ok(());
    }
    let topo = HardwareTopology::discover()?;
    let rows: Vec<PlaneRow> = topo.planes.iter().map(|p| PlaneRow {
        id: p.id.clone(),
        kind: format!("{:?}", p.kind),
        total: format_bytes(p.total_memory_bytes),
        available: format_bytes(p.available_memory_bytes),
        triage: if p.is_triage_reserved { "YES".into() } else { "NO".into() },
    }).collect();
    let mut table = Table::new(rows);
    table.with(Style::rounded());
    println!("{}", table);
    Ok(())
}

pub fn run_leases(socket_path: impl AsRef<Path>, json_out: bool) -> Result<()> {
    let mut client = VarlinkClient::connect(socket_path)?;
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
            table.with(Style::rounded());
            println!("{}", table);
        }
        _ => {
            println!("{}", "No active compute leases. All compute planes idle.".dimmed());
        }
    }
    Ok(())
}

pub fn run_freeze(socket_path: impl AsRef<Path>, lease_id: &str) -> Result<()> {
    let mut client = VarlinkClient::connect(socket_path)?;
    client.freeze(lease_id)?;
    println!("Froze compute lease {}", lease_id.green());
    Ok(())
}

pub fn run_thaw(socket_path: impl AsRef<Path>, lease_id: &str) -> Result<()> {
    let mut client = VarlinkClient::connect(socket_path)?;
    client.thaw(lease_id)?;
    println!("Thawed compute lease {}", lease_id.green());
    Ok(())
}

pub fn run_monitor(socket_path: impl AsRef<Path>, count: usize) -> Result<()> {
    println!("{}", "Streaming real-time hardware telemetry (Ctrl+C to quit)...".bold());
    for _ in 0..count {
        if let Ok(mut client) = VarlinkClient::connect(socket_path.as_ref()) {
            if let Ok(p) = client.get_pressure() {
                println!(
                    "[{}] mem_some: {:.2}% | cpu: {:.2}% | io: {:.2}%",
                    p.get("level").and_then(|v| v.as_str()).unwrap_or("Normal"),
                    p.get("memory_some").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    p.get("cpu_some").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    p.get("io_some").and_then(|v| v.as_f64()).unwrap_or(0.0),
                );
                std::thread::sleep(std::time::Duration::from_millis(1000));
                continue;
            }
        }
        let psi = PressureMetrics::read_current();
        println!(
            "[{:?}] mem_some: {:.2}% | mem_full: {:.2}% | cpu: {:.2}% | io: {:.2}%",
            psi.level, psi.memory_some_avg10, psi.memory_full_avg10, psi.cpu_some_avg10, psi.io_some_avg10
        );
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
    Ok(())
}
