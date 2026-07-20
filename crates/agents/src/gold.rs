//! Deterministic Ideal ADE gold-set evals (I5).
//!
//! These tasks gate harness changes without live LLM spend. Expand toward 50
//! tasks over time; dogfood/self-build probes are included.

use crate::autonomy::AutonomyLevel;
use crate::authority::{classify_tool_effect, ToolAuthRequest, ToolEffect, WriteScope};
use crate::context::ContextBudget;
use crate::skills::SkillLoader;
use crate::start_prompt::StartPromptBuilder;
use ade_core::error::AdeError;
use ade_core::guided::{self, GuidedWinId};
use ade_core::money::Money;
use ade_workflow::plan_enforcement::PlanEnforcer;
use ade_workflow::verify::VerifyRunner;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const GOLD_MANIFEST_REL: &str = "evals/gold/manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldTask {
    pub id: String,
    pub title: String,
    pub kind: String,
    #[serde(default)]
    pub dogfood: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldManifest {
    pub schema: String,
    pub tasks: Vec<GoldTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldTaskResult {
    pub id: String,
    pub title: String,
    pub passed: bool,
    pub detail: String,
    pub dogfood: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldReport {
    pub schema: String,
    pub passed: usize,
    pub failed: usize,
    pub total: usize,
    pub results: Vec<GoldTaskResult>,
}

impl GoldReport {
    pub fn ok(&self) -> bool {
        self.failed == 0 && self.total > 0
    }
}

pub struct GoldRunner {
    root: PathBuf,
}

impl GoldRunner {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn run_builtin(&self) -> GoldReport {
        let results = builtin_tasks()
            .iter()
            .map(|task| self.run_task(task))
            .collect::<Vec<_>>();
        summarize(results)
    }

    pub fn run_manifest(&self) -> Result<GoldReport, AdeError> {
        let path = self.root.join(GOLD_MANIFEST_REL);
        if !path.is_file() {
            return Ok(self.run_builtin());
        }
        let raw = fs::read_to_string(path)?;
        let manifest: GoldManifest = serde_json::from_str(&raw)?;
        let results = manifest
            .tasks
            .iter()
            .map(|task| self.run_task(task))
            .collect::<Vec<_>>();
        Ok(summarize(results))
    }

    fn run_task(&self, task: &GoldTask) -> GoldTaskResult {
        let outcome = match task.kind.as_str() {
            "agents_md_present" => probe_agents_md(&self.root),
            "skills_catalog_loads" => probe_skills_catalog(&self.root),
            "activate_skill_missing" => probe_activate_missing(&self.root),
            "autonomy_observe_blocks_write" => probe_autonomy_observe(),
            "tool_effect_activate_skill_readonly" => probe_activate_skill_effect(),
            "understand_artifact" => probe_understand(),
            "guided_mark_verify" => probe_guided_verify(),
            "money_roundtrip" => probe_money(),
            "plan_enforcer_blocks_risky" => probe_plan_enforcer(),
            "verify_g0" => probe_verify_g0(&self.root),
            "context_budget_skills_cap" => probe_context_budget(),
            "start_prompt_mentions_activate" => probe_start_prompt(),
            other => Err(format!("unknown gold task kind '{other}'")),
        };
        match outcome {
            Ok(detail) => GoldTaskResult {
                id: task.id.clone(),
                title: task.title.clone(),
                passed: true,
                detail,
                dogfood: task.dogfood,
            },
            Err(detail) => GoldTaskResult {
                id: task.id.clone(),
                title: task.title.clone(),
                passed: false,
                detail,
                dogfood: task.dogfood,
            },
        }
    }
}

fn summarize(results: Vec<GoldTaskResult>) -> GoldReport {
    let passed = results.iter().filter(|result| result.passed).count();
    let failed = results.len().saturating_sub(passed);
    GoldReport {
        schema: "ade.eval.gold-report/v1".into(),
        passed,
        failed,
        total: results.len(),
        results,
    }
}

fn builtin_tasks() -> Vec<GoldTask> {
    vec![
        task("g01", "AGENTS.md present", "agents_md_present", true),
        task("g02", "Skills catalog loads", "skills_catalog_loads", true),
        task(
            "g03",
            "activate_skill rejects unknown",
            "activate_skill_missing",
            false,
        ),
        task(
            "g04",
            "Observe autonomy blocks writes",
            "autonomy_observe_blocks_write",
            false,
        ),
        task(
            "g05",
            "activate_skill is ReadOnly",
            "tool_effect_activate_skill_readonly",
            false,
        ),
        task(
            "g06",
            "Understand artifact guided win",
            "understand_artifact",
            true,
        ),
        task(
            "g07",
            "Guided verify win persists",
            "guided_mark_verify",
            true,
        ),
        task("g08", "Money USD roundtrip", "money_roundtrip", false),
        task(
            "g09",
            "Plan enforcer blocks risky without plan",
            "plan_enforcer_blocks_risky",
            true,
        ),
        task("g10", "Verify G0 probe", "verify_g0", true),
        task(
            "g11",
            "Context budget skills cap",
            "context_budget_skills_cap",
            false,
        ),
        task(
            "g12",
            "T0 mentions activate_skill",
            "start_prompt_mentions_activate",
            false,
        ),
    ]
}

fn task(id: &str, title: &str, kind: &str, dogfood: bool) -> GoldTask {
    GoldTask {
        id: id.into(),
        title: title.into(),
        kind: kind.into(),
        dogfood,
    }
}

fn probe_agents_md(root: &Path) -> Result<String, String> {
    if root.join("AGENTS.md").is_file() {
        Ok("AGENTS.md found".into())
    } else {
        Err("AGENTS.md missing".into())
    }
}

fn probe_skills_catalog(root: &Path) -> Result<String, String> {
    let skills = SkillLoader::new(root)
        .load_all()
        .map_err(|error| error.to_string())?;
    if skills.is_empty() {
        return Err("no skills loaded from .ade/skills".into());
    }
    let prompt = SkillLoader::new(root)
        .prompt_context("hello", 2_000)
        .map_err(|error| error.to_string())?;
    if !prompt.contains("AVAILABLE SKILLS") {
        return Err("skills prompt missing T1 catalog".into());
    }
    Ok(format!("{} skills; catalog present", skills.len()))
}

fn probe_activate_missing(root: &Path) -> Result<String, String> {
    match SkillLoader::new(root).activate("definitely-missing-skill-xyz") {
        Err(error) => Ok(format!("expected miss: {error}")),
        Ok(_) => Err("activate unexpectedly succeeded".into()),
    }
}

fn probe_autonomy_observe() -> Result<String, String> {
    if AutonomyLevel::Observe.allows_tool_effect(ToolEffect::WorkspaceWrite) {
        return Err("Observe allowed WorkspaceWrite".into());
    }
    if !AutonomyLevel::Observe.allows_tool_effect(ToolEffect::ReadOnly) {
        return Err("Observe blocked ReadOnly".into());
    }
    Ok("Observe blocks writes".into())
}

fn probe_activate_skill_effect() -> Result<String, String> {
    let effect = classify_tool_effect(&ToolAuthRequest {
        server: "ade".into(),
        tool: "activate_skill".into(),
        arguments: serde_json::json!({ "name": "x" }),
        input_schema: None,
        annotations: None,
        write_scope: WriteScope::PlanOwnedPaths,
        human_approved: false,
    });
    if effect != ToolEffect::ReadOnly {
        return Err(format!("expected ReadOnly, got {effect:?}"));
    }
    Ok("activate_skill classified ReadOnly".into())
}

fn probe_understand() -> Result<String, String> {
    let scratch = std::env::temp_dir().join(format!("ade-gold-understand-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(scratch.join("crates")).map_err(|error| error.to_string())?;
    fs::write(scratch.join("Cargo.toml"), "[workspace]\nmembers=[]\n")
        .map_err(|error| error.to_string())?;
    fs::write(scratch.join("AGENTS.md"), "# Agents\n").map_err(|error| error.to_string())?;
    let result = guided::write_understand_project(&scratch).map_err(|error| error.to_string())?;
    let _ = fs::remove_dir_all(&scratch);
    if !result.wins.understand {
        return Err("understand win not marked".into());
    }
    Ok(format!("wrote {}", result.path))
}

fn probe_guided_verify() -> Result<String, String> {
    let scratch = std::env::temp_dir().join(format!("ade-gold-verify-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&scratch).map_err(|error| error.to_string())?;
    let wins = guided::mark_win(&scratch, GuidedWinId::Verify).map_err(|error| error.to_string())?;
    let _ = fs::remove_dir_all(&scratch);
    if !wins.verify {
        return Err("verify win not marked".into());
    }
    Ok("guided verify win persisted".into())
}

fn probe_money() -> Result<String, String> {
    let money = Money::try_from_usd_f64(1.25).map_err(|error| error.to_string())?;
    if money.micros() != 1_250_000 {
        return Err(format!("unexpected micros {}", money.micros()));
    }
    Ok("Money 1.25 USD = 1250000 micros".into())
}

fn probe_plan_enforcer() -> Result<String, String> {
    let scratch = std::env::temp_dir().join(format!("ade-gold-plan-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&scratch).map_err(|error| error.to_string())?;
    fs::write(scratch.join("AGENTS.md"), "# Agents\n").map_err(|error| error.to_string())?;
    let result = PlanEnforcer::new().ensure_approved_plan(
        &scratch,
        &["db/schema.sql".into()],
        Some("run the migration"),
    );
    let _ = fs::remove_dir_all(&scratch);
    match result {
        Err(_) => Ok("plan enforcer blocked risky work".into()),
        Ok(()) => Err("plan enforcer allowed risky work without plan".into()),
    }
}

fn probe_verify_g0(root: &Path) -> Result<String, String> {
    let result = VerifyRunner::with_root(root).run_gate_sync(ade_core::verify::VerifyGate::G0);
    if result.passed || result.status == ade_core::verify::VerifyStatus::Unavailable {
        Ok(format!("G0 {}", result.status_label()))
    } else {
        Err(format!("G0 failed: {}", result.command))
    }
}

fn probe_context_budget() -> Result<String, String> {
    let budget = ContextBudget::default_daily();
    if budget.skills_tokens == 0 {
        return Err("skills_tokens is zero".into());
    }
    Ok(format!("skills_tokens={}", budget.skills_tokens))
}

fn probe_start_prompt() -> Result<String, String> {
    let text = StartPromptBuilder::new().build();
    if !text.contains("ade__activate_skill") {
        return Err("T0 missing ade__activate_skill".into());
    }
    Ok("T0 mentions activate_skill".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_gold_set_has_at_least_ten_and_passes() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf();
        let report = GoldRunner::new(root).run_builtin();
        assert!(
            report.total >= 10,
            "expected ≥10 gold tasks, got {}",
            report.total
        );
        assert!(
            report.ok(),
            "gold set failures: {}",
            report
                .results
                .iter()
                .filter(|result| !result.passed)
                .map(|result| format!("{}: {}", result.id, result.detail))
                .collect::<Vec<_>>()
                .join("; ")
        );
        assert!(report.results.iter().any(|result| result.dogfood));
    }
}
