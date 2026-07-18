use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};

pub fn routes() -> Router {
    Router::new()
        .route("/", get(list_workspaces))
        .route("/", post(create_workspace))
}

async fn list_workspaces() -> Json<Vec<String>> {
    Json(vec![])
}

async fn create_workspace() -> Json<&'static str> {
    Json("workspace created")
}
