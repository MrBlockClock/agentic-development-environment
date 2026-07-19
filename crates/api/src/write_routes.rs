use crate::auth::ApiScope;
use crate::router::{map_ade_error, require_approve, require_scope, ApiError, ApiResult, ApiState};
use ade_workflow::parallel::{LeaseManager, PathLease};
use ade_workflow::tasks::{AgentTask, TaskCoordinator};
use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Duration;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ClaimTaskRequest {
    pub agent_id: Uuid,
    #[serde(default = "default_ttl_secs")]
    pub ttl_secs: u64,
    pub approve: bool,
}

#[derive(Debug, Deserialize)]
pub struct TaskAgentRequest {
    pub agent_id: Uuid,
    pub approve: bool,
}

#[derive(Debug, Deserialize)]
pub struct HeartbeatTaskRequest {
    pub agent_id: Uuid,
    #[serde(default = "default_ttl_secs")]
    pub ttl_secs: u64,
    pub approve: bool,
}

#[derive(Debug, Deserialize)]
pub struct FailTaskRequest {
    pub agent_id: Uuid,
    pub failure: String,
    pub approve: bool,
}

#[derive(Debug, Deserialize)]
pub struct RenewLeaseRequest {
    pub agent_id: Uuid,
    #[serde(default = "default_ttl_secs")]
    pub ttl_secs: u64,
    pub approve: bool,
}

fn default_ttl_secs() -> u64 {
    300
}

fn ttl(secs: u64) -> Result<Duration, ApiError> {
    if secs == 0 {
        return Err(ApiError::bad_request("ttl_secs must be positive"));
    }
    Ok(Duration::seconds(secs as i64))
}

pub(crate) async fn claim_task(
    State(state): State<ApiState>,
    Json(body): Json<ClaimTaskRequest>,
) -> ApiResult<Option<AgentTask>> {
    require_scope(&state, ApiScope::TasksWrite)?;
    require_approve(body.approve, "task claim")?;
    TaskCoordinator::new(state.workspace_root())
        .claim(body.agent_id, ttl(body.ttl_secs)?)
        .map(Json)
        .map_err(map_ade_error)
}

pub(crate) async fn start_task(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
    Json(body): Json<TaskAgentRequest>,
) -> ApiResult<AgentTask> {
    require_scope(&state, ApiScope::TasksWrite)?;
    require_approve(body.approve, "task start")?;
    TaskCoordinator::new(state.workspace_root())
        .start(&task_id, body.agent_id)
        .map(Json)
        .map_err(map_ade_error)
}

pub(crate) async fn heartbeat_task(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
    Json(body): Json<HeartbeatTaskRequest>,
) -> ApiResult<AgentTask> {
    require_scope(&state, ApiScope::TasksWrite)?;
    require_approve(body.approve, "task heartbeat")?;
    TaskCoordinator::new(state.workspace_root())
        .heartbeat(&task_id, body.agent_id, ttl(body.ttl_secs)?)
        .map(Json)
        .map_err(map_ade_error)
}

pub(crate) async fn complete_task(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
    Json(body): Json<TaskAgentRequest>,
) -> ApiResult<AgentTask> {
    require_scope(&state, ApiScope::TasksWrite)?;
    require_approve(body.approve, "task complete")?;
    TaskCoordinator::new(state.workspace_root())
        .complete(&task_id, body.agent_id)
        .map(Json)
        .map_err(map_ade_error)
}

pub(crate) async fn fail_task(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
    Json(body): Json<FailTaskRequest>,
) -> ApiResult<AgentTask> {
    require_scope(&state, ApiScope::TasksWrite)?;
    require_approve(body.approve, "task fail")?;
    TaskCoordinator::new(state.workspace_root())
        .fail(&task_id, body.agent_id, body.failure)
        .map(Json)
        .map_err(map_ade_error)
}

pub(crate) async fn renew_lease(
    State(state): State<ApiState>,
    Path(lease_id): Path<String>,
    Json(body): Json<RenewLeaseRequest>,
) -> ApiResult<PathLease> {
    require_scope(&state, ApiScope::LeasesWrite)?;
    require_approve(body.approve, "lease renew")?;
    LeaseManager::new(state.workspace_root())
        .renew(body.agent_id, &lease_id, ttl(body.ttl_secs)?)
        .map(Json)
        .map_err(map_ade_error)
}
