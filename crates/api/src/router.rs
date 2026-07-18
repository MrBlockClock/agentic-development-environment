use crate::middleware::{audit_middleware, auth_middleware};
use crate::sse::SseManager;
use ade_core::audit::{AuditMode, AuditReport, AuditRunner};
use ade_core::plan::{PlanBuilder, PlanReport};
use ade_workflow::parallel::{LeaseManager, PathLease, WorktreeInfo, WorktreeManager};
use axum::{
    extract::State,
    http::StatusCode,
    middleware,
    response::{
        sse::{Event, KeepAlive},
        IntoResponse, Response, Sse,
    },
    routing::get,
    Json, Router,
};
use serde::Serialize;
use serde_json::json;
use std::convert::Infallible;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpListener;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

#[derive(Clone)]
pub struct ApiState {
    workspace_root: Arc<PathBuf>,
    started_at: Instant,
    events: SseManager,
    auth_token: Option<Arc<str>>,
}

impl ApiState {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: Arc::new(workspace_root.into()),
            started_at: Instant::now(),
            events: SseManager::new(),
            auth_token: None,
        }
    }

    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        let token = token.into();
        if !token.trim().is_empty() {
            self.auth_token = Some(Arc::from(token));
        }
        self
    }

    pub fn auth_token(&self) -> Option<&str> {
        self.auth_token.as_deref()
    }

    pub fn workspace_root(&self) -> &PathBuf {
        &self.workspace_root
    }
}

#[derive(Debug, Serialize)]
struct LiveHealth {
    status: &'static str,
    version: &'static str,
    uptime_seconds: u64,
}

#[derive(Debug, Serialize)]
struct ReadyHealth {
    status: &'static str,
    workspace_root: String,
    contract_present: bool,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ApiSnapshot {
    schema: &'static str,
    workspace_root: String,
    audit: AuditReport,
    plan: PlanReport,
    handoff: ade_agents::handoff::HandoffMetrics,
    leases: Vec<PathLease>,
    worktrees: Vec<WorktreeInfo>,
    worktree_error: Option<String>,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn internal(error: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message: error.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({
                "error": {
                    "code": self.code,
                    "message": self.message
                }
            })),
        )
            .into_response()
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

pub fn build_router() -> Router {
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    build_router_with_state(ApiState::new(root))
}

pub fn build_router_with_state(state: ApiState) -> Router {
    let api = Router::new()
        .route("/audit", get(audit_status))
        .route("/plan", get(plan_status))
        .route("/state", get(state_snapshot))
        .route("/leases", get(list_leases))
        .route("/worktrees", get(list_worktrees))
        .route("/handoff", get(handoff_status))
        .route("/events", get(events))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .route_layer(middleware::from_fn(audit_middleware));

    Router::new()
        .route("/health", get(live_health))
        .route("/health/live", get(live_health))
        .route("/health/ready", get(ready_health))
        .nest("/api", api)
        .with_state(state)
}

pub async fn serve<F>(
    listener: TcpListener,
    state: ApiState,
    shutdown: F,
) -> Result<(), std::io::Error>
where
    F: Future<Output = ()> + Send + 'static,
{
    axum::serve(listener, build_router_with_state(state))
        .with_graceful_shutdown(shutdown)
        .await
}

async fn live_health(State(state): State<ApiState>) -> Json<LiveHealth> {
    Json(LiveHealth {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: state.started_at.elapsed().as_secs(),
    })
}

async fn ready_health(State(state): State<ApiState>) -> Response {
    let contract_present = state.workspace_root().join("AGENTS.md").is_file();
    let audit = AuditRunner::new(state.workspace_root()).run(AuditMode::EvaluateExisting);
    let ready = contract_present && audit.blockers.is_empty();
    let report = ReadyHealth {
        status: if ready { "ready" } else { "not_ready" },
        workspace_root: state.workspace_root().display().to_string(),
        contract_present,
        blockers: audit.blockers,
    };
    (
        if ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        Json(report),
    )
        .into_response()
}

async fn audit_status(State(state): State<ApiState>) -> ApiResult<AuditReport> {
    let report = AuditRunner::new(state.workspace_root()).run(AuditMode::EvaluateExisting);
    state.events.send_event(
        Event::default()
            .event("audit.read")
            .data(format!("{}/{}", report.score, report.score_max)),
    );
    Ok(Json(report))
}

async fn plan_status(State(state): State<ApiState>) -> ApiResult<PlanReport> {
    let audit = AuditRunner::new(state.workspace_root()).run(AuditMode::EvaluateExisting);
    let plan = PlanBuilder::new().build(&audit);
    state.events.send_event(
        Event::default()
            .event("plan.read")
            .data(plan.phases.len().to_string()),
    );
    Ok(Json(plan))
}

async fn list_leases(State(state): State<ApiState>) -> ApiResult<Vec<PathLease>> {
    LeaseManager::new(state.workspace_root())
        .list()
        .map(Json)
        .map_err(ApiError::internal)
}

async fn list_worktrees(State(state): State<ApiState>) -> ApiResult<Vec<WorktreeInfo>> {
    WorktreeManager::new(state.workspace_root())
        .list()
        .map(Json)
        .map_err(ApiError::internal)
}

async fn handoff_status(
    State(state): State<ApiState>,
) -> ApiResult<ade_agents::handoff::HandoffMetrics> {
    ade_agents::handoff::HandoffManager::new(state.workspace_root())
        .metrics()
        .map(Json)
        .map_err(ApiError::internal)
}

async fn state_snapshot(State(state): State<ApiState>) -> ApiResult<ApiSnapshot> {
    let audit = AuditRunner::new(state.workspace_root()).run(AuditMode::EvaluateExisting);
    let plan = PlanBuilder::new().build(&audit);
    let handoff = ade_agents::handoff::HandoffManager::new(state.workspace_root())
        .metrics()
        .map_err(ApiError::internal)?;
    let leases = LeaseManager::new(state.workspace_root())
        .list()
        .map_err(ApiError::internal)?;
    let (worktrees, worktree_error) = match WorktreeManager::new(state.workspace_root()).list() {
        Ok(worktrees) => (worktrees, None),
        Err(error) => (Vec::new(), Some(error.to_string())),
    };
    Ok(Json(ApiSnapshot {
        schema: "ade.api.snapshot/v1",
        workspace_root: state.workspace_root().display().to_string(),
        audit,
        plan,
        handoff,
        leases,
        worktrees,
        worktree_error,
    }))
}

async fn events(
    State(state): State<ApiState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let stream = BroadcastStream::new(state.events.subscribe())
        .filter_map(|event| event.ok())
        .map(Ok::<Event, Infallible>);
    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{header, Request};
    use tower::ServiceExt;

    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!("ade-api-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("AGENTS.md"), "# API test contract\n").unwrap();
        root
    }

    #[tokio::test]
    async fn health_is_public_and_api_honors_bearer_token() {
        let root = fixture();
        let app = build_router_with_state(ApiState::new(&root).with_auth_token("test-token"));

        let health = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health/live")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health.status(), StatusCode::OK);

        let denied = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/audit")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);

        let allowed = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/audit")
                    .header(header::AUTHORIZATION, "Bearer test-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(allowed.status(), StatusCode::OK);
        assert!(allowed.headers().contains_key("x-ade-request-id"));

        let snapshot = app
            .oneshot(
                Request::builder()
                    .uri("/api/state")
                    .header(header::AUTHORIZATION, "Bearer test-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(snapshot.status(), StatusCode::OK);
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn readiness_fails_without_contract() {
        let root = std::env::temp_dir().join(format!("ade-api-empty-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let response = build_router_with_state(ApiState::new(&root))
            .oneshot(
                Request::builder()
                    .uri("/health/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let _ = std::fs::remove_dir_all(root);
    }
}
