use axum::{routing::get, Json, Router};

pub fn routes() -> Router {
    Router::new()
        .route("/", get(list_audit_entries))
        .route("/stream", get(stream_audit))
}

async fn list_audit_entries() -> Json<Vec<String>> {
    Json(vec![])
}

async fn stream_audit() -> Json<&'static str> {
    Json("audit stream")
}
