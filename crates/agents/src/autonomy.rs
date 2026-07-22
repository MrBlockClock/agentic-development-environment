use crate::authority::ToolEffect;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Product autonomy dial: Observe → Propose → Act → Automate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyLevel {
    /// Read-only tools; explain and point.
    Observe,
    /// Read-only tools; plan + diffs, no apply until approve.
    #[default]
    Propose,
    /// Execute within approved PLAN owned_paths / leases.
    Act,
    /// Act with verify-on-complete required and worker-style caps.
    Automate,
}

impl AutonomyLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::Propose => "propose",
            Self::Act => "act",
            Self::Automate => "automate",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Observe => "Observe",
            Self::Propose => "Propose",
            Self::Act => "Act",
            Self::Automate => "Automate",
        }
    }

    /// Whether workspace-write / process tools may be offered to the model.
    pub fn allows_mutating_tools(self) -> bool {
        matches!(self, Self::Act | Self::Automate)
    }

    /// Automate always gates "done" on verify.
    pub fn requires_verify_on_complete(self) -> bool {
        matches!(self, Self::Automate)
    }

    pub fn allows_tool_effect(self, effect: ToolEffect) -> bool {
        if self.allows_mutating_tools() {
            return true;
        }
        // Propose gets inspect-only shell (ProcessExecution enforced at runtime).
        match self {
            Self::Propose => matches!(effect, ToolEffect::ReadOnly | ToolEffect::ProcessExecution),
            _ => matches!(effect, ToolEffect::ReadOnly),
        }
    }

    pub fn prompt_clause(self) -> &'static str {
        match self {
            Self::Observe => {
                "AUTONOMY=Observe: read-only. Explain and point at evidence. Do not propose file patches or claim you applied changes."
            }
            Self::Propose => {
                "AUTONOMY=Propose (Suggest): read tools + shell__run_command in inspect mode (list/pwd/Get-Content). Use cwd for Desktop/home paths. Do not write/move/mkdir via shell — ask the user to switch to Apply for that. When offering choices, prefer a fenced ```ade.next-actions block with schema ade.next-actions/v1 and ≥2 items (label + optional prompt); otherwise a short numbered list. Prefer Queue PLAN→tasks on Desktop rather than self-claiming work."
            }
            Self::Act => {
                "AUTONOMY=Act (Apply): human approved writes for this turn. Prefer approved PLAN owned_paths / leases when present; if none, workspace writes are still allowed (AGENTS.md + sensitive-path policy still apply). shell__run_command is full (minus dangerous wipes); set cwd for Desktop/home. Execute ONE claimed task when role-split is active. Prefer verify after mutations."
            }
            Self::Automate => {
                "AUTONOMY=Automate: Apply one claimed task under leases/caps with full shell. Completion requires verify gates to pass; do not self-certify or claim a second task in the same turn."
            }
        }
    }
}

impl FromStr for AutonomyLevel {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "observe" => Ok(Self::Observe),
            "propose" => Ok(Self::Propose),
            "act" => Ok(Self::Act),
            "automate" => Ok(Self::Automate),
            other => Err(format!(
                "unknown autonomy level '{other}' (expected observe|propose|act|automate)"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observe_blocks_writes() {
        assert!(!AutonomyLevel::Observe.allows_mutating_tools());
        assert!(AutonomyLevel::Observe.allows_tool_effect(ToolEffect::ReadOnly));
        assert!(!AutonomyLevel::Observe.allows_tool_effect(ToolEffect::WorkspaceWrite));
        assert!(!AutonomyLevel::Observe.allows_tool_effect(ToolEffect::ProcessExecution));
    }

    #[test]
    fn propose_allows_inspect_shell_not_writes() {
        assert!(AutonomyLevel::Propose.allows_tool_effect(ToolEffect::ProcessExecution));
        assert!(!AutonomyLevel::Propose.allows_tool_effect(ToolEffect::WorkspaceWrite));
    }

    #[test]
    fn automate_requires_verify() {
        assert!(AutonomyLevel::Automate.requires_verify_on_complete());
        assert!(!AutonomyLevel::Act.requires_verify_on_complete());
    }
}
