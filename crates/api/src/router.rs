use crate::auth::ApiScope;
use crate::middleware::{audit_middleware, auth_middleware};
use crate::sse::SseManager;
use crate::verify_routes::run_verify;
use crate::write_routes::{
    claim_task, complete_task, fail_task, heartbeat_task, renew_lease, start_task,
};
use ade_core::audit::{AuditMode, AuditReport, AuditRunner};
use ade_core::error::AdeError;
use ade_core::plan::{PlanBuilder, PlanReport};
use ade_core::recipe::StackRecipe;
use ade_workflow::parallel::{LeaseManager, PathLease, WorktreeInfo, WorktreeManager};
use ade_workflow::tasks::{AgentTask, TaskCoordinator};
use axum::{
    extract::State,
    http::{header, Method, StatusCode},
    middleware,
    response::{
        sse::{Event, KeepAlive},
        IntoResponse, Response, Sse,
    },
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use serde_json::json;
use std::collections::HashSet;
use std::convert::Infallible;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpListener;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tower_http::cors::{AllowOrigin, CorsLayer};

#[derive(Clone)]
pub struct ApiState {
    workspace_root: Arc<PathBuf>,
    started_at: Instant,
    events: SseManager,
    auth_token: Option<Arc<str>>,
    scopes: Arc<HashSet<ApiScope>>,
}

impl ApiState {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: Arc::new(workspace_root.into()),
            started_at: Instant::now(),
            events: SseManager::new(),
            auth_token: None,
            scopes: Arc::new(HashSet::new()),
        }
    }

    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        let token = token.into();
        if !token.trim().is_empty() {
            self.auth_token = Some(Arc::from(token));
            if self.scopes.is_empty() {
                self.scopes = Arc::new(ApiScope::coordination_defaults());
            }
        }
        self
    }

    pub fn with_scopes(mut self, scopes: HashSet<ApiScope>) -> Self {
        self.scopes = Arc::new(scopes);
        self
    }

    pub fn auth_token(&self) -> Option<&str> {
        self.auth_token.as_deref()
    }

    pub fn has_scope(&self, scope: ApiScope) -> bool {
        self.scopes.contains(&scope)
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
    tasks: Vec<AgentTask>,
    worktrees: Vec<WorktreeInfo>,
    worktree_error: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ApiError {
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

    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: message.into(),
        }
    }

    pub(crate) fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: message.into(),
        }
    }

    pub(crate) fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: message.into(),
        }
    }
}

pub(crate) fn require_approve(approve: bool, operation: &str) -> Result<(), ApiError> {
    if approve {
        Ok(())
    } else {
        Err(ApiError::forbidden(format!(
            "{operation} mutates coordination state; set approve=true"
        )))
    }
}

pub(crate) fn require_scope(state: &ApiState, scope: ApiScope) -> Result<(), ApiError> {
    if state.auth_token().is_none() {
        return Err(ApiError::unauthorized(
            "coordination writes require ADE_API_TOKEN",
        ));
    }
    if state.has_scope(scope) {
        Ok(())
    } else {
        Err(ApiError::forbidden(format!(
            "bearer token lacks scope '{}'",
            scope.as_str()
        )))
    }
}

pub(crate) fn map_ade_error(error: AdeError) -> ApiError {
    match error {
        AdeError::Authorization(message) => ApiError::forbidden(message),
        AdeError::Auth(message) => ApiError::unauthorized(message),
        AdeError::NotFound(message) => ApiError {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message,
        },
        AdeError::Other(message) => ApiError::bad_request(message),
        other => ApiError::internal(other),
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

pub(crate) type ApiResult<T> = Result<Json<T>, ApiError>;

pub fn build_router() -> Router {
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    build_router_with_state(ApiState::new(root))
}

pub fn build_router_with_state(state: ApiState) -> Router {
    let api = Router::new()
        .route("/audit", get(audit_status))
        .route("/plan", get(plan_status))
        .route("/state", get(state_snapshot))
        .route("/recipes", get(list_recipes))
        .route("/rules", get(list_rules))
        .route("/skills", get(list_skills))
        .route("/leases", get(list_leases))
        .route("/tasks", get(list_tasks))
        .route("/tasks/claim", post(claim_task))
        .route("/tasks/:id/start", post(start_task))
        .route("/tasks/:id/heartbeat", post(heartbeat_task))
        .route("/tasks/:id/complete", post(complete_task))
        .route("/tasks/:id/fail", post(fail_task))
        .route("/worktrees", get(list_worktrees))
        .route("/handoff", get(handoff_status))
        .route("/events", get(events))
        .route("/verify", post(run_verify))
        .route("/leases/:id/renew", post(renew_lease))
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
        .layer(local_cors())
        .with_state(state)
}

/// CORS for browser previews of the desktop UI. Only local-origin pages may
/// call this API; the bearer-token gate still applies to every /api route.
fn local_cors() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin, _| {
            origin.to_str().is_ok_and(|value| {
                let localhost = value
                    .strip_prefix("http://localhost")
                    .or_else(|| value.strip_prefix("http://127.0.0.1"));
                localhost.is_some_and(|rest| rest.is_empty() || rest.starts_with(':'))
                    || value == "tauri://localhost"
            })
        }))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
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

async fn list_recipes() -> Json<Vec<StackRecipe>> {
    Json(ade_core::recipe::builtin_recipes())
}

async fn list_rules(
    State(state): State<ApiState>,
) -> ApiResult<Vec<ade_agents::authority::RuleFileInfo>> {
    ade_agents::authority::list_rule_files(state.workspace_root())
        .map(Json)
        .map_err(map_ade_error)
}

async fn list_skills(
    State(state): State<ApiState>,
) -> ApiResult<Vec<ade_agents::skills::SkillDefinition>> {
    ade_agents::skills::SkillLoader::new(state.workspace_root())
        .load_all()
        .map(Json)
        .map_err(map_ade_error)
}

async fn list_leases(State(state): State<ApiState>) -> ApiResult<Vec<PathLease>> {
    LeaseManager::new(state.workspace_root())
        .list()
        .map(Json)
        .map_err(ApiError::internal)
}

async fn list_tasks(State(state): State<ApiState>) -> ApiResult<Vec<AgentTask>> {
    TaskCoordinator::new(state.workspace_root())
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
    let tasks = TaskCoordinator::new(state.workspace_root())
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
        tasks,
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

    #[tokio::test]
    async fn task_claim_requires_token_scope_and_approve() {
        use ade_workflow::parallel::LeaseMode;
        use ade_workflow::tasks::EnqueueTask;

        let root = fixture();
        let agent = uuid::Uuid::new_v4();
        TaskCoordinator::new(&root)
            .enqueue(EnqueueTask {
                goal: "exercise write API".into(),
                owned_paths: vec!["src/api".into()],
                depends_on: Vec::new(),
                lease_mode: LeaseMode::Exclusive,
            })
            .unwrap();

        let unauth = build_router_with_state(ApiState::new(&root))
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/tasks/claim")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "agent_id": agent,
                            "approve": true
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauth.status(), StatusCode::UNAUTHORIZED);

        let read_only = build_router_with_state(
            ApiState::new(&root)
                .with_auth_token("test-token")
                .with_scopes(HashSet::from([ApiScope::Read])),
        )
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tasks/claim")
                .header(header::AUTHORIZATION, "Bearer test-token")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "agent_id": agent,
                        "approve": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(read_only.status(), StatusCode::FORBIDDEN);

        let app = build_router_with_state(ApiState::new(&root).with_auth_token("test-token"));
        let missing_approve = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/tasks/claim")
                    .header(header::AUTHORIZATION, "Bearer test-token")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "agent_id": agent,
                            "approve": false
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing_approve.status(), StatusCode::FORBIDDEN);

        let claimed = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/tasks/claim")
                    .header(header::AUTHORIZATION, "Bearer test-token")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "agent_id": agent,
                            "ttl_secs": 120,
                            "approve": true
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(claimed.status(), StatusCode::OK);
        let body = axum::body::to_bytes(claimed.into_body(), usize::MAX)
            .await
            .unwrap();
        let task: AgentTask = serde_json::from_slice(&body).unwrap();
        assert_eq!(task.agent_id, Some(agent));

        let heartbeat = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/tasks/{}/heartbeat", task.id))
                    .header(header::AUTHORIZATION, "Bearer test-token")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "agent_id": agent,
                            "ttl_secs": 180,
                            "approve": true
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(heartbeat.status(), StatusCode::OK);
        let _ = std::fs::remove_dir_all(root);
    }
}
