use crate::client::VarlinkClient;
use anyhow::{bail, Context, Result};
use colored::*;
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
    let mut client = VarlinkClient::connect(socket_path)?;
    let topo = client.get_topology()?;
    let pressure = client.get_pressure()?;
    let leases = client.list_leases()?;
    let models = client.list_models().unwrap_or_default();
    let info = client.get_info().unwrap_or_default();

    let dump = serde_json::json!({
        "info": info,
        "topology": topo,
        "pressure": pressure,
        "leases": leases,
        "models": models,
    });
    println!("{}", serde_json::to_string_pretty(&dump)?);
    Ok(())
}

pub fn run_inspect(socket_path: impl AsRef<Path>, plane_id: &str) -> Result<()> {
    let mut client = VarlinkClient::connect(socket_path)?;
    let topo = client.get_topology()?;
    let planes = topo.get("planes").and_then(|v| v.as_array());

    if let Some(list) = planes {
        for p in list {
            if p.get("id").and_then(|v| v.as_str()) == Some(plane_id) {
                println!("{}", format!("Compute Plane: {}", plane_id).bold().underline());
                println!("  Name:      {}", p.get("name").and_then(|v| v.as_str()).unwrap_or(""));
                println!("  Kind:      {}", p.get("kind").and_then(|v| v.as_str()).unwrap_or(""));
                println!("  Total:     {} bytes", p.get("total_memory").and_then(|v| v.as_u64()).unwrap_or(0));
                println!("  Available: {} bytes", p.get("available_memory").and_then(|v| v.as_u64()).unwrap_or(0));
                println!("  Triage:    {}", p.get("is_triage_reserved").and_then(|v| v.as_bool()).unwrap_or(false));
                return Ok(());
            }
        }
    }

    bail!("Compute plane '{}' not found", plane_id);
}
