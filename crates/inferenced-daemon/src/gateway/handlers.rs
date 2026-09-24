use super::types::*;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use inferenced_core::{arbiter::Arbiter, lease::LeasePriority};
use std::sync::Arc;
use tracing::info;

pub struct AppState {
    pub arbiter: Arc<Arbiter>,
}

pub async fn health_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let topo = state.arbiter.get_topology().await;
    Json(HealthResponse {
        status: "ok".into(),
        planes: topo.planes.len(),
        daemon: "systemd-inferenced v0.1.0".into(),
    })
}

pub async fn models_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "object": "list",
        "data": [
            { "id": "qwen2.5-coder:7b", "object": "model", "owned_by": "systemd-inferenced" },
            { "id": "llama3.2:1b", "object": "model", "owned_by": "systemd-inferenced" }
        ]
    }))
}

pub async fn chat_completions_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatCompletionRequest>,
) -> impl IntoResponse {
    let memory_needed = 2 * 1024 * 1024 * 1024;
    let lease = match state
        .arbiter
        .acquire_lease(
            LeasePriority::Interactive,
            memory_needed,
            None,
            Some("gateway-client".into()),
            None,
        )
        .await
    {
        Ok(l) => l,
        Err(e) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": e.to_string() })),
            );
        }
    };

    info!("Executing inference for model {} under lease {}", req.model, lease.id);
    let _ = state.arbiter.release_lease(lease.id).await;

    (
        StatusCode::OK,
        Json(serde_json::json!(ChatCompletionResponse {
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
            model: req.model,
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".into(),
                    content: "systemd-inferenced: Model compute slice allocated and verified.".into(),
                },
                finish_reason: "stop".into(),
            }],
        })),
    )
}

pub async fn ollama_generate_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let model = payload.get("model").and_then(|v| v.as_str()).unwrap_or("default");
    let memory_needed = 2 * 1024 * 1024 * 1024;

    let lease = match state
        .arbiter
        .acquire_lease(
            LeasePriority::Interactive,
            memory_needed,
            None,
            Some("ollama-client".into()),
            None,
        )
        .await
    {
        Ok(l) => l,
        Err(e) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": e.to_string() })),
            );
        }
    };

    let _ = state.arbiter.release_lease(lease.id).await;
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "model": model,
            "response": "systemd-inferenced: compute lease scheduled and released.",
            "done": true
        })),
    )
}
