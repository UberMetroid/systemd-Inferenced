use crate::client::VarlinkClient;
use anyhow::{Context, Result};
use colored::*;
use std::path::Path;

pub fn run_freeze(socket_path: impl AsRef<Path>, lease_id: &str) -> Result<()> {
    let mut client = VarlinkClient::connect(socket_path.as_ref())
        .with_context(|| format!("Failed to connect to systemd-inferenced at {:?}", socket_path.as_ref()))?;
    client.freeze(lease_id)?;
    println!("Froze compute lease {}", lease_id.green());
    Ok(())
}

pub fn run_thaw(socket_path: impl AsRef<Path>, lease_id: &str) -> Result<()> {
    let mut client = VarlinkClient::connect(socket_path.as_ref())
        .with_context(|| format!("Failed to connect to systemd-inferenced at {:?}", socket_path.as_ref()))?;
    client.thaw(lease_id)?;
    println!("Thawed compute lease {}", lease_id.green());
    Ok(())
}
