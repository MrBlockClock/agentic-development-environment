use axum::{routing::get, Json, Router};

pub fn routes() -> Router {
    Router::new()
        .route("/usage", get(usage_report))
        .route("/quality", get(quality_report))
        .route("/costs", get(cost_report))
}

async fn usage_report() -> Json<&'static str> {
    Json("usage analytics")
}

async fn quality_report() -> Json<&'static str> {
    Json("quality analytics")
}

async fn cost_report() -> Json<&'static str> {
    Json("cost analytics")
}
