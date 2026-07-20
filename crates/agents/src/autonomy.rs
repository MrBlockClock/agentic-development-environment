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
        matches!(effect, ToolEffect::ReadOnly)
    }

    pub fn prompt_clause(self) -> &'static str {
        match self {
            Self::Observe => {
                "AUTONOMY=Observe: read-only. Explain and point at evidence. Do not propose file patches or claim you applied changes."
            }
            Self::Propose => {
                "AUTONOMY=Propose: you may draft plans and unified diffs in chat. Do not apply writes; wait for human approval / Act mode."
            }
            Self::Act => {
                "AUTONOMY=Act: execute only inside approved PLAN owned_paths / active leases. Prefer verify after mutations."
            }
            Self::Automate => {
                "AUTONOMY=Automate: execute under leases/caps. Completion requires verify gates to pass; do not self-certify."
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
    }

    #[test]
    fn automate_requires_verify() {
        assert!(AutonomyLevel::Automate.requires_verify_on_complete());
        assert!(!AutonomyLevel::Act.requires_verify_on_complete());
    }
}
