use serde::{Deserialize, Serialize};

pub const HANDOFF_SCHEMA: &str = "ade.handoff/v1";

#[derive(Debug, Serialize, Deserialize)]
pub struct HandoffCapsule {
    pub schema: String,
    pub goal: String,
    pub mode: String,
    pub orchestrating_ade: String,
    pub branch: Option<String>,
    pub changed_paths: Vec<String>,
    pub verify_results: Vec<HandoffVerify>,
    pub score_before: Option<u32>,
    pub score_after: Option<u32>,
    pub decisions_touched: Vec<String>,
    pub next_safe_command: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HandoffVerify {
    pub gate: String,
    pub status: String,
}
