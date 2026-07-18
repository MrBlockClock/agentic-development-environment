use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub category: AuditCategory,
    pub action: String,
    pub actor_id: Option<Uuid>,
    pub workspace_id: Option<Uuid>,
    pub details: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AuditCategory {
    Authentication,
    AgentAction,
    PolicyChange,
    Secrets,
    Billing,
    Compliance,
}
