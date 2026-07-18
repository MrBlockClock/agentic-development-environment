use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Team {
    pub id: Uuid,
    pub name: String,
    pub org_id: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TeamMember {
    pub team_id: Uuid,
    pub user_id: Uuid,
    pub role: TeamRole,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TeamRole {
    Owner,
    Admin,
    Member,
    Viewer,
}
