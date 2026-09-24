use super::types::*;
use axum::{
    body::Body,
    extract::State,
    http::{header::CONTENT_TYPE, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use inferenced_core::{arbiter::Arbiter, lease::LeasePriority, preempt::PreemptCoordinator};
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::info;

pub struct AppState {
    pub arbiter: Arc<Arbiter>,
    #[allow(dead_code)]
    pub preempt: Arc<PreemptCoordinator>,
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
            )
                .into_response();
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
        .into_response()
}

pub async fn ollama_generate_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let model = payload
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();
    let prompt = payload
        .get("prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let stream_requested = payload
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

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
            )
                .into_response();
        }
    };

    let arbiter = state.arbiter.clone();
    let lease_id = lease.id;

    if !stream_requested {
        let _ = arbiter.release_lease(lease_id).await;
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "model": model,
                "created_at": chrono::Utc::now().to_rfc3339(),
                "response": format!("systemd-inferenced: processed prompt '{}'", prompt),
                "done": true,
                "total_duration": 1500000,
            })),
        )
            .into_response();
    }

    // Streaming response with NDJSON chunks
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<axum::body::Bytes, Infallible>>(16);
    let stream = ReceiverStream::new(rx);
    let body = Body::from_stream(stream);

    tokio::spawn(async move {
        let tokens = vec![
            "systemd-inferenced: ".to_string(),
            "streaming ".to_string(),
            "token ".to_string(),
            "generation ".to_string(),
            format!("for prompt '{}'.", prompt),
        ];

        for tok in tokens {
            let chunk = serde_json::json!({
                "model": model,
                "created_at": chrono::Utc::now().to_rfc3339(),
                "response": tok,
                "done": false
            });
            let mut bytes = serde_json::to_vec(&chunk).unwrap_or_default();
            bytes.push(b'\n');
            if tx.send(Ok(bytes.into())).await.is_err() {
                break;
            }
        }

        let final_chunk = serde_json::json!({
            "model": model,
            "created_at": chrono::Utc::now().to_rfc3339(),
            "response": "",
            "done": true,
            "total_duration": 1500000
        });
        let mut final_bytes = serde_json::to_vec(&final_chunk).unwrap_or_default();
        final_bytes.push(b'\n');
        let _ = tx.send(Ok(final_bytes.into())).await;

        let _ = arbiter.release_lease(lease_id).await;
    });

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/x-ndjson")
        .body(body)
        .unwrap()
        .into_response()
}
