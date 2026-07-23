//! Eng-goal objects — durable outcome + scope + verify, not free chat.
//!
//! Layout (workspace):
//! - `.ade/goals/{id}.json` — immutable-ish goal records
//! - `.ade/goals/active.json` — `{ "id": "…" }` pointer to the current goal

use ade_core::error::AdeError;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const GOAL_SCHEMA: &str = "ade.goal/v1";

/// Stable prefix for `AdeError::Authorization` when Act tools are blocked (Desktop matches).
pub const CONTRACT_GATE_PREFIX: &str = "contract_gate:";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContractWaive {
    pub at: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EngGoal {
    pub schema: String,
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    /// User-stated outcome (the product unit of work).
    pub statement: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub success_criteria: Vec<String>,
    /// Explicit non-goals / out of scope (master-gameplan G1 contract field).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub out_of_scope: Vec<String>,
    /// `workspace` | `home` — maps to shell preferred cwd (orch G1).
    pub shell_scope: String,
    /// `propose` | `act` | `automate` (observe rare for goals).
    pub autonomy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify_gate: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owned_paths: Vec<String>,
    /// `active` | `paused` | `done` | `abandoned`
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_handoff_id: Option<String>,
    /// Logged human waive of the Apply contract gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_waive: Option<ContractWaive>,
    /// ≤3 clarify answers that unlock Apply when a full contract is not yet written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clarify_resolutions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveGoalPointer {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GoalCreateInput {
    pub statement: String,
    #[serde(default)]
    pub success_criteria: Vec<String>,
    #[serde(default)]
    pub out_of_scope: Vec<String>,
    #[serde(default = "default_shell_scope")]
    pub shell_scope: String,
    #[serde(default = "default_autonomy")]
    pub autonomy: String,
    pub verify_gate: Option<String>,
    #[serde(default)]
    pub owned_paths: Vec<String>,
    /// When true, also write `active.json`.
    #[serde(default = "default_true")]
    pub activate: bool,
}

fn default_shell_scope() -> String {
    "workspace".into()
}

fn default_autonomy() -> String {
    "propose".into()
}

fn default_true() -> bool {
    true
}

impl EngGoal {
    /// Full Apply contract: AC + out-of-scope + verify pointer on an active goal.
    pub fn is_contract_ready(&self) -> bool {
        self.status == "active"
            && self.success_criteria.iter().any(|c| !c.trim().is_empty())
            && self.out_of_scope.iter().any(|c| !c.trim().is_empty())
            && self
                .verify_gate
                .as_ref()
                .is_some_and(|g| !g.trim().is_empty())
    }

    /// Clarify escape: 1..=3 recorded resolutions on an active goal.
    pub fn clarify_unlocks_act(&self) -> bool {
        self.status == "active"
            && !self.clarify_resolutions.is_empty()
            && self.clarify_resolutions.len() <= 3
            && self
                .clarify_resolutions
                .iter()
                .all(|c| !c.trim().is_empty())
    }

    /// Act/Automate tools may run when contract ready, waived, or clarify-resolved.
    pub fn allows_act_tools(&self) -> bool {
        self.is_contract_ready() || self.contract_waive.is_some() || self.clarify_unlocks_act()
    }

    pub fn contract_block_detail(&self) -> String {
        let mut missing = Vec::new();
        if !self.success_criteria.iter().any(|c| !c.trim().is_empty()) {
            missing.push("acceptance criteria");
        }
        if !self.out_of_scope.iter().any(|c| !c.trim().is_empty()) {
            missing.push("out-of-scope");
        }
        if !self
            .verify_gate
            .as_ref()
            .is_some_and(|g| !g.trim().is_empty())
        {
            missing.push("verify pointer");
        }
        if missing.is_empty() {
            format!(
                "{CONTRACT_GATE_PREFIX} eng-goal '{}' is not active (status={})",
                self.id, self.status
            )
        } else {
            format!(
                "{CONTRACT_GATE_PREFIX} Act tools blocked until eng-goal has {} (or ≤3 clarify / logged waive). Define a goal or switch to Suggest.",
                missing.join(", ")
            )
        }
    }

    pub fn prompt_block(&self) -> String {
        let mut lines = vec![
            format!("ENG GOAL (id={}):", self.id),
            format!("Statement: {}", self.statement.trim()),
            format!(
                "Shell scope: {} · Autonomy: {} · Status: {}",
                self.shell_scope, self.autonomy, self.status
            ),
        ];
        if let Some(gate) = self.verify_gate.as_deref().filter(|g| !g.trim().is_empty()) {
            lines.push(format!("Verify gate: {gate}"));
        }
        if !self.success_criteria.is_empty() {
            lines.push("Success criteria:".into());
            for (i, c) in self.success_criteria.iter().enumerate() {
                lines.push(format!("  {}. {}", i + 1, c.trim()));
            }
        }
        if !self.out_of_scope.is_empty() {
            lines.push("Out of scope:".into());
            for (i, c) in self.out_of_scope.iter().enumerate() {
                lines.push(format!("  {}. {}", i + 1, c.trim()));
            }
        }
        if !self.clarify_resolutions.is_empty() {
            lines.push("Clarify resolutions:".into());
            for (i, c) in self.clarify_resolutions.iter().enumerate() {
                lines.push(format!("  {}. {}", i + 1, c.trim()));
            }
        }
        if let Some(waive) = &self.contract_waive {
            lines.push(format!(
                "Contract waive: {} ({})",
                waive.reason.trim(),
                waive.at
            ));
        }
        if !self.owned_paths.is_empty() {
            lines.push(format!("Owned paths hint: {}", self.owned_paths.join(", ")));
        }
        if self.allows_act_tools() {
            lines.push("Apply contract: unlocked for Act/Automate tools.".into());
        } else {
            lines.push(
                "Apply contract: incomplete — Suggest/inspect only until AC + out-of-scope + verify (or waive/clarify)."
                    .into(),
            );
        }
        lines.push(
            "Treat this goal as the outcome to advance this turn; chat is the channel, not the product."
                .into(),
        );
        lines.join("\n")
    }

    /// Prompt used when the user hits “Run goal” (statement + criteria).
    pub fn run_prompt(&self) -> String {
        let mut parts = vec![self.statement.trim().to_string()];
        if !self.success_criteria.is_empty() {
            parts.push(String::new());
            parts.push("Success criteria:".into());
            for c in &self.success_criteria {
                parts.push(format!("- {}", c.trim()));
            }
        }
        if !self.out_of_scope.is_empty() {
            parts.push(String::new());
            parts.push("Out of scope:".into());
            for c in &self.out_of_scope {
                parts.push(format!("- {}", c.trim()));
            }
        }
        parts.join("\n")
    }
}

pub struct GoalStore {
    root: PathBuf,
}

impl GoalStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn goals_dir(&self) -> PathBuf {
        self.root.join(".ade").join("goals")
    }

    fn goal_path(&self, id: &str) -> PathBuf {
        self.goals_dir().join(format!("{id}.json"))
    }

    fn active_path(&self) -> PathBuf {
        self.goals_dir().join("active.json")
    }

    pub fn create(&self, input: GoalCreateInput) -> Result<EngGoal, AdeError> {
        let statement = input.statement.trim();
        if statement.is_empty() {
            return Err(AdeError::Config("goal statement cannot be empty".into()));
        }
        let shell_scope = normalize_shell_scope(&input.shell_scope)?;
        let autonomy = normalize_autonomy(&input.autonomy)?;
        let now = Utc::now().to_rfc3339();
        let goal = EngGoal {
            schema: GOAL_SCHEMA.into(),
            id: Uuid::new_v4().to_string(),
            created_at: now.clone(),
            updated_at: now,
            statement: statement.chars().take(2_000).collect(),
            success_criteria: input
                .success_criteria
                .into_iter()
                .map(|c| c.trim().to_string())
                .filter(|c| !c.is_empty())
                .take(12)
                .collect(),
            out_of_scope: input
                .out_of_scope
                .into_iter()
                .map(|c| c.trim().to_string())
                .filter(|c| !c.is_empty())
                .take(12)
                .collect(),
            shell_scope,
            autonomy,
            verify_gate: input
                .verify_gate
                .map(|g| g.trim().to_string())
                .filter(|g| !g.is_empty()),
            owned_paths: input
                .owned_paths
                .into_iter()
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .take(32)
                .collect(),
            status: "active".into(),
            last_handoff_id: None,
            contract_waive: None,
            clarify_resolutions: Vec::new(),
        };
        self.save(&goal)?;
        if input.activate {
            self.set_active(&goal.id)?;
        }
        Ok(goal)
    }

    pub fn save(&self, goal: &EngGoal) -> Result<(), AdeError> {
        validate_goal(goal)?;
        let dir = self.goals_dir();
        std::fs::create_dir_all(&dir)?;
        let payload = serde_json::to_vec_pretty(goal)?;
        write_atomic(&self.goal_path(&goal.id), &payload)?;
        Ok(())
    }

    pub fn load(&self, id: &str) -> Result<EngGoal, AdeError> {
        let id = validate_id(id)?;
        let path = self.goal_path(id);
        let raw = std::fs::read(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                AdeError::NotFound(format!("eng-goal '{id}' not found"))
            } else {
                AdeError::Io(error)
            }
        })?;
        let goal: EngGoal = serde_json::from_slice(&raw)?;
        validate_goal(&goal)?;
        Ok(goal)
    }

    pub fn list(&self) -> Result<Vec<EngGoal>, AdeError> {
        let dir = self.goals_dir();
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut goals = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == "active.json" || !name.ends_with(".json") {
                continue;
            }
            let id = name.trim_end_matches(".json");
            if let Ok(goal) = self.load(id) {
                goals.push(goal);
            }
        }
        goals.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(goals)
    }

    pub fn set_active(&self, id: &str) -> Result<EngGoal, AdeError> {
        let mut goal = self.load(id)?;
        goal.status = "active".into();
        goal.updated_at = Utc::now().to_rfc3339();
        self.save(&goal)?;
        let dir = self.goals_dir();
        std::fs::create_dir_all(&dir)?;
        let pointer = ActiveGoalPointer {
            id: goal.id.clone(),
        };
        let payload = serde_json::to_vec_pretty(&pointer)?;
        write_atomic(&self.active_path(), &payload)?;
        Ok(goal)
    }

    pub fn clear_active(&self) -> Result<(), AdeError> {
        let path = self.active_path();
        if path.is_file() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    pub fn load_active(&self) -> Result<Option<EngGoal>, AdeError> {
        let path = self.active_path();
        if !path.is_file() {
            return Ok(None);
        }
        let raw = std::fs::read(&path)?;
        let pointer: ActiveGoalPointer = serde_json::from_slice(&raw)?;
        match self.load(&pointer.id) {
            Ok(goal) => Ok(Some(goal)),
            Err(AdeError::NotFound(_)) => {
                let _ = std::fs::remove_file(&path);
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    pub fn mark_status(&self, id: &str, status: &str) -> Result<EngGoal, AdeError> {
        let status = normalize_status(status)?;
        let mut goal = self.load(id)?;
        goal.status = status;
        goal.updated_at = Utc::now().to_rfc3339();
        self.save(&goal)?;
        if goal.status == "done" || goal.status == "abandoned" {
            if let Ok(Some(active)) = self.load_active() {
                if active.id == goal.id {
                    self.clear_active()?;
                }
            }
        }
        Ok(goal)
    }

    pub fn attach_handoff(&self, id: &str, handoff_id: &str) -> Result<EngGoal, AdeError> {
        let mut goal = self.load(id)?;
        goal.last_handoff_id = Some(handoff_id.trim().to_string());
        goal.updated_at = Utc::now().to_rfc3339();
        self.save(&goal)?;
        Ok(goal)
    }

    /// Patch contract fields on an existing goal (AC / OOS / verify / clarify).
    pub fn update_contract(
        &self,
        id: &str,
        success_criteria: Option<Vec<String>>,
        out_of_scope: Option<Vec<String>>,
        verify_gate: Option<Option<String>>,
        clarify_resolutions: Option<Vec<String>>,
    ) -> Result<EngGoal, AdeError> {
        let mut goal = self.load(id)?;
        if let Some(criteria) = success_criteria {
            goal.success_criteria = criteria
                .into_iter()
                .map(|c| c.trim().to_string())
                .filter(|c| !c.is_empty())
                .take(12)
                .collect();
        }
        if let Some(oos) = out_of_scope {
            goal.out_of_scope = oos
                .into_iter()
                .map(|c| c.trim().to_string())
                .filter(|c| !c.is_empty())
                .take(12)
                .collect();
        }
        if let Some(gate) = verify_gate {
            goal.verify_gate = gate.map(|g| g.trim().to_string()).filter(|g| !g.is_empty());
        }
        if let Some(clarify) = clarify_resolutions {
            goal.clarify_resolutions = clarify
                .into_iter()
                .map(|c| c.trim().to_string())
                .filter(|c| !c.is_empty())
                .take(3)
                .collect();
        }
        goal.updated_at = Utc::now().to_rfc3339();
        self.save(&goal)?;
        Ok(goal)
    }

    /// Log a human waive so Apply can proceed without a full contract.
    pub fn waive_contract(&self, id: &str, reason: &str) -> Result<EngGoal, AdeError> {
        let reason = reason.trim();
        if reason.is_empty() {
            return Err(AdeError::Config(
                "contract waive reason cannot be empty".into(),
            ));
        }
        let mut goal = self.load(id)?;
        goal.contract_waive = Some(ContractWaive {
            at: Utc::now().to_rfc3339(),
            reason: reason.chars().take(500).collect(),
        });
        goal.updated_at = Utc::now().to_rfc3339();
        self.save(&goal)?;
        Ok(goal)
    }
}

/// Authorization error when Act/Automate lacks a ready contract and no active goal.
pub fn no_active_goal_contract_error() -> AdeError {
    AdeError::Authorization(format!(
        "{CONTRACT_GATE_PREFIX} Act tools blocked until an active eng-goal has acceptance criteria, out-of-scope, and verify pointer (or ≤3 clarify / logged waive). Define a goal or switch to Suggest."
    ))
}

/// True for tool effects that mutate workspace/process (Apply-class).
pub fn is_act_class_effect(effect: crate::authority::ToolEffect) -> bool {
    use crate::authority::ToolEffect;
    matches!(
        effect,
        ToolEffect::WorkspaceWrite
            | ToolEffect::ExternalWrite
            | ToolEffect::ProcessExecution
            | ToolEffect::Unknown
    )
}

fn validate_id(id: &str) -> Result<&str, AdeError> {
    let id = id.trim();
    if id.is_empty() || id.len() > 64 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(AdeError::Config("invalid eng-goal id".into()));
    }
    Ok(id)
}

fn validate_goal(goal: &EngGoal) -> Result<(), AdeError> {
    if goal.schema != GOAL_SCHEMA {
        return Err(AdeError::Config(format!(
            "unsupported eng-goal schema '{}'",
            goal.schema
        )));
    }
    validate_id(&goal.id)?;
    if goal.statement.trim().is_empty() {
        return Err(AdeError::Config("goal statement cannot be empty".into()));
    }
    normalize_shell_scope(&goal.shell_scope)?;
    normalize_autonomy(&goal.autonomy)?;
    normalize_status(&goal.status)?;
    Ok(())
}

fn normalize_shell_scope(raw: &str) -> Result<String, AdeError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "workspace" => Ok("workspace".into()),
        "home" | "desktop" | "profile" => Ok("home".into()),
        other => Err(AdeError::Config(format!(
            "unknown shell_scope '{other}' (expected workspace|home)"
        ))),
    }
}

fn normalize_autonomy(raw: &str) -> Result<String, AdeError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "observe" | "propose" | "act" | "automate" => Ok(raw.trim().to_ascii_lowercase()),
        other => Err(AdeError::Config(format!(
            "unknown autonomy '{other}' (expected observe|propose|act|automate)"
        ))),
    }
}

fn normalize_status(raw: &str) -> Result<String, AdeError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "active" | "paused" | "done" | "abandoned" => Ok(raw.trim().to_ascii_lowercase()),
        other => Err(AdeError::Config(format!(
            "unknown goal status '{other}' (expected active|paused|done|abandoned)"
        ))),
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), AdeError> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_activate_load_and_done() {
        let root = std::env::temp_dir().join(format!("ade-goal-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let store = GoalStore::new(&root);
        let goal = store
            .create(GoalCreateInput {
                statement: "Organize Desktop into typed folders".into(),
                success_criteria: vec!["No loose PDFs on Desktop".into()],
                out_of_scope: vec!["Do not delete Documents".into()],
                shell_scope: "home".into(),
                autonomy: "act".into(),
                verify_gate: Some("G3".into()),
                owned_paths: vec![],
                activate: true,
            })
            .unwrap();
        assert_eq!(goal.shell_scope, "home");
        assert!(goal.is_contract_ready());
        assert!(goal.allows_act_tools());
        let active = store.load_active().unwrap().expect("active");
        assert_eq!(active.id, goal.id);
        assert!(active.prompt_block().contains("ENG GOAL"));
        assert!(active.prompt_block().contains("Out of scope"));
        assert!(active.run_prompt().contains("Organize Desktop"));

        store.mark_status(&goal.id, "done").unwrap();
        assert!(store.load_active().unwrap().is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn contract_gate_requires_ac_oos_verify_or_waive() {
        let root = std::env::temp_dir().join(format!("ade-goal-contract-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let store = GoalStore::new(&root);
        let incomplete = store
            .create(GoalCreateInput {
                statement: "Ship G1".into(),
                success_criteria: vec![],
                out_of_scope: vec![],
                shell_scope: "workspace".into(),
                autonomy: "act".into(),
                verify_gate: None,
                owned_paths: vec![],
                activate: true,
            })
            .unwrap();
        assert!(!incomplete.is_contract_ready());
        assert!(!incomplete.allows_act_tools());
        assert!(incomplete
            .contract_block_detail()
            .starts_with(CONTRACT_GATE_PREFIX));

        let ready = store
            .update_contract(
                &incomplete.id,
                Some(vec!["Tests pass".into()]),
                Some(vec!["No refactor".into()]),
                Some(Some("G3".into())),
                None,
            )
            .unwrap();
        assert!(ready.is_contract_ready());

        let root2 = std::env::temp_dir().join(format!("ade-goal-waive-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root2).unwrap();
        let store2 = GoalStore::new(&root2);
        let bare = store2
            .create(GoalCreateInput {
                statement: "Hotfix".into(),
                success_criteria: vec![],
                out_of_scope: vec![],
                shell_scope: "workspace".into(),
                autonomy: "act".into(),
                verify_gate: None,
                owned_paths: vec![],
                activate: true,
            })
            .unwrap();
        let waived = store2
            .waive_contract(&bare.id, "emergency path fix")
            .unwrap();
        assert!(!waived.is_contract_ready());
        assert!(waived.allows_act_tools());
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(root2);
    }

    #[test]
    fn list_skips_active_pointer() {
        let root = std::env::temp_dir().join(format!("ade-goal-list-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let store = GoalStore::new(&root);
        store
            .create(GoalCreateInput {
                statement: "Ship G2".into(),
                success_criteria: vec![],
                out_of_scope: vec![],
                shell_scope: "workspace".into(),
                autonomy: "propose".into(),
                verify_gate: None,
                owned_paths: vec![],
                activate: true,
            })
            .unwrap();
        let listed = store.list().unwrap();
        assert_eq!(listed.len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }
}
