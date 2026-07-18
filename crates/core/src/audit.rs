use serde::{Deserialize, Serialize};

pub const AUDIT_SCHEMA: &str = "ade.audit.report/v1";

/// How the AUDIT phase was invoked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditMode {
    /// Score an existing project/environment.
    #[default]
    EvaluateExisting,
    /// Assess a greenfield/bootstrap setup.
    Bootstrap,
}

impl AuditMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EvaluateExisting => "evaluate_existing",
            Self::Bootstrap => "bootstrap",
        }
    }
}

/// Read-only discovery + scoring result produced by the AUDIT phase.
#[derive(Debug, Serialize, Deserialize)]
pub struct AuditReport {
    pub schema: String,
    pub mode: String,
    pub score: u32,
    pub score_max: u32,
    pub findings: Vec<AuditFinding>,
    pub human_summary_markdown: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuditFinding {
    pub layer: String,
    pub severity: String,
    pub detail: String,
}
