use super::*;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use inferenced_core::{
    arbiter::Arbiter,
    preempt::PreemptCoordinator,
    topology::{ComputePlane, ComputePlaneKind, HardwareTopology},
};
use tower::ServiceExt;

fn make_test_state() -> Arc<AppState> {
    let mut topo = HardwareTopology::default();
    topo.planes.push(ComputePlane {
        id: "plane-gateway-test".into(),
        name: "Gateway GPU".into(),
        kind: ComputePlaneKind::DiscreteGpu,
        device_path: None,
        total_memory_bytes: 8 * 1024 * 1024 * 1024,
        available_memory_bytes: 8 * 1024 * 1024 * 1024,
        numa_node: None,
        supported_formats: vec![],
        is_triage_reserved: false,
        hardware_features: vec![],
    });
    let arbiter = Arc::new(Arbiter::new(topo));
    let preempt = Arc::new(PreemptCoordinator::new(arbiter.clone()));
    Arc::new(AppState { arbiter, preempt })
}

#[tokio::test]
async fn test_gateway_health_endpoint() {
    let state = make_test_state();
    let app = build_gateway_router(state);

    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["planes"], 1);
}

#[tokio::test]
async fn test_gateway_ollama_generate_streaming_and_non_streaming() {
    let state = make_test_state();
    let app = build_gateway_router(state);

    // 1. Non-streaming
    let req_non_stream = Request::builder()
        .method("POST")
        .uri("/api/generate")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&serde_json::json!({
                "model": "qwen2.5-coder:7b",
                "prompt": "write hello world",
                "stream": false
            }))
            .unwrap(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(req_non_stream).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["model"], "qwen2.5-coder:7b");
    assert_eq!(json["done"], true);

    // 2. Streaming
    let req_stream = Request::builder()
        .method("POST")
        .uri("/api/generate")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&serde_json::json!({
                "model": "qwen2.5-coder:7b",
                "prompt": "write hello world",
                "stream": true
            }))
            .unwrap(),
        ))
        .unwrap();

    let resp = app.oneshot(req_stream).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("ndjson"));

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = body_str.trim().lines().collect();
    assert!(lines.len() >= 2, "Must produce multiple streaming NDJSON chunks");
    let last_chunk: serde_json::Value = serde_json::from_str(lines.last().unwrap()).unwrap();
    assert_eq!(last_chunk["done"], true);
}
