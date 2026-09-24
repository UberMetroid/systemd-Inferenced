use crate::client::VarlinkClient;
use anyhow::{Context, Result};
use colored::*;
use serde_json::json;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Instant;

pub fn run_test_triage(sentry_socket: impl AsRef<Path>) -> Result<()> {
    let path = sentry_socket.as_ref();
    println!("{}", "Initiating synthetic systemd-sentry emergency triage ping...".bold());
    println!("Connecting to out-of-band enclave socket: {:?}", path);

    let mut stream = UnixStream::connect(path)
        .with_context(|| format!("Failed to connect to Sentry triage socket at {:?}", path))?;

    let payload = json!({
        "incident_id": "00000000-0000-0000-0000-000000000001",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "unit_name": "systemd-sentry.service",
        "root_cause": {
            "summary": "Synthetic triage verification ping",
            "detail": "Testing zero-latency emergency preemption on dedicated slice",
        },
        "evidence": {
            "journal_lines": ["kernel: [12345.67] GPU Hang detected", "systemd: Unit crashed"],
            "signal": "SIGSEGV"
        },
        "severity": "CRITICAL",
        "proposed_remediation": {
            "action": "RestartService",
            "rationale": "Hardware enclave confirmed responsive",
            "risk_level": "LOW",
            "confidence": 0.99
        }
    });

    let bytes = serde_json::to_vec(&payload)?;
    stream.write_all(&bytes)?;
    stream.flush()?;

    let mut response_buf = [0u8; 4096];
    let n = stream.read(&mut response_buf)?;
    if n == 0 {
        anyhow::bail!("Sentry socket closed without response");
    }

    let resp_str = String::from_utf8_lossy(&response_buf[..n]);
    println!("{}: Received emergency enclave response!", "SUCCESS".green().bold());
    println!("  Payload: {}", resp_str);
    Ok(())
}

pub fn run_benchmark(socket_path: impl AsRef<Path>, iterations: usize) -> Result<()> {
    println!("{}", "Benchmarking systemd-inferenced IPC & Scheduler...".bold());
    let mut client = VarlinkClient::connect(socket_path)?;

    // 1. IPC Ping latency
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = client.get_pressure()?;
    }
    let elapsed = start.elapsed();
    let avg_us = elapsed.as_micros() as f64 / iterations as f64;
    println!("  Varlink IPC Ping ({iterations} calls): total {elapsed:?}, avg {avg_us:.2} µs/call");

    // 2. Lease Acquire & Release throughput
    let start = Instant::now();
    for _i in 0..iterations {
        let lease = client.acquire_lease("Interactive", 1024 * 1024, None)?;
        if let Some(id) = lease.get("lease_id").and_then(|v| v.as_str()) {
            client.release_lease(id)?;
        }
    }
    let elapsed = start.elapsed();
    let ops = (iterations * 2) as f64 / elapsed.as_secs_f64();
    println!("  Lease Lifecycle Throughput: {ops:.0} ops/sec ({elapsed:?})");
    Ok(())
}
