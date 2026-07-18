use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalyticsEvent {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub event_type: AnalyticsEventType,
    pub session_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub workspace_id: Option<Uuid>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
    pub cost_estimate: Option<f64>,
    pub latency_ms: Option<u64>,
    pub accept_rate: Option<f32>,
    pub gate: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AnalyticsEventType {
    SessionStarted,
    SessionEnded,
    ModelRequest,
    ModelResponse,
    VerifyRun,
    PhaseTransition,
    CostAlert,
    AnomalyDetected,
    Error,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelQualityMetrics {
    pub model_name: String,
    pub provider: String,
    pub total_sessions: u64,
    pub total_cost: f64,
    pub avg_latency_ms: f64,
    pub accept_rate: f32,
    pub reversion_rate: f32,
    pub error_rate: f32,
    pub g5_pass_rate: f32,
    pub lines_per_dollar: f64,
    pub best_task_types: Vec<String>,
    pub worst_task_types: Vec<String>,
}
