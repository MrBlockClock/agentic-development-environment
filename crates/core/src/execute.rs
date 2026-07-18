use serde::{Deserialize, Serialize};

pub const EXECUTE_SCHEMA: &str = "ade.execute.report/v1";

#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteReport {
    pub schema: String,
    pub mode: String,
    pub approved_plan_ref: String,
    pub phases_completed: Vec<String>,
    pub changed_paths: Vec<String>,
    pub verify_evidence: Vec<VerifyEvidence>,
    pub score_before: Option<u32>,
    pub score_after: Option<u32>,
    pub score_max: u32,
    pub improvements: Vec<String>,
    pub remaining_gaps: Vec<String>,
    pub requires_human: Vec<String>,
    pub ready_for_focused_work: bool,
    pub human_summary_markdown: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyEvidence {
    pub gate: String,
    pub command: String,
    pub status: String,
    pub notes: Option<String>,
}
