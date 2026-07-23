//! Model profiles + router (H3) — `ade.model-profile/v1`.
//!
//! Binds provider/model traits → autonomy ceiling, tool mask, effort floor,
//! spend ceiling, and slot eligibility. Router annotates turns with a visible
//! "why this model" reason — never silent mid-task swaps.

use crate::authority::ToolEffect;
use crate::autonomy::AutonomyLevel;
use crate::slots::SlotRole;
use ade_core::error::AdeError;
use ade_core::money::Money;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const MODEL_PROFILE_SCHEMA: &str = "ade.model-profile/v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelProfile {
    #[serde(default = "default_schema")]
    pub schema: String,
    pub id: String,
    #[serde(default)]
    pub label: String,
    /// Exact provider id, or empty/`*` for any.
    #[serde(default)]
    pub provider: String,
    /// Exact model id preferred by this profile, or empty for role-only binding.
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub allowed_autonomy: Vec<String>,
    /// ToolEffect names this profile may not run (`workspace_write`, `process_execution`, …).
    #[serde(default)]
    pub tool_effect_deny: Vec<String>,
    #[serde(default)]
    pub effort_floor_steps: Option<u32>,
    #[serde(default)]
    pub spend_ceiling_usd: Option<f64>,
    #[serde(default)]
    pub slot_eligibility: Vec<String>,
    #[serde(default)]
    pub require_verify: bool,
    #[serde(default)]
    pub prefer_worktree: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_schema() -> String {
    MODEL_PROFILE_SCHEMA.into()
}

impl ModelProfile {
    pub fn display_label(&self) -> &str {
        if self.label.trim().is_empty() {
            self.id.as_str()
        } else {
            self.label.as_str()
        }
    }

    pub fn allows_slot(&self, slot: SlotRole) -> bool {
        if self.slot_eligibility.is_empty() {
            return true;
        }
        self.slot_eligibility
            .iter()
            .any(|s| SlotRole::parse(s).ok() == Some(slot))
    }

    pub fn allows_autonomy(&self, autonomy: AutonomyLevel) -> bool {
        if self.allowed_autonomy.is_empty() {
            return true;
        }
        self.allowed_autonomy
            .iter()
            .any(|a| a.eq_ignore_ascii_case(autonomy.as_str()))
    }

    pub fn denies_effect(&self, effect: ToolEffect) -> bool {
        let needle = effect_label(effect);
        self.tool_effect_deny
            .iter()
            .any(|d| d.eq_ignore_ascii_case(needle))
    }

    pub fn provider_matches(&self, provider: &str) -> bool {
        let p = self.provider.trim();
        p.is_empty() || p == "*" || p.eq_ignore_ascii_case(provider.trim())
    }

    pub fn model_matches(&self, model: &str) -> bool {
        let m = self.model.trim();
        !m.is_empty() && m.eq_ignore_ascii_case(model.trim())
    }

    pub fn spend_ceiling(&self) -> Result<Option<Money>, AdeError> {
        match self.spend_ceiling_usd {
            Some(usd) => Ok(Some(Money::try_from_usd_f64(usd)?)),
            None => Ok(None),
        }
    }
}

fn effect_label(effect: ToolEffect) -> &'static str {
    match effect {
        ToolEffect::ReadOnly => "read_only",
        ToolEffect::WorkspaceWrite => "workspace_write",
        ToolEffect::ExternalWrite => "external_write",
        ToolEffect::ProcessExecution => "process_execution",
        ToolEffect::Unknown => "unknown",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteInput {
    pub provider: String,
    pub model: String,
    pub autonomy: AutonomyLevel,
    pub max_tool_rounds: usize,
    pub session_cap: Option<Money>,
    #[serde(default)]
    pub slot_override: Option<SlotRole>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDecision {
    pub profile_id: String,
    pub profile_label: String,
    pub slot: SlotRole,
    pub reason: String,
    pub eligible: bool,
    pub warnings: Vec<String>,
    pub effort_floor_steps: Option<u32>,
    pub require_verify: bool,
    pub prefer_worktree: bool,
    pub tool_effect_deny: Vec<String>,
}

impl RouteDecision {
    pub fn effective_max_tool_rounds(&self, requested: usize) -> usize {
        match self.effort_floor_steps {
            Some(floor) => requested.max(floor as usize).max(1),
            None => requested.max(1),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ModelProfileCatalog {
    profiles: Vec<ModelProfile>,
}

impl ModelProfileCatalog {
    pub fn builtins() -> Self {
        Self {
            profiles: builtin_profiles(),
        }
    }

    pub fn load(workspace_root: impl AsRef<Path>) -> Self {
        let mut catalog = Self::builtins();
        let dir = workspace_root.as_ref().join(".ade").join("model-profiles");
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let is_json = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("json"))
                    .unwrap_or(false);
                if is_json {
                    if let Ok(raw) = std::fs::read_to_string(&path) {
                        if let Ok(profile) = serde_json::from_str::<ModelProfile>(&raw) {
                            catalog.upsert(profile);
                        }
                    }
                }
            }
        }
        catalog
    }

    fn upsert(&mut self, profile: ModelProfile) {
        self.profiles.retain(|p| p.id != profile.id);
        self.profiles.push(profile);
    }

    pub fn profiles(&self) -> &[ModelProfile] {
        &self.profiles
    }

    pub fn get(&self, id: &str) -> Option<&ModelProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn find_exact(&self, provider: &str, model: &str) -> Option<&ModelProfile> {
        self.profiles
            .iter()
            .find(|p| p.provider_matches(provider) && p.model_matches(model))
    }

    pub fn default_for_slot(&self, slot: SlotRole) -> &ModelProfile {
        let prefer_tag = match slot {
            SlotRole::Planner => "fast",
            SlotRole::Worker => "strong",
            SlotRole::Verifier => "independent",
        };
        self.profiles
            .iter()
            .find(|p| p.allows_slot(slot) && p.tags.iter().any(|t| t == prefer_tag))
            .or_else(|| self.profiles.iter().find(|p| p.allows_slot(slot)))
            .unwrap_or(&self.profiles[0])
    }
}

/// Route by slot × autonomy × spend headroom; keep user model lock visible.
pub fn route(catalog: &ModelProfileCatalog, input: &RouteInput) -> RouteDecision {
    let slot = input
        .slot_override
        .unwrap_or_else(|| SlotRole::from_autonomy(input.autonomy));
    let mut warnings = Vec::new();

    if let Some(exact) = catalog.find_exact(&input.provider, &input.model) {
        let eligible = exact.allows_slot(slot) && exact.allows_autonomy(input.autonomy);
        if !eligible {
            warnings.push(format!(
                "profile '{}' is not eligible for slot={} autonomy={}",
                exact.id,
                slot.as_str(),
                input.autonomy.as_str()
            ));
        }
        if let Ok(Some(ceiling)) = exact.spend_ceiling() {
            if let Some(cap) = input.session_cap {
                if cap > Money::ZERO && ceiling < cap {
                    warnings.push(format!(
                        "profile spend ceiling ({ceiling}) is below session cap ({cap})"
                    ));
                }
            }
        }
        return RouteDecision {
            profile_id: exact.id.clone(),
            profile_label: exact.display_label().to_string(),
            slot,
            reason: if eligible {
                format!(
                    "user lock · {} ({}) · slot={}",
                    exact.id,
                    exact.display_label(),
                    slot.as_str()
                )
            } else {
                format!(
                    "user lock · {} — not eligible for {} / {}",
                    exact.id,
                    slot.as_str(),
                    input.autonomy.as_str()
                )
            },
            eligible,
            warnings,
            effort_floor_steps: exact.effort_floor_steps,
            require_verify: exact.require_verify,
            prefer_worktree: exact.prefer_worktree,
            tool_effect_deny: exact.tool_effect_deny.clone(),
        };
    }

    let role = catalog.default_for_slot(slot);
    if !role.allows_autonomy(input.autonomy) {
        warnings.push(format!(
            "role default '{}' disallows autonomy={}",
            role.id,
            input.autonomy.as_str()
        ));
    }
    RouteDecision {
        profile_id: role.id.clone(),
        profile_label: role.display_label().to_string(),
        slot,
        reason: format!(
            "{} · {} for {} (model locked: {}/{}",
            role.id,
            role.display_label(),
            slot.as_str(),
            input.provider.trim(),
            input.model.trim()
        ) + ")",
        eligible: role.allows_slot(slot) && role.allows_autonomy(input.autonomy),
        warnings,
        effort_floor_steps: role.effort_floor_steps,
        require_verify: role.require_verify,
        prefer_worktree: role.prefer_worktree,
        tool_effect_deny: role.tool_effect_deny.clone(),
    }
}

pub fn ensure_default_profiles(workspace_root: impl AsRef<Path>) -> Result<PathBuf, AdeError> {
    let dir = workspace_root.as_ref().join(".ade").join("model-profiles");
    std::fs::create_dir_all(&dir)
        .map_err(|e| AdeError::Config(format!("cannot create model-profiles dir: {e}")))?;
    for profile in builtin_profiles() {
        let path = dir.join(format!("{}.json", profile.id));
        if path.exists() {
            continue;
        }
        let raw = serde_json::to_string_pretty(&profile)
            .map_err(|e| AdeError::Config(format!("serialize model profile: {e}")))?;
        std::fs::write(&path, raw)
            .map_err(|e| AdeError::Config(format!("write {}: {e}", path.display())))?;
    }
    Ok(dir)
}

fn builtin_profiles() -> Vec<ModelProfile> {
    vec![
        ModelProfile {
            schema: MODEL_PROFILE_SCHEMA.into(),
            id: "planner-fast".into(),
            label: "Planner (fast)".into(),
            provider: String::new(),
            model: String::new(),
            allowed_autonomy: vec!["observe".into(), "propose".into()],
            tool_effect_deny: vec!["workspace_write".into(), "external_write".into()],
            effort_floor_steps: Some(8),
            spend_ceiling_usd: None,
            slot_eligibility: vec!["planner".into()],
            require_verify: false,
            prefer_worktree: false,
            tags: vec!["fast".into(), "cheap".into()],
        },
        ModelProfile {
            schema: MODEL_PROFILE_SCHEMA.into(),
            id: "worker-strong".into(),
            label: "Worker (strong)".into(),
            provider: String::new(),
            model: String::new(),
            allowed_autonomy: vec!["act".into(), "automate".into()],
            tool_effect_deny: vec![],
            effort_floor_steps: Some(16),
            spend_ceiling_usd: None,
            slot_eligibility: vec!["worker".into()],
            require_verify: false,
            prefer_worktree: true,
            tags: vec!["strong".into()],
        },
        ModelProfile {
            schema: MODEL_PROFILE_SCHEMA.into(),
            id: "verifier-independent".into(),
            label: "Verifier (independent)".into(),
            provider: String::new(),
            model: String::new(),
            allowed_autonomy: vec!["observe".into(), "propose".into()],
            tool_effect_deny: vec![
                "workspace_write".into(),
                "external_write".into(),
                "process_execution".into(),
            ],
            effort_floor_steps: Some(8),
            spend_ceiling_usd: None,
            slot_eligibility: vec!["verifier".into()],
            require_verify: true,
            prefer_worktree: false,
            tags: vec!["independent".into()],
        },
        ModelProfile {
            schema: MODEL_PROFILE_SCHEMA.into(),
            id: "scout-cheap".into(),
            label: "Scout (cheap)".into(),
            provider: String::new(),
            model: String::new(),
            allowed_autonomy: vec!["observe".into(), "propose".into()],
            tool_effect_deny: vec!["external_write".into()],
            effort_floor_steps: Some(4),
            spend_ceiling_usd: Some(0.50),
            slot_eligibility: vec!["planner".into()],
            require_verify: false,
            prefer_worktree: false,
            tags: vec!["cheap".into(), "scout".into()],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_annotates_user_lock_for_worker() {
        let catalog = ModelProfileCatalog::builtins();
        let decision = route(
            &catalog,
            &RouteInput {
                provider: "opencode".into(),
                model: "claude-sonnet-4".into(),
                autonomy: AutonomyLevel::Act,
                max_tool_rounds: 24,
                session_cap: None,
                slot_override: None,
            },
        );
        assert_eq!(decision.slot, SlotRole::Worker);
        assert_eq!(decision.profile_id, "worker-strong");
        assert!(decision.reason.contains("worker"));
        assert!(decision.eligible);
        assert_eq!(decision.effective_max_tool_rounds(8), 16);
    }

    #[test]
    fn planner_profile_denies_writes() {
        let catalog = ModelProfileCatalog::builtins();
        let p = catalog.get("planner-fast").unwrap();
        assert!(p.denies_effect(ToolEffect::WorkspaceWrite));
        assert!(!p.allows_autonomy(AutonomyLevel::Act));
    }

    #[test]
    fn suggest_routes_to_planner_fast() {
        let catalog = ModelProfileCatalog::builtins();
        let decision = route(
            &catalog,
            &RouteInput {
                provider: "openai".into(),
                model: "gpt-4.1-mini".into(),
                autonomy: AutonomyLevel::Propose,
                max_tool_rounds: 16,
                session_cap: None,
                slot_override: None,
            },
        );
        assert_eq!(decision.profile_id, "planner-fast");
        assert_eq!(decision.slot, SlotRole::Planner);
    }

    #[test]
    fn verifier_override_routes_independent() {
        let catalog = ModelProfileCatalog::builtins();
        let decision = route(
            &catalog,
            &RouteInput {
                provider: "opencode".into(),
                model: "test-model".into(),
                autonomy: AutonomyLevel::Propose,
                max_tool_rounds: 8,
                session_cap: None,
                slot_override: Some(SlotRole::Verifier),
            },
        );
        assert_eq!(decision.slot, SlotRole::Verifier);
        assert_eq!(decision.profile_id, "verifier-independent");
    }
}
