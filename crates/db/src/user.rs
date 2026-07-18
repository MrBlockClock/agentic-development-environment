use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub sso_subject: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
