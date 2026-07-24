use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const HANDOFF_SCHEMA: &str = "ade.handoff/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_max: Option<u32>,
    pub decisions_touched: Vec<String>,
    pub next_safe_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_status: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blockers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_verified_gate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_compaction: Option<HandoffContextCompaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffVerify {
    pub gate: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HandoffPromptSection {
    pub name: String,
    pub tokens: u32,
    pub truncated: bool,
}

/// PromptAssembler metrics persisted on capsules without prompt text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HandoffContextCompaction {
    pub tokens_estimated: u32,
    pub status: String,
    pub sections: Vec<HandoffPromptSection>,
}

impl HandoffCapsule {
    pub fn new(goal: impl Into<String>, mode: impl Into<String>) -> Self {
        Self {
            schema: HANDOFF_SCHEMA.into(),
            goal: goal.into(),
            mode: mode.into(),
            orchestrating_ade: "ade".into(),
            branch: None,
            changed_paths: vec![],
            verify_results: vec![],
            score_before: None,
            score_after: None,
            score_max: None,
            decisions_touched: vec![],
            next_safe_command: None,
            session_id: None,
            provider: None,
            model: None,
            created_at: Some(Utc::now().to_rfc3339()),
            turn_status: None,
            blockers: vec![],
            last_verified_gate: None,
            compact_summary: None,
            context_compaction: None,
        }
    }

    pub fn from_execute(goal: impl Into<String>, report: &crate::execute::ExecuteReport) -> Self {
        let mut capsule = Self {
            schema: HANDOFF_SCHEMA.into(),
            goal: goal.into(),
            mode: report.mode.clone(),
            orchestrating_ade: "ade".into(),
            branch: None,
            changed_paths: report.changed_paths.clone(),
            verify_results: report
                .verify_evidence
                .iter()
                .map(|evidence| HandoffVerify {
                    gate: evidence.gate.clone(),
                    status: evidence.status.clone(),
                })
                .collect(),
            score_before: report.score_before,
            score_after: report.score_after,
            score_max: Some(report.score_max),
            decisions_touched: vec![],
            next_safe_command: report
                .verify_evidence
                .iter()
                .find(|evidence| evidence.status != "pass")
                .map(|evidence| evidence.command.clone())
                .or_else(|| Some("ade verify --gate G0 --through".into())),
            session_id: None,
            provider: None,
            model: None,
            created_at: Some(Utc::now().to_rfc3339()),
            turn_status: Some("execute_complete".into()),
            blockers: report
                .verify_evidence
                .iter()
                .filter(|evidence| evidence.status != "pass")
                .map(|evidence| format!("{}:{}", evidence.gate, evidence.status))
                .collect(),
            last_verified_gate: report.verify_evidence.last().map(|item| item.gate.clone()),
            compact_summary: None,
            context_compaction: None,
        };
        capsule.compact_summary = Some(capsule.prompt_summary(480));
        capsule
    }

    pub fn from_agent_turn(
        goal: impl Into<String>,
        session_id: Uuid,
        provider: impl Into<String>,
        model: impl Into<String>,
        turn_status: impl Into<String>,
        blockers: Vec<String>,
    ) -> Self {
        let turn_status = turn_status.into();
        let next = match turn_status.as_str() {
            "completed" => "ade verify --gate G0 --through",
            "cancelled" => "ade audit",
            _ => "ade audit",
        };
        let mut capsule = Self {
            schema: HANDOFF_SCHEMA.into(),
            goal: goal.into(),
            mode: "agent_turn".into(),
            orchestrating_ade: "ade".into(),
            branch: None,
            changed_paths: vec![],
            verify_results: vec![],
            score_before: None,
            score_after: None,
            score_max: None,
            decisions_touched: vec![],
            next_safe_command: Some(next.into()),
            session_id: Some(session_id.to_string()),
            provider: Some(provider.into()),
            model: Some(model.into()),
            created_at: Some(Utc::now().to_rfc3339()),
            turn_status: Some(turn_status),
            blockers,
            last_verified_gate: None,
            compact_summary: None,
            context_compaction: None,
        };
        capsule.compact_summary = Some(capsule.prompt_summary(480));
        capsule
    }

    pub fn apply_verify_results(&mut self, results: &[crate::verify::VerifyResult]) {
        self.verify_results = results
            .iter()
            .map(|result| HandoffVerify {
                gate: result.gate.clone(),
                status: result.status_label().into(),
            })
            .collect();
        self.blockers = results
            .iter()
            .filter(|result| !result.passed)
            .map(|result| format!("{}:{}", result.gate, result.status_label()))
            .collect();
        self.last_verified_gate = results.last().map(|result| result.gate.clone());
        self.next_safe_command = results
            .iter()
            .find(|result| !result.passed)
            .map(|result| result.command.clone())
            .or_else(|| {
                results
                    .last()
                    .filter(|result| result.passed)
                    .map(|_| "ade verify --gate G0 --through".into())
            });
        self.turn_status = Some("verify_complete".into());
        self.created_at = Some(Utc::now().to_rfc3339());
        self.compact_summary = Some(self.prompt_summary(480));
    }

    /// User-facing prompt for Home “Continue last handoff” (N4).
    /// System prompt still injects the budgeted handoff summary separately.
    pub fn resume_user_prompt(&self) -> String {
        self.resume_user_prompt_with(false, None)
    }

    /// Continuity thrift variant when the Desktop/CLI host already ran `next_safe_command`.
    pub fn resume_user_prompt_with(&self, host_ran_next: bool, host_exit: Option<i32>) -> String {
        let next = self.next_safe_command.as_deref().unwrap_or("ade audit");
        let goal = truncate(&self.goal, 280);
        let status = self.turn_status.as_deref().unwrap_or("unknown");
        let blockers = if self.blockers.is_empty() {
            String::new()
        } else {
            format!(
                "\nKnown blockers: {}",
                self.blockers
                    .iter()
                    .take(4)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };
        let facts = self.thrift_continuity_facts();
        let thrift = "Do not paste prior chat or full transcripts.\n\
             Do not run long shell discovery loops, rebuilds, or broad edits.\n\
             Prefer one focused owned-path action or verify, then stop.";
        if host_ran_next {
            let exit = host_exit
                .map(|code| format!(" (exit {code})"))
                .unwrap_or_default();
            format!(
                "Continue from the latest ADE handoff (status: {status}).\n\
                 Prior goal: {goal}\n\
                 Host already ran next_safe_command: `{next}`{exit}.\n\
                 Do not re-run it unless that output clearly failed.{blockers}\n\
                 {facts}\
                 Stay inside approved owned_paths / leases.\n\
                 {thrift}"
            )
        } else {
            format!(
                "Continue from the latest ADE handoff (status: {status}).\n\
                 Prior goal: {goal}\n\
                 Do next_safe_command first: `{next}`{blockers}\n\
                 {facts}\
                 Stay inside approved owned_paths / leases. Prefer verify before expanding scope.\n\
                 {thrift}"
            )
        }
    }

    /// Compact durable facts for thrift resume (disk under `.ade/continuity/`, not chat paste).
    fn thrift_continuity_facts(&self) -> String {
        let paths = if self.changed_paths.is_empty() {
            "(see `.ade/continuity/last-write.json`)".into()
        } else {
            self.changed_paths
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        };
        let decisions = if self.decisions_touched.is_empty() {
            "(none recorded)".into()
        } else {
            self.decisions_touched
                .iter()
                .take(6)
                .cloned()
                .collect::<Vec<_>>()
                .join("; ")
        };
        let verify = if self.verify_results.is_empty() {
            "(unchanged)".into()
        } else {
            self.verify_results
                .iter()
                .take(6)
                .map(|item| format!("{}={}", item.gate, item.status))
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "Durable Continuity (`.ade/continuity/last-write.json`): paths={paths}; decisions={decisions}; verify={verify}.\n"
        )
    }

    /// Budgeted prompt injection — never dump the full capsule JSON.
    pub fn prompt_summary(&self, max_chars: usize) -> String {
        self.prompt_summary_with(max_chars, false)
    }

    pub fn prompt_summary_with(&self, max_chars: usize, host_ran_next: bool) -> String {
        let mut paths = self.changed_paths.clone();
        let truncated_paths = if paths.len() > 12 {
            paths.truncate(12);
            format!(
                "{} (+{} more)",
                paths.join(", "),
                self.changed_paths.len() - 12
            )
        } else {
            paths.join(", ")
        };
        let verify = self
            .verify_results
            .iter()
            .map(|item| format!("{}={}", item.gate, item.status))
            .collect::<Vec<_>>()
            .join(", ");
        let next = self
            .next_safe_command
            .clone()
            .unwrap_or_else(|| "ade audit".into());
        let blockers = if self.blockers.is_empty() {
            "-".into()
        } else {
            self.blockers
                .iter()
                .take(6)
                .cloned()
                .collect::<Vec<_>>()
                .join("; ")
        };
        let follow = if host_ran_next {
            "Host already ran next_safe_command — do not re-run unless it failed; prefer verify / one owned-path write."
        } else {
            "If the user is continuing this handoff, prefer next_safe_command before expanding scope. If they started a new topic, ignore this handoff and follow their message."
        };
        let mut summary = format!(
            "HANDOFF SUMMARY (ade.handoff/v1)\n\
             - next_safe_command: `{next}`\n\
             - goal: {goal}\n\
             - mode: {mode}\n\
             - status: {status}\n\
             - session: {session}\n\
             - provider/model: {provider}/{model}\n\
             - branch: {branch}\n\
             - score: {before:?} → {after:?} / {score_max:?}\n\
             - context: {context}\n\
             - verify: {verify}\n\
             - blockers: {blockers}\n\
             - changed_paths: {paths}\n\
             {follow}",
            goal = truncate(&self.goal, 240),
            mode = self.mode,
            status = self.turn_status.as_deref().unwrap_or("-"),
            session = self.session_id.as_deref().unwrap_or("-"),
            provider = self.provider.as_deref().unwrap_or("-"),
            model = self.model.as_deref().unwrap_or("-"),
            branch = self.branch.as_deref().unwrap_or("-"),
            before = self.score_before,
            after = self.score_after,
            score_max = self.score_max,
            context = self
                .context_compaction
                .as_ref()
                .map(|item| format!("{} ({} tokens)", item.status, item.tokens_estimated))
                .unwrap_or_else(|| "-".into()),
            verify = if verify.is_empty() {
                "-".into()
            } else {
                verify
            },
            blockers = blockers,
            paths = if truncated_paths.is_empty() {
                "-".into()
            } else {
                truncated_paths
            },
            next = next,
            follow = follow,
        );
        if summary.chars().count() > max_chars {
            summary = truncate(&summary, max_chars);
        }
        summary
    }
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let clipped: String = value.chars().take(max.saturating_sub(3)).collect();
    format!("{clipped}...")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify::{VerifyResult, VerifyStatus};

    #[test]
    fn verify_results_replace_capsule_evidence() {
        let mut capsule = HandoffCapsule::new("verify", "evaluate_existing");
        capsule.apply_verify_results(&[VerifyResult {
            gate: "G2".into(),
            command: "cargo fmt --check".into(),
            exit_code: Some(1),
            stdout: None,
            stderr: Some("formatting".into()),
            passed: false,
            status: VerifyStatus::Fail,
        }]);
        assert_eq!(capsule.verify_results[0].status, "fail");
        assert_eq!(
            capsule.next_safe_command.as_deref(),
            Some("cargo fmt --check")
        );
        assert!(!capsule.blockers.is_empty());
    }

    #[test]
    fn agent_turn_capsule_carries_session_metadata() {
        let capsule = HandoffCapsule::from_agent_turn(
            "fix the auth flow",
            Uuid::nil(),
            "openai",
            "gpt-4.1-mini",
            "completed",
            vec![],
        );
        assert_eq!(capsule.mode, "agent_turn");
        assert_eq!(capsule.provider.as_deref(), Some("openai"));
        assert!(capsule.compact_summary.is_some());
        assert!(capsule.prompt_summary(200).contains("next_safe_command"));
    }

    #[test]
    fn prompt_summary_respects_unicode_char_boundaries() {
        let mut capsule = HandoffCapsule::new("goal", "evaluate_existing");
        capsule.goal = "hello 🦀 world — unicode".repeat(40);
        let summary = capsule.prompt_summary(80);
        assert!(summary.chars().count() <= 80);
        assert!(summary.ends_with("..."));
    }

    #[test]
    fn resume_user_prompt_mentions_next_safe_command() {
        let mut capsule = HandoffCapsule::new("Ship N4 continuity", "agent_turn");
        capsule.next_safe_command = Some("ade verify --gate G0 --through".into());
        capsule.turn_status = Some("completed".into());
        let prompt = capsule.resume_user_prompt();
        assert!(prompt.contains("Continue from the latest ADE handoff"));
        assert!(prompt.contains("Do next_safe_command first"));
        assert!(prompt.contains("ade verify --gate G0 --through"));
        assert!(prompt.contains("Ship N4 continuity"));
        assert!(prompt.contains("discovery loops"));
    }

    #[test]
    fn resume_user_prompt_host_ran_is_thrifty() {
        let mut capsule = HandoffCapsule::new("Ship N4 continuity", "agent_turn");
        capsule.next_safe_command = Some("ade audit".into());
        capsule.turn_status = Some("budget_exhausted".into());
        capsule.changed_paths = vec![".ade/dogfood/continuity-acceptance.md".into()];
        capsule.decisions_touched = vec!["prefer Continuity thrift over paste".into()];
        let prompt = capsule.resume_user_prompt_with(true, Some(0));
        assert!(prompt.contains("Host already ran next_safe_command"));
        assert!(prompt.contains("exit 0"));
        assert!(!prompt.contains("Do next_safe_command first"));
        assert!(prompt.contains("discovery loops"));
        assert!(prompt.contains("Do not paste prior chat"));
        assert!(prompt.contains(".ade/continuity/last-write.json"));
        assert!(prompt.contains("continuity-acceptance.md"));
        let summary = capsule.prompt_summary_with(800, true);
        assert!(summary.contains("Host already ran next_safe_command"));
    }
}
