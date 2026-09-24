use crate::client::VarlinkClient;
use crate::format::format_bytes;
use anyhow::{bail, Context, Result};
use colored::*;
use inferenced_core::topology::HardwareTopology;
use serde_json::json;
use std::fs;
use std::path::Path;

pub fn run_cat_config(config_path: impl AsRef<Path>) -> Result<()> {
    let path = config_path.as_ref();
    if !path.exists() {
        println!("# Default built-in configuration for systemd-inferenced");
        println!("[daemon]");
        println!("bind = \"127.0.0.1:11434\"");
        println!("varlink_socket = \"/run/systemd-inferenced/io.systemd.inferenced1\"");
        println!("sentry_socket = \"/run/systemd-inferenced/sentry.sock\"");
        println!("fd_socket = \"/run/systemd-inferenced/fd.sock\"");
        return Ok(());
    }
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config from {:?}", path))?;
    print!("{}", content);
    Ok(())
}

pub fn run_check_config(config_path: impl AsRef<Path>) -> Result<()> {
    let path = config_path.as_ref();
    if !path.exists() {
        println!("{}: Configuration file {:?} not found (using defaults)", "WARNING".yellow(), path);
        return Ok(());
    }
    let content = fs::read_to_string(path)?;
    let parsed: toml::Value = toml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Configuration syntax error in {:?}: {}", path, e))?;

    println!("{}: Configuration {:?} is valid syntax TOML.", "OK".green().bold(), path);
    if let Some(table) = parsed.as_table() {
        println!("  Sections defined: {:?}", table.keys().collect::<Vec<_>>());
    }
    Ok(())
}

pub fn run_dump(socket_path: impl AsRef<Path>) -> Result<()> {
    let mut client = VarlinkClient::connect(socket_path.as_ref())
        .with_context(|| format!("Failed to connect to systemd-inferenced at {:?}", socket_path.as_ref()))?;
    let topo = client.get_topology()?;
    let pressure = client.get_pressure()?;
    let leases = client.list_leases()?;
    let models = client.list_models().unwrap_or_default();
    let info = client.get_info().unwrap_or_default();

    let dump = json!({
        "info": info,
        "topology": topo,
        "pressure": pressure,
        "leases": leases,
        "models": models,
    });
    println!("{}", serde_json::to_string_pretty(&dump)?);
    Ok(())
}

pub fn run_inspect(socket_path: impl AsRef<Path>, plane_id: &str, json_out: bool) -> Result<()> {
    let path = socket_path.as_ref();
    if let Ok(mut client) = VarlinkClient::connect(path) {
        if let Ok(topo) = client.get_topology() {
            if let Some(planes) = topo.get("planes").and_then(|v| v.as_array()) {
                for p in planes {
                    if p.get("id").and_then(|v| v.as_str()) == Some(plane_id) {
                        return print_plane_details(p, json_out);
                    }
                }
            }
        }
    }

    let topo = HardwareTopology::discover().unwrap_or_default();
    for p in &topo.planes {
        if p.id == plane_id {
            let p_val = json!({
                "id": p.id,
                "name": p.name,
                "kind": format!("{:?}", p.kind),
                "total_memory": p.total_memory_bytes,
                "available_memory": p.available_memory_bytes,
                "is_triage_reserved": p.is_triage_reserved,
                "features": p.hardware_features,
            });
            return print_plane_details(&p_val, json_out);
        }
    }

    bail!("Compute plane '{}' not found", plane_id);
}

fn print_plane_details(p: &serde_json::Value, json_out: bool) -> Result<()> {
    if json_out {
        println!("{}", serde_json::to_string_pretty(p)?);
        return Ok(());
    }
    let plane_id = p.get("id").and_then(|v| v.as_str()).unwrap_or("");
    println!("{}", format!("Compute Plane: {}", plane_id).bold().underline());
    println!("  Name:      {}", p.get("name").and_then(|v| v.as_str()).unwrap_or(""));
    println!("  Kind:      {}", p.get("kind").and_then(|v| v.as_str()).unwrap_or(""));
    let total = p.get("total_memory").and_then(|v| v.as_u64()).unwrap_or(0);
    let avail = p.get("available_memory").and_then(|v| v.as_u64()).unwrap_or(0);
    println!("  Total:     {} ({})", total, format_bytes(total));
    println!("  Available: {} ({})", avail, format_bytes(avail));
    println!("  Triage:    {}", p.get("is_triage_reserved").and_then(|v| v.as_bool()).unwrap_or(false));
    if let Some(feats) = p.get("features").and_then(|v| v.as_array()) {
        let f_str: Vec<_> = feats.iter().filter_map(|f| f.as_str()).collect();
        println!("  Features:  {}", f_str.join(", "));
    }
    Ok(())
}
