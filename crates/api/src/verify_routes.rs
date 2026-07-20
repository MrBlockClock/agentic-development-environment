use crate::router::{map_ade_error, ApiError, ApiResult, ApiState};
use ade_core::verify::{VerifyGate, VerifyResult};
use ade_workflow::verify::VerifyRunner;
use axum::{extract::State, Json};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct VerifyRequest {
    pub gate: String,
    #[serde(default)]
    pub through: bool,
}

pub(crate) async fn run_verify(
    State(state): State<ApiState>,
    Json(body): Json<VerifyRequest>,
) -> ApiResult<Vec<VerifyResult>> {
    let gate: VerifyGate = body.gate.parse().map_err(ApiError::bad_request)?;
    let runner = VerifyRunner::with_root(state.workspace_root());
    let results = if body.through {
        runner.run_through(gate).await
    } else {
        vec![runner.run_gate(gate).await]
    };

    // Mirror desktop: fold results into the latest handoff capsule when present.
    let manager = ade_agents::handoff::HandoffManager::new(state.workspace_root());
    let mut capsule = manager.load_latest().unwrap_or_else(|_| {
        ade_core::handoff::HandoffCapsule::new(
            "Continue after workspace verification",
            "evaluate_existing",
        )
    });
    capsule.apply_verify_results(&results);
    manager.save_capsule(&capsule).map_err(map_ade_error)?;

    Ok(Json(results))
}
