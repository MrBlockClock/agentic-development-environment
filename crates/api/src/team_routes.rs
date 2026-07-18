use axum::{
    routing::{get, post},
    Json, Router,
};

pub fn routes() -> Router {
    Router::new()
        .route("/", get(list_teams))
        .route("/", post(create_team))
}

async fn list_teams() -> Json<Vec<String>> {
    Json(vec![])
}

async fn create_team() -> Json<&'static str> {
    Json("team created")
}
