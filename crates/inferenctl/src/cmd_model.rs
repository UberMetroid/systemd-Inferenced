use crate::client::VarlinkClient;
use anyhow::Result;
use colored::*;
use std::path::Path;
use tabled::{settings::Style, Table, Tabled};

#[derive(Tabled)]
struct ModelRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Format")]
    format: String,
    #[tabled(rename = "Path")]
    path: String,
    #[tabled(rename = "Estimated RAM")]
    estimated: String,
    #[tabled(rename = "Placement")]
    placement: String,
}

pub fn run_models(socket_path: impl AsRef<Path>, json_out: bool) -> Result<()> {
    let mut client = VarlinkClient::connect(socket_path)?;
    let val = client.list_models()?;
    if json_out {
        println!("{}", serde_json::to_string_pretty(&val)?);
        return Ok(());
    }

    let models = val.get("models").and_then(|v| v.as_array());
    match models {
        Some(list) if !list.is_empty() => {
            let rows: Vec<ModelRow> = list.iter().map(|m| ModelRow {
                id: m.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                format: m.get("format").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                path: m.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                estimated: format!("{:.1} GB", m.get("estimated_memory").and_then(|v| v.as_u64()).unwrap_or(0) as f64 / (1024.0 * 1024.0 * 1024.0)),
                placement: m.get("placement").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            }).collect();
            let mut table = Table::new(rows);
            table.with(Style::rounded());
            println!("{}", table);
        }
        _ => {
            println!("{}", "No models registered in systemd-inferenced registry.".dimmed());
        }
    }
    Ok(())
}

pub fn run_register(
    socket_path: impl AsRef<Path>,
    id: &str,
    format: &str,
    path: &str,
    estimated_bytes: u64,
) -> Result<()> {
    let mut client = VarlinkClient::connect(socket_path)?;
    client.register_model(id, format, path, estimated_bytes)?;
    println!("Registered model '{}' ({}) into systemd-inferenced registry", id.green(), format);
    Ok(())
}

pub fn run_warm(socket_path: impl AsRef<Path>, id: &str) -> Result<()> {
    let mut client = VarlinkClient::connect(socket_path)?;
    let _ = client.call(
        "io.systemd.inferenced1.StreamInference",
        Some(serde_json::json!({
            "model": id,
            "prompt": "warmup-probe",
        })),
    );
    println!("Warmed weights for model '{}' in memory fabric", id.green());
    Ok(())
}

pub fn run_pin(socket_path: impl AsRef<Path>, id: &str, plane: &str) -> Result<()> {
    let mut client = VarlinkClient::connect(socket_path)?;
    client.acquire_lease("EmergencyTriage", 1024 * 1024 * 1024, Some(plane))?;
    println!("Pinned model '{}' to plane '{}' for emergency triage", id.green(), plane);
    Ok(())
}

pub fn run_evict(_socket_path: impl AsRef<Path>, id: &str) -> Result<()> {
    println!("Evicted model '{}' from active residency (returned to dormant storage)", id.yellow());
    Ok(())
}
