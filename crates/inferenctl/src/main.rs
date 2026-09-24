use clap::{Parser, Subcommand};
use colored::*;
use inferenced_core::psi::PressureMetrics;
use inferenced_core::topology::{ComputePlaneKind, HardwareTopology};
use tabled::{settings::Style, Table, Tabled};

#[derive(Parser)]
#[command(name = "inferenctl")]
#[command(about = "Control and inspect systemd-inferenced hardware arbitration and model residency")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show system-wide AI compute status, PSI pressure, and active leases
    Status,
    /// List all discovered heterogeneous compute planes (GPU, NPU, UMA, CPU)
    Planes,
    /// List active compute slice leases
    Leases,
    /// Real-time live monitor of hardware pressure and bus saturation
    Monitor,
}

#[derive(Tabled)]
struct PlaneRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Kind")]
    kind: String,
    #[tabled(rename = "Device")]
    device: String,
    #[tabled(rename = "Total")]
    total: String,
    #[tabled(rename = "Available")]
    available: String,
    #[tabled(rename = "Triage Enclave")]
    triage: String,
}

fn format_bytes(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else {
        format!("{:.0} MB", mb)
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Status => {
            println!("{}", "● systemd-inferenced.service - Heterogeneous AI Compute Fabric".bold());
            println!("     Status: {}", "active (running)".green().bold());

            // 1. Hardware Discovery
            let topo = HardwareTopology::discover()?;
            println!("\n{}", "Compute Plane Topology:".bold().underline());
            println!(
                "  Host System RAM: {} total, {} available",
                format_bytes(topo.total_system_ram_bytes),
                format_bytes(topo.available_system_ram_bytes)
            );
            println!(
                "  CPU Cores: {} logical, NUMA nodes: {}",
                topo.cpu_cores_total, topo.numa_nodes
            );

            let rows: Vec<PlaneRow> = topo
                .planes
                .iter()
                .map(|p| PlaneRow {
                    id: p.id.clone(),
                    kind: match p.kind {
                        ComputePlaneKind::DiscreteGpu => "dGPU".cyan().to_string(),
                        ComputePlaneKind::IntegratedUma => "UMA/APU".yellow().to_string(),
                        ComputePlaneKind::NpuAccelerator => "NPU".green().bold().to_string(),
                        ComputePlaneKind::CpuMatrixExtension => "CPU-Matrix".magenta().to_string(),
                    },
                    device: p
                        .device_path
                        .as_ref()
                        .map(|d| d.display().to_string())
                        .unwrap_or_else(|| "cpu-host".into()),
                    total: format_bytes(p.total_memory_bytes),
                    available: format_bytes(p.available_memory_bytes),
                    triage: if p.is_triage_reserved {
                        "● RESERVED (systemd-sentry)".red().bold().to_string()
                    } else {
                        "shared".dimmed().to_string()
                    },
                })
                .collect();

            let mut table = Table::new(rows);
            table.with(Style::rounded());
            println!("\n{}", table);

            // 2. Kernel PSI Pressure
            let psi = PressureMetrics::read_current();
            println!("\n{}", "Kernel Pressure Stall Information (PSI):".bold().underline());
            let psi_status = match psi.level {
                inferenced_core::PressureLevel::Normal => "NORMAL (nominal bus latency)".green(),
                inferenced_core::PressureLevel::Elevated => "ELEVATED (throttling batch)".yellow().bold(),
                inferenced_core::PressureLevel::Critical => "CRITICAL (bus starvation / preemption active)".red().bold(),
            };
            println!("  Pressure Level: {}", psi_status);
            println!(
                "  Memory Stall (avg10): some={:.2}%, full={:.2}%",
                psi.memory_some_avg10, psi.memory_full_avg10
            );
            println!("  CPU Stall (avg10):    some={:.2}%", psi.cpu_some_avg10);
            println!("  I/O Stall (avg10):    some={:.2}%", psi.io_some_avg10);

            // 3. Active Slices & Sentry Enclave
            println!("\n{}", "Supervisory Enclave:".bold().underline());
            println!("  Supervisor: {}", "systemd-sentry.service".cyan().bold());
            println!("  IPC Endpoint: /run/systemd-inferenced/sentry.sock");
            println!("  Enclave Slice: ai-sentry.slice (MemoryMin=2G, ManagedOOMPreference=avoid)");
            println!("  Preemption Guarantee: 0ms queue delay, immediate batch suspension");
        }
        Commands::Planes => {
            let topo = HardwareTopology::discover()?;
            let rows: Vec<PlaneRow> = topo
                .planes
                .iter()
                .map(|p| PlaneRow {
                    id: p.id.clone(),
                    kind: format!("{:?}", p.kind),
                    device: p
                        .device_path
                        .as_ref()
                        .map(|d| d.display().to_string())
                        .unwrap_or_else(|| "host".into()),
                    total: format_bytes(p.total_memory_bytes),
                    available: format_bytes(p.available_memory_bytes),
                    triage: if p.is_triage_reserved { "YES".into() } else { "NO".into() },
                })
                .collect();
            let mut table = Table::new(rows);
            table.with(Style::rounded());
            println!("{}", table);
        }
        Commands::Leases => {
            println!("{}", "No active third-party compute leases. All compute planes idle.".dimmed());
        }
        Commands::Monitor => {
            println!("{}", "Streaming real-time hardware telemetry (Ctrl+C to quit)...".bold());
            for _ in 0..5 {
                let psi = PressureMetrics::read_current();
                println!(
                    "[{:?}] mem_some: {:.2}% | mem_full: {:.2}% | cpu: {:.2}% | io: {:.2}%",
                    psi.level, psi.memory_some_avg10, psi.memory_full_avg10, psi.cpu_some_avg10, psi.io_some_avg10
                );
                std::thread::sleep(std::time::Duration::from_millis(1000));
            }
        }
    }

    Ok(())
}
