use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IgnoreSurface {
    Git,
    AiIndex,
    Docker,
    AgentPolicy,
    BackupSync,
    CiPublish,
}

impl IgnoreSurface {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Git => ".gitignore",
            Self::AiIndex => ".cursorignore",
            Self::Docker => ".dockerignore",
            Self::AgentPolicy => "AGENTS.md policy",
            Self::BackupSync => "Backup/Sync exclusions",
            Self::CiPublish => "CI/Publish filters",
        }
    }

    pub fn all() -> Vec<IgnoreSurface> {
        vec![
            Self::Git,
            Self::AiIndex,
            Self::Docker,
            Self::AgentPolicy,
            Self::BackupSync,
            Self::CiPublish,
        ]
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IgnoreAlignment {
    pub surface: String,
    pub status: IgnoreStatus,
    pub missing_patterns: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IgnoreStatus {
    Synced,
    Drifted,
    Missing,
    NotApplicable,
}
