use axum::{Router, routing::get};

pub fn build_router() -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/api/workspaces", get(|| async { "workspaces" }))
        .route("/api/teams", get(|| async { "teams" }))
        .route("/api/analytics", get(|| async { "analytics" }))
}
