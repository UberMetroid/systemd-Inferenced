use super::protocol::VarlinkReply;
use inferenced_core::{
    arbiter::Arbiter,
    lease::{LeaseId, LeasePriority},
    model::{ModelDescriptor, ModelPlacementState},
    psi::PressureMetrics,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::unix::OwnedWriteHalf;
use uuid::Uuid;

pub async fn handle_method(
    method: &str,
    params: Option<&Value>,
    arbiter: &Arc<Arbiter>,
    writer: &mut Option<OwnedWriteHalf>,
) -> Option<VarlinkReply> {
    match method {
        "io.systemd.inferenced1.GetTopology" => Some(handle_get_topology(arbiter).await),
        "io.systemd.inferenced1.GetPressure" => Some(handle_get_pressure()),
        "io.systemd.inferenced1.AcquireLease" => Some(handle_acquire_lease(params, arbiter).await),
        "io.systemd.inferenced1.ReleaseLease" => Some(handle_release_lease(params, arbiter).await),
        "io.systemd.inferenced1.Yield" => Some(handle_yield(params, arbiter).await),
        "io.systemd.inferenced1.Freeze" => Some(handle_freeze(params, arbiter).await),
        "io.systemd.inferenced1.Thaw" => Some(handle_thaw(params, arbiter).await),
        "io.systemd.inferenced1.ListLeases" => Some(handle_list_leases(arbiter).await),
        "io.systemd.inferenced1.ListModels" => Some(handle_list_models(arbiter).await),
        "io.systemd.inferenced1.RegisterModel" => Some(handle_register_model(params, arbiter).await),
        "io.systemd.inferenced1.StreamInference" => {
            handle_stream_inference(params, arbiter, writer).await;
            None
        }
        _ => None,
    }
}

async fn handle_get_topology(arbiter: &Arc<Arbiter>) -> VarlinkReply {
    let topo = arbiter.get_topology().await;
    let planes: Vec<Value> = topo.planes.into_iter().map(|p| {
        json!({
            "id": p.id,
            "name": p.name,
            "kind": format!("{:?}", p.kind),
            "total_memory": p.total_memory_bytes,
            "available_memory": p.available_memory_bytes,
            "is_triage_reserved": p.is_triage_reserved,
        })
    }).collect();

    VarlinkReply::ok(json!({
        "planes": planes,
        "total_ram": topo.total_system_ram_bytes,
        "available_ram": topo.available_system_ram_bytes,
        "cpu_cores": topo.cpu_cores_total,
    }))
}

fn handle_get_pressure() -> VarlinkReply {
    let psi = PressureMetrics::read_current();
    VarlinkReply::ok(json!({
        "level": format!("{:?}", psi.level),
        "cpu_some": psi.cpu_some_avg10,
        "memory_some": psi.memory_some_avg10,
        "io_some": psi.io_some_avg10,
    }))
}

async fn handle_acquire_lease(params: Option<&Value>, arbiter: &Arc<Arbiter>) -> VarlinkReply {
    let params = match params {
        Some(p) => p,
        None => return VarlinkReply::error("org.varlink.service.InvalidParameter", json!({"parameter": "parameters"})),
    };

    let prio_str = params.get("priority").and_then(|v| v.as_str()).unwrap_or("Interactive");
    let priority = match prio_str {
        "Batch" => LeasePriority::Batch,
        "EmergencyTriage" => LeasePriority::EmergencyTriage,
        _ => LeasePriority::Interactive,
    };

    let mem_bytes = params.get("memory_bytes").and_then(|v| v.as_u64()).unwrap_or(1024 * 1024 * 1024);
    let plane = params.get("plane").and_then(|v| v.as_str()).map(ToString::to_string);
    let unit = params.get("unit").and_then(|v| v.as_str()).map(ToString::to_string);
    let pid = params.get("pid").and_then(|v| v.as_u64()).map(|p| p as u32);

    match arbiter.acquire_lease(priority, mem_bytes, plane, unit, pid).await {
        Ok(lease) => VarlinkReply::ok(json!({
            "lease_id": lease.id.to_string(),
            "plane_id": lease.plane_id,
            "allocated_memory": lease.allocated_memory_bytes,
        })),
        Err(e) => VarlinkReply::error("io.systemd.inferenced1.ResourceExhaustion", json!({"error": e.to_string()})),
    }
}

async fn handle_release_lease(params: Option<&Value>, arbiter: &Arc<Arbiter>) -> VarlinkReply {
    let id_str = params.and_then(|p| p.get("lease_id")).and_then(|v| v.as_str()).unwrap_or("");
    let lease_id = match Uuid::parse_str(id_str) {
        Ok(u) => LeaseId(u),
        Err(_) => return VarlinkReply::error("org.varlink.service.InvalidParameter", json!({"parameter": "lease_id"})),
    };

    match arbiter.release_lease(lease_id).await {
        Ok(_) => VarlinkReply::ok(json!({})),
        Err(e) => VarlinkReply::error("io.systemd.inferenced1.LeaseNotFound", json!({"error": e.to_string()})),
    }
}

async fn handle_yield(params: Option<&Value>, arbiter: &Arc<Arbiter>) -> VarlinkReply {
    let id_str = params.and_then(|p| p.get("lease_id")).and_then(|v| v.as_str()).unwrap_or("");
    if let Ok(u) = Uuid::parse_str(id_str) {
        let _ = arbiter.yield_lease(LeaseId(u)).await;
    }
    VarlinkReply::ok(json!({}))
}

async fn handle_freeze(params: Option<&Value>, arbiter: &Arc<Arbiter>) -> VarlinkReply {
    let id_str = params.and_then(|p| p.get("lease_id")).and_then(|v| v.as_str()).unwrap_or("");
    if let Ok(u) = Uuid::parse_str(id_str) {
        let _ = arbiter.freeze_lease(LeaseId(u)).await;
    }
    VarlinkReply::ok(json!({}))
}

async fn handle_thaw(params: Option<&Value>, arbiter: &Arc<Arbiter>) -> VarlinkReply {
    let id_str = params.and_then(|p| p.get("lease_id")).and_then(|v| v.as_str()).unwrap_or("");
    if let Ok(u) = Uuid::parse_str(id_str) {
        let _ = arbiter.thaw_lease(LeaseId(u)).await;
    }
    VarlinkReply::ok(json!({}))
}

async fn handle_list_leases(arbiter: &Arc<Arbiter>) -> VarlinkReply {
    let leases = arbiter.list_leases().await;
    let list: Vec<Value> = leases.into_iter().map(|l| {
        json!({
            "id": l.id.to_string(),
            "plane_id": l.plane_id,
            "allocated_memory": l.allocated_memory_bytes,
            "priority": format!("{:?}", l.priority),
            "state": format!("{:?}", l.state),
            "client_unit": l.client_unit,
            "client_pid": l.client_pid,
        })
    }).collect();
    VarlinkReply::ok(json!({ "leases": list }))
}

async fn handle_list_models(arbiter: &Arc<Arbiter>) -> VarlinkReply {
    let models = arbiter.list_models().await;
    let list: Vec<Value> = models.into_iter().map(|m| {
        json!({
            "id": m.id,
            "format": m.format,
            "path": m.path.to_string_lossy(),
            "estimated_memory": m.estimated_memory_bytes,
            "placement": format!("{:?}", m.placement),
        })
    }).collect();
    VarlinkReply::ok(json!({ "models": list }))
}

async fn handle_register_model(params: Option<&Value>, arbiter: &Arc<Arbiter>) -> VarlinkReply {
    let p = match params {
        Some(v) => v,
        None => return VarlinkReply::error("org.varlink.service.InvalidParameter", json!({"parameter": "parameters"})),
    };
    let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let format = p.get("format").and_then(|v| v.as_str()).unwrap_or("GGUF").to_string();
    let path = PathBuf::from(p.get("path").and_then(|v| v.as_str()).unwrap_or(""));
    let estimated = p.get("estimated_bytes").and_then(|v| v.as_u64()).unwrap_or(0);

    let desc = ModelDescriptor {
        id,
        format,
        path,
        estimated_memory_bytes: estimated,
        placement: ModelPlacementState::Dormant,
        resident_plane_id: None,
        last_accessed: chrono::Utc::now(),
        access_count: 0,
        preferred_plane: None,
    };
    arbiter.register_model(desc).await;
    VarlinkReply::ok(json!({}))
}

async fn handle_stream_inference(
    params: Option<&Value>,
    _arbiter: &Arc<Arbiter>,
    writer: &mut Option<OwnedWriteHalf>,
) {
    let w = match writer {
        Some(w) => w,
        None => return,
    };
    let prompt = params.and_then(|p| p.get("prompt")).and_then(|v| v.as_str()).unwrap_or("");
    let tokens = [
        "Inference: ",
        "Analyzing prompt '",
        prompt,
        "'. Execution dispatched via systemd-inferenced.",
    ];

    for (i, token) in tokens.iter().enumerate() {
        let continues = i < tokens.len() - 1;
        let reply = VarlinkReply::streaming(json!({ "chunk": token }), continues);
        let _ = w.write_all(&reply.to_bytes()).await;
    }
}
