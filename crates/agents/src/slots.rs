//! Slot roles for multi-agent Orchestrator (H2).
//!
//! Planner (Suggest) ≠ Worker (Apply/Automate) ≠ Verifier.
//! Backend gates use this so UI convention alone cannot dual-write.

use crate::autonomy::AutonomyLevel;
use ade_core::error::AdeError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotRole {
    Planner,
    Worker,
    Verifier,
}

impl SlotRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planner => "planner",
            Self::Worker => "worker",
            Self::Verifier => "verifier",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, AdeError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "planner" | "suggest" | "propose" => Ok(Self::Planner),
            "worker" | "apply" | "act" | "automate" => Ok(Self::Worker),
            "verifier" | "verify" | "judge" => Ok(Self::Verifier),
            other => Err(AdeError::Config(format!(
                "unknown slot role '{other}' (expected planner|worker|verifier)"
            ))),
        }
    }

    pub fn from_autonomy(autonomy: AutonomyLevel) -> Self {
        match autonomy {
            AutonomyLevel::Observe | AutonomyLevel::Propose => Self::Planner,
            AutonomyLevel::Act | AutonomyLevel::Automate => Self::Worker,
        }
    }

    pub fn may_acquire_write_leases(self) -> bool {
        matches!(self, Self::Worker)
    }

    pub fn may_claim_tasks(self) -> bool {
        matches!(self, Self::Worker)
    }

    /// Verifier (and Automate Worker) may run sensors / verify ladder.
    pub fn may_run_verify_sensors(self) -> bool {
        matches!(self, Self::Verifier | Self::Worker)
    }

    pub fn require_write_lease(self) -> Result<(), AdeError> {
        if self.may_acquire_write_leases() {
            Ok(())
        } else {
            Err(AdeError::Authorization(format!(
                "slot_gate: {} cannot acquire write leases — switch to Apply (Worker)",
                self.as_str()
            )))
        }
    }

    pub fn require_claim_tasks(self) -> Result<(), AdeError> {
        if self.may_claim_tasks() {
            Ok(())
        } else {
            Err(AdeError::Authorization(format!(
                "slot_gate: {} cannot claim tasks — switch to Apply (Worker)",
                self.as_str()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggest_is_planner_apply_is_worker() {
        assert_eq!(
            SlotRole::from_autonomy(AutonomyLevel::Propose),
            SlotRole::Planner
        );
        assert_eq!(
            SlotRole::from_autonomy(AutonomyLevel::Act),
            SlotRole::Worker
        );
        assert!(SlotRole::Planner.require_claim_tasks().is_err());
        assert!(SlotRole::Worker.require_write_lease().is_ok());
        assert!(SlotRole::Verifier.require_write_lease().is_err());
        assert!(SlotRole::Verifier.require_claim_tasks().is_err());
        assert!(SlotRole::Verifier.may_run_verify_sensors());
    }
}
