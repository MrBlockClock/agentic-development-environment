use axum::{routing::post, Json, Router};

pub fn routes() -> Router {
    Router::new()
        .route("/audit", post(run_audit))
        .route("/plan", post(run_plan))
        .route("/execute", post(run_execute))
}

async fn run_audit() -> Json<&'static str> {
    Json("audit phase")
}

async fn run_plan() -> Json<&'static str> {
    Json("plan phase")
}

async fn run_execute() -> Json<&'static str> {
    Json("execute phase")
}
