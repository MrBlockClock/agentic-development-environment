use axum::{routing::get, Json, Router};

pub fn routes() -> Router {
    Router::new().route("/", get(list_projects))
}

async fn list_projects() -> Json<Vec<String>> {
    Json(vec![])
}
