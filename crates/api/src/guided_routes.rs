use ade_core::error::AdeError;
use ade_core::guided::{self, GuidedWinId, GuidedWinsState, UnderstandResult};
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::router::{ApiError, ApiResult, ApiState};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/wins", get(guided_wins).post(mark_guided_win))
        .route("/understand", post(understand_project))
}

async fn guided_wins(State(state): State<ApiState>) -> ApiResult<GuidedWinsState> {
    guided::load_wins(state.workspace_root())
        .map(Json)
        .map_err(map_ade)
}

#[derive(Debug, Deserialize)]
pub struct MarkWinBody {
    pub win: String,
}

async fn mark_guided_win(
    State(state): State<ApiState>,
    Json(body): Json<MarkWinBody>,
) -> ApiResult<GuidedWinsState> {
    let win = parse_win(&body.win)?;
    guided::mark_win(state.workspace_root(), win)
        .map(Json)
        .map_err(map_ade)
}

async fn understand_project(State(state): State<ApiState>) -> ApiResult<UnderstandResult> {
    guided::write_understand_project(state.workspace_root())
        .map(Json)
        .map_err(map_ade)
}

fn parse_win(raw: &str) -> Result<GuidedWinId, ApiError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "understand" => Ok(GuidedWinId::Understand),
        "verify" => Ok(GuidedWinId::Verify),
        "improve_ade" | "improve-ade" | "improve" => Ok(GuidedWinId::ImproveAde),
        other => Err(ApiError::bad_request(format!(
            "unknown guided win '{other}'"
        ))),
    }
}

fn map_ade(error: AdeError) -> ApiError {
    ApiError::internal(error)
}
