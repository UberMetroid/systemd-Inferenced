pub mod handlers;
pub mod types;

pub use handlers::AppState;

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub fn build_gateway_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(handlers::health_handler))
        .route("/v1/models", get(handlers::models_handler))
        .route("/v1/chat/completions", post(handlers::chat_completions_handler))
        .route("/api/tags", get(handlers::models_handler))
        .route("/api/generate", post(handlers::ollama_generate_handler))
        .with_state(state)
}
