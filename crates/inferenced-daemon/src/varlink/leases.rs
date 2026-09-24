use super::protocol::VarlinkReply;
use inferenced_core::{
    arbiter::Arbiter,
    lease::{LeaseId, LeasePriority},
};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

pub async fn handle_acquire_lease(
    params: Option<&Value>,
    arbiter: &Arc<Arbiter>,
    active_leases: &mut Vec<LeaseId>,
) -> VarlinkReply {
    let params = match params {
        Some(p) => p,
        None => {
            return VarlinkReply::error(
                "org.varlink.service.InvalidParameter",
                json!({"parameter": "parameters"}),
            )
        }
    };

    let prio_str = params
        .get("priority")
        .and_then(|v| v.as_str())
        .unwrap_or("Interactive");
    let priority = match prio_str {
        "Batch" => LeasePriority::Batch,
        "EmergencyTriage" => LeasePriority::EmergencyTriage,
        _ => LeasePriority::Interactive,
    };

    let mem_bytes = params
        .get("memory_bytes")
        .and_then(|v| v.as_u64())
        .unwrap_or(1024 * 1024 * 1024);
    let plane = params
        .get("plane")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    let unit = params
        .get("unit")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    let pid = params.get("pid").and_then(|v| v.as_u64()).map(|p| p as u32);

    match arbiter
        .acquire_lease(priority, mem_bytes, plane, unit, pid)
        .await
    {
        Ok(lease) => {
            active_leases.push(lease.id);
            VarlinkReply::ok(json!({
                "lease_id": lease.id.to_string(),
                "plane_id": lease.plane_id,
                "allocated_memory": lease.allocated_memory_bytes,
            }))
        }
        Err(e) => VarlinkReply::error(
            "io.systemd.inferenced1.ResourceExhaustion",
            json!({"error": e.to_string()}),
        ),
    }
}

pub async fn handle_release_lease(
    params: Option<&Value>,
    arbiter: &Arc<Arbiter>,
    active_leases: &mut Vec<LeaseId>,
) -> VarlinkReply {
    let id_str = params
        .and_then(|p| p.get("lease_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let lease_id = match Uuid::parse_str(id_str) {
        Ok(u) => LeaseId(u),
        Err(_) => {
            return VarlinkReply::error(
                "org.varlink.service.InvalidParameter",
                json!({"parameter": "lease_id"}),
            )
        }
    };

    match arbiter.release_lease(lease_id).await {
        Ok(_) => {
            active_leases.retain(|&id| id != lease_id);
            VarlinkReply::ok(json!({}))
        }
        Err(e) => VarlinkReply::error(
            "io.systemd.inferenced1.LeaseNotFound",
            json!({"error": e.to_string()}),
        ),
    }
}

pub async fn handle_yield(params: Option<&Value>, arbiter: &Arc<Arbiter>) -> VarlinkReply {
    let id_str = params
        .and_then(|p| p.get("lease_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if let Ok(u) = Uuid::parse_str(id_str) {
        let _ = arbiter.yield_lease(LeaseId(u)).await;
    }
    VarlinkReply::ok(json!({}))
}

pub async fn handle_freeze(params: Option<&Value>, arbiter: &Arc<Arbiter>) -> VarlinkReply {
    let id_str = params
        .and_then(|p| p.get("lease_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if let Ok(u) = Uuid::parse_str(id_str) {
        let _ = arbiter.freeze_lease(LeaseId(u)).await;
    }
    VarlinkReply::ok(json!({}))
}

pub async fn handle_thaw(params: Option<&Value>, arbiter: &Arc<Arbiter>) -> VarlinkReply {
    let id_str = params
        .and_then(|p| p.get("lease_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if let Ok(u) = Uuid::parse_str(id_str) {
        let _ = arbiter.thaw_lease(LeaseId(u)).await;
    }
    VarlinkReply::ok(json!({}))
}

pub async fn handle_list_leases(arbiter: &Arc<Arbiter>) -> VarlinkReply {
    let leases = arbiter.list_leases().await;
    let list: Vec<Value> = leases
        .into_iter()
        .map(|l| {
            json!({
                "id": l.id.to_string(),
                "plane_id": l.plane_id,
                "allocated_memory": l.allocated_memory_bytes,
                "priority": format!("{:?}", l.priority),
                "state": format!("{:?}", l.state),
                "client_unit": l.client_unit,
                "client_pid": l.client_pid,
            })
        })
        .collect();
    VarlinkReply::ok(json!({ "leases": list }))
}
