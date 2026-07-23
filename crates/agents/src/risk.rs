//! Risk-tiered HITL (G2).
//!
//! Secrets / infra / migrate / publish force a human confirm even under
//! Apply/Automate — kills review habituation on high-blast paths.

use crate::authority::ToolEffect;
use ade_core::error::AdeError;
use ade_core::ignore::SensitivePathPolicy;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

pub const RISK_GATE_PREFIX: &str = "risk_gate:";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskTier {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    pub fn requires_hitl(self) -> bool {
        matches!(self, Self::High | Self::Critical)
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "low" => Some(Self::Low),
            "medium" | "med" => Some(Self::Medium),
            "high" => Some(Self::High),
            "critical" | "crit" => Some(Self::Critical),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskCategory {
    Normal,
    Secrets,
    Infra,
    Migrate,
    Publish,
    External,
}

impl RiskCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Secrets => "secrets",
            Self::Infra => "infra",
            Self::Migrate => "migrate",
            Self::Publish => "publish",
            Self::External => "external",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "normal" => Some(Self::Normal),
            "secrets" | "secret" => Some(Self::Secrets),
            "infra" | "destructive" => Some(Self::Infra),
            "migrate" | "migration" => Some(Self::Migrate),
            "publish" | "push" | "deploy" => Some(Self::Publish),
            "external" | "network" => Some(Self::External),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskAssessment {
    pub tier: RiskTier,
    pub category: RiskCategory,
    pub reason: String,
}

impl RiskAssessment {
    pub fn requires_hitl(&self) -> bool {
        self.tier.requires_hitl()
    }
}

/// Classify a tool call for risk-tiered HITL.
pub fn assess_tool(
    server: &str,
    tool: &str,
    arguments: &Value,
    effect: ToolEffect,
) -> RiskAssessment {
    let server_l = server.to_ascii_lowercase();
    let tool_l = tool.to_ascii_lowercase();
    let cmd = arguments
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("");
    let cmd_l = cmd.to_ascii_lowercase();
    let path = arguments.get("path").and_then(Value::as_str).unwrap_or("");

    if !path.is_empty() && SensitivePathPolicy::is_secret_path(path) {
        return RiskAssessment {
            tier: RiskTier::Critical,
            category: RiskCategory::Secrets,
            reason: format!("secret-bearing path '{path}'"),
        };
    }

    if matches!(effect, ToolEffect::ExternalWrite)
        || (server_l == "git" && matches!(tool_l.as_str(), "push" | "publish"))
        || (server_l == "http" && matches!(tool_l.as_str(), "upload" | "put" | "post"))
        || cmd_l.contains("git push")
        || cmd_l.contains("gh release")
        || cmd_l.contains("npm publish")
        || cmd_l.contains("cargo publish")
    {
        return RiskAssessment {
            tier: RiskTier::High,
            category: RiskCategory::Publish,
            reason: "publish / external write".into(),
        };
    }

    if migrate_command(&cmd_l) || (server_l == "db" && tool_l.contains("migrate")) {
        return RiskAssessment {
            tier: RiskTier::High,
            category: RiskCategory::Migrate,
            reason: "schema migrate / destructive data command".into(),
        };
    }

    if let Some(reason) = crate::shell::dangerous_command_reason(cmd) {
        return RiskAssessment {
            tier: RiskTier::Critical,
            category: RiskCategory::Infra,
            reason: format!("destructive shell: {reason}"),
        };
    }

    if infra_command(&cmd_l) {
        return RiskAssessment {
            tier: RiskTier::High,
            category: RiskCategory::Infra,
            reason: "infra / destructive shell pattern".into(),
        };
    }

    if matches!(effect, ToolEffect::ProcessExecution) && !cmd.is_empty() {
        if cmd_l.contains("force") && (cmd_l.contains("push") || cmd_l.contains("reset")) {
            return RiskAssessment {
                tier: RiskTier::High,
                category: RiskCategory::Infra,
                reason: "force push/reset".into(),
            };
        }
        return RiskAssessment {
            tier: RiskTier::Medium,
            category: RiskCategory::Normal,
            reason: "process execution".into(),
        };
    }

    if matches!(effect, ToolEffect::WorkspaceWrite) {
        return RiskAssessment {
            tier: RiskTier::Low,
            category: RiskCategory::Normal,
            reason: "workspace write".into(),
        };
    }

    RiskAssessment {
        tier: RiskTier::Low,
        category: RiskCategory::Normal,
        reason: "read / low impact".into(),
    }
}

fn migrate_command(cmd: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "prisma migrate",
        "diesel migration",
        "alembic upgrade",
        "alembic downgrade",
        "flyway",
        "liquibase",
        " knex migrate",
        "rake db:migrate",
        "drop table",
        "drop database",
        "truncate table",
        "alter table",
    ];
    NEEDLES.iter().any(|n| cmd.contains(n))
}

fn infra_command(cmd: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "terraform destroy",
        "terraform apply",
        "pulumi destroy",
        "pulumi up",
        "kubectl delete",
        "helm uninstall",
        "docker system prune",
        "rm -rf",
        "rm -fr",
        "remove-item -recurse",
        "format-volume",
        "diskpart",
    ];
    NEEDLES.iter().any(|n| cmd.contains(n))
}

/// True when the turn's approved categories/tiers cover this assessment.
pub fn risk_is_approved(
    assessment: &RiskAssessment,
    approved_categories: &[String],
    approved_tiers: &[String],
) -> bool {
    if !assessment.requires_hitl() {
        return true;
    }
    let cat = assessment.category.as_str();
    let tier = assessment.tier.as_str();
    let cats_ok = approved_categories.iter().any(|c| {
        c.eq_ignore_ascii_case(cat) || c.eq_ignore_ascii_case("all") || c.eq_ignore_ascii_case("*")
    });
    let tiers_ok = approved_tiers.iter().any(|t| {
        if t.eq_ignore_ascii_case("all") || t.eq_ignore_ascii_case("*") {
            return true;
        }
        match (RiskTier::parse(t), Some(assessment.tier)) {
            (Some(approved), Some(needed)) => approved >= needed,
            _ => t.eq_ignore_ascii_case(tier),
        }
    });
    cats_ok || tiers_ok
}

pub fn risk_deny_message(assessment: &RiskAssessment) -> String {
    format!(
        "{RISK_GATE_PREFIX} {} ({}) — confirm high-risk category '{}' (or switch to Suggest). Automate/Apply is not a blank check for secrets/infra/migrate/publish.",
        assessment.reason,
        assessment.tier.as_str(),
        assessment.category.as_str()
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskWaiveRecord {
    pub at: String,
    pub category: String,
    pub tier: String,
    pub reason: String,
    pub autonomy: String,
    pub server: String,
    pub tool: String,
}

pub fn log_risk_waive(
    workspace_root: impl AsRef<Path>,
    record: &RiskWaiveRecord,
) -> Result<(), AdeError> {
    let dir = workspace_root.as_ref().join(".ade").join("risk");
    std::fs::create_dir_all(&dir)
        .map_err(|e| AdeError::Config(format!("cannot create risk dir: {e}")))?;
    let path = dir.join("waives.jsonl");
    let line = serde_json::to_string(record)
        .map_err(|e| AdeError::Config(format!("serialize risk waive: {e}")))?;
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| AdeError::Config(format!("open {}: {e}", path.display())))?;
    writeln!(file, "{line}").map_err(|e| AdeError::Config(format!("write risk waive: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn git_push_is_high_publish() {
        let a = assess_tool(
            "shell",
            "run_command",
            &json!({ "command": "git push origin main" }),
            ToolEffect::ProcessExecution,
        );
        assert_eq!(a.tier, RiskTier::High);
        assert_eq!(a.category, RiskCategory::Publish);
        assert!(a.requires_hitl());
    }

    #[test]
    fn migrate_is_high() {
        let a = assess_tool(
            "shell",
            "run_command",
            &json!({ "command": "prisma migrate deploy" }),
            ToolEffect::ProcessExecution,
        );
        assert_eq!(a.category, RiskCategory::Migrate);
        assert!(a.requires_hitl());
    }

    #[test]
    fn normal_write_is_low() {
        let a = assess_tool(
            "fs",
            "write_file",
            &json!({ "path": "src/lib.rs", "content": "x" }),
            ToolEffect::WorkspaceWrite,
        );
        assert_eq!(a.tier, RiskTier::Low);
        assert!(!a.requires_hitl());
    }

    #[test]
    fn approval_covers_category() {
        let a = assess_tool(
            "shell",
            "run_command",
            &json!({ "command": "git push" }),
            ToolEffect::ProcessExecution,
        );
        assert!(!risk_is_approved(&a, &[], &[]));
        assert!(risk_is_approved(&a, &["publish".into()], &[]));
        assert!(risk_is_approved(&a, &[], &["high".into()]));
    }
}
