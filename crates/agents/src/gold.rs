//! Deterministic Ideal ADE gold-set evals (I5).
//!
//! These tasks gate harness changes without live LLM spend. Expand toward 50
//! tasks over time; dogfood/self-build probes are included.

use crate::authority::{classify_tool_effect, ToolAuthRequest, ToolEffect, WriteScope};
use crate::autonomy::AutonomyLevel;
use crate::context::ContextBudget;
use crate::skills::SkillLoader;
use crate::start_prompt::StartPromptBuilder;
use ade_core::error::AdeError;
use ade_core::guided::{self, GuidedWinId};
use ade_core::handoff::HandoffCapsule;
use ade_core::ignore::SensitivePathPolicy;
use ade_core::money::Money;
use ade_core::recipe::{builtin_recipe, canonical_recipe_ids};
use ade_core::verify::VerifyGate;
use ade_workflow::plan_enforcement::PlanEnforcer;
use ade_workflow::verify::VerifyRunner;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

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
            "autonomy_propose_blocks_write" => probe_autonomy_propose(),
            "autonomy_act_allows_write" => probe_autonomy_act(),
            "autonomy_automate_requires_verify" => probe_autonomy_automate_verify(),
            "autonomy_parse_roundtrip" => probe_autonomy_parse(),
            "autonomy_parse_rejects_unknown" => probe_autonomy_parse_bad(),
            "money_from_usd_str" => probe_money_str(),
            "money_cost_for_tokens" => probe_money_tokens(),
            "money_saturating_add" => probe_money_add(),
            "secret_path_blocks_env" => probe_secret_env(),
            "secret_path_allows_src" => probe_secret_src(),
            "env_example_not_secret" => probe_env_example(),
            "recipes_canonical_count" => probe_recipes_count(),
            "recipe_business_saas" => probe_recipe_saas(),
            "verify_gates_six" => probe_verify_gates(),
            "verify_gate_parse_g3" => probe_verify_parse(),
            "cargo_toml_present" => probe_cargo_toml(&self.root),
            "ade_rules_dir" => probe_ade_rules(&self.root),
            "ade_skills_dir" => probe_ade_skills(&self.root),
            "activate_known_skill" => probe_activate_known(&self.root),
            "tool_effect_fs_write" => probe_fs_write_effect(),
            "tool_effect_fs_read" => probe_fs_read_effect(),
            "tool_effect_shell_process" => probe_shell_effect(),
            "guided_mark_improve" => probe_guided_improve(),
            "guided_load_empty" => probe_guided_load(),
            "context_total_allowance" => probe_context_total(),
            "start_prompt_nonempty" => probe_start_prompt_len(),
            "agents_md_nonempty" => probe_agents_md_body(&self.root),
            "handoff_prompt_summary" => probe_handoff_summary(),
            "observe_no_mutating" => probe_observe_mutating_flag(),
            "propose_no_mutating" => probe_propose_mutating_flag(),
            "act_mutating" => probe_act_mutating_flag(),
            "automate_mutating" => probe_automate_mutating_flag(),
            "verify_gate_id_g0" => probe_gate_id_g0(),
            "money_rejects_nan" => probe_money_nan(),
            "classify_git_push_external" => probe_git_push_effect(),
            "skill_catalog_mentions_name" => probe_catalog_mentions(&self.root),
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
        task(
            "g13",
            "Propose autonomy blocks writes",
            "autonomy_propose_blocks_write",
            false,
        ),
        task(
            "g14",
            "Act autonomy allows writes",
            "autonomy_act_allows_write",
            false,
        ),
        task(
            "g15",
            "Automate requires verify-on-complete",
            "autonomy_automate_requires_verify",
            false,
        ),
        task(
            "g16",
            "Autonomy parse roundtrip",
            "autonomy_parse_roundtrip",
            false,
        ),
        task(
            "g17",
            "Autonomy parse rejects unknown",
            "autonomy_parse_rejects_unknown",
            false,
        ),
        task("g18", "Money from USD string", "money_from_usd_str", false),
        task(
            "g19",
            "Money cost_for_tokens",
            "money_cost_for_tokens",
            false,
        ),
        task("g20", "Money saturating_add", "money_saturating_add", false),
        task(
            "g21",
            "Secret path blocks .env",
            "secret_path_blocks_env",
            true,
        ),
        task(
            "g22",
            "Secret path allows src",
            "secret_path_allows_src",
            false,
        ),
        task(
            "g23",
            ".env.example is not secret",
            "env_example_not_secret",
            false,
        ),
        task(
            "g24",
            "Canonical recipes count",
            "recipes_canonical_count",
            true,
        ),
        task(
            "g25",
            "business-saas recipe loads",
            "recipe_business_saas",
            true,
        ),
        task("g26", "Verify gates are six", "verify_gates_six", false),
        task("g27", "VerifyGate parse G3", "verify_gate_parse_g3", false),
        task("g28", "Cargo.toml present", "cargo_toml_present", true),
        task("g29", ".ade/rules present", "ade_rules_dir", true),
        task("g30", ".ade/skills present", "ade_skills_dir", true),
        task("g31", "activate known skill", "activate_known_skill", true),
        task(
            "g32",
            "fs write_file is WorkspaceWrite",
            "tool_effect_fs_write",
            false,
        ),
        task(
            "g33",
            "fs read_file is ReadOnly",
            "tool_effect_fs_read",
            false,
        ),
        task(
            "g34",
            "shell run_command is ProcessExecution",
            "tool_effect_shell_process",
            false,
        ),
        task(
            "g35",
            "Guided improve win persists",
            "guided_mark_improve",
            true,
        ),
        task("g36", "Guided wins load empty", "guided_load_empty", false),
        task(
            "g37",
            "Context total allowance",
            "context_total_allowance",
            false,
        ),
        task(
            "g38",
            "T0 start prompt nonempty",
            "start_prompt_nonempty",
            false,
        ),
        task("g39", "AGENTS.md nonempty", "agents_md_nonempty", true),
        task(
            "g40",
            "Handoff prompt_summary",
            "handoff_prompt_summary",
            false,
        ),
        task(
            "g41",
            "Observe no mutating tools",
            "observe_no_mutating",
            false,
        ),
        task(
            "g42",
            "Propose no mutating tools",
            "propose_no_mutating",
            false,
        ),
        task("g43", "Act allows mutating tools", "act_mutating", false),
        task(
            "g44",
            "Automate allows mutating tools",
            "automate_mutating",
            false,
        ),
        task("g45", "VerifyGate G0 id", "verify_gate_id_g0", false),
        task("g46", "Money rejects NaN", "money_rejects_nan", false),
        task(
            "g47",
            "git push is ExternalWrite",
            "classify_git_push_external",
            false,
        ),
        task(
            "g48",
            "Skill catalog mentions a loaded skill",
            "skill_catalog_mentions_name",
            true,
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
    let scratch =
        std::env::temp_dir().join(format!("ade-gold-understand-{}", uuid::Uuid::new_v4()));
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
    let wins =
        guided::mark_win(&scratch, GuidedWinId::Verify).map_err(|error| error.to_string())?;
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

fn auth(server: &str, tool: &str) -> ToolAuthRequest {
    ToolAuthRequest {
        server: server.into(),
        tool: tool.into(),
        arguments: serde_json::json!({}),
        input_schema: None,
        annotations: None,
        write_scope: WriteScope::PlanOwnedPaths,
        human_approved: false,
    }
}

fn expect_effect(server: &str, tool: &str, want: ToolEffect) -> Result<String, String> {
    let got = classify_tool_effect(&auth(server, tool));
    if got != want {
        return Err(format!("{server}::{tool} expected {want:?}, got {got:?}"));
    }
    Ok(format!("{server}::{tool}={want:?}"))
}

fn probe_autonomy_propose() -> Result<String, String> {
    if AutonomyLevel::Propose.allows_tool_effect(ToolEffect::WorkspaceWrite) {
        return Err("Propose allowed WorkspaceWrite".into());
    }
    Ok("Propose blocks writes".into())
}

fn probe_autonomy_act() -> Result<String, String> {
    if !AutonomyLevel::Act.allows_tool_effect(ToolEffect::WorkspaceWrite) {
        return Err("Act blocked WorkspaceWrite".into());
    }
    Ok("Act allows writes".into())
}

fn probe_autonomy_automate_verify() -> Result<String, String> {
    if !AutonomyLevel::Automate.requires_verify_on_complete() {
        return Err("Automate missing verify-on-complete".into());
    }
    if AutonomyLevel::Act.requires_verify_on_complete() {
        return Err("Act unexpectedly requires verify".into());
    }
    Ok("Automate requires verify".into())
}

fn probe_autonomy_parse() -> Result<String, String> {
    for level in [
        AutonomyLevel::Observe,
        AutonomyLevel::Propose,
        AutonomyLevel::Act,
        AutonomyLevel::Automate,
    ] {
        let parsed = AutonomyLevel::from_str(level.as_str()).map_err(|e| e)?;
        if parsed != level {
            return Err(format!("parse mismatch for {}", level.as_str()));
        }
    }
    Ok("autonomy parse roundtrip".into())
}

fn probe_autonomy_parse_bad() -> Result<String, String> {
    match AutonomyLevel::from_str("yolo") {
        Err(_) => Ok("rejected unknown autonomy".into()),
        Ok(_) => Err("accepted unknown autonomy".into()),
    }
}

fn probe_money_str() -> Result<String, String> {
    let money = Money::from_usd_str("2.50").map_err(|e| e.to_string())?;
    if money.micros() != 2_500_000 {
        return Err(format!("unexpected micros {}", money.micros()));
    }
    Ok("Money from_usd_str 2.50".into())
}

fn probe_money_tokens() -> Result<String, String> {
    let rate = Money::try_from_usd_f64(1.0).map_err(|e| e.to_string())?;
    let cost = Money::cost_for_tokens(1_000_000, rate);
    if cost.micros() != rate.micros() {
        return Err(format!("unexpected cost micros {}", cost.micros()));
    }
    Ok("cost_for_tokens 1M @ $1".into())
}

fn probe_money_add() -> Result<String, String> {
    let a = Money::try_from_usd_f64(0.10).map_err(|e| e.to_string())?;
    let b = Money::try_from_usd_f64(0.20).map_err(|e| e.to_string())?;
    let sum = a.saturating_add(b);
    if sum.micros() != 300_000 {
        return Err(format!("unexpected sum {}", sum.micros()));
    }
    Ok("saturating_add 0.10+0.20".into())
}

fn probe_secret_env() -> Result<String, String> {
    if !SensitivePathPolicy::is_secret_path(".env") {
        return Err(".env not secret".into());
    }
    Ok(".env blocked".into())
}

fn probe_secret_src() -> Result<String, String> {
    if SensitivePathPolicy::path_is_blocked("src/lib.rs") {
        return Err("src/lib.rs blocked".into());
    }
    Ok("src allowed".into())
}

fn probe_env_example() -> Result<String, String> {
    if SensitivePathPolicy::is_secret_path(".env.example") {
        return Err(".env.example treated as secret".into());
    }
    Ok(".env.example ok".into())
}

fn probe_recipes_count() -> Result<String, String> {
    let ids = canonical_recipe_ids();
    if ids.len() < 10 {
        return Err(format!("expected ≥10 recipes, got {}", ids.len()));
    }
    Ok(format!("{} canonical recipes", ids.len()))
}

fn probe_recipe_saas() -> Result<String, String> {
    let recipe = builtin_recipe("business-saas").map_err(|e| e.to_string())?;
    if recipe.id != "business-saas" {
        return Err(format!("unexpected id {}", recipe.id));
    }
    Ok("business-saas loaded".into())
}

fn probe_verify_gates() -> Result<String, String> {
    let gates = VerifyGate::all();
    if gates.len() != 6 {
        return Err(format!("expected 6 gates, got {}", gates.len()));
    }
    Ok("G0–G5 present".into())
}

fn probe_verify_parse() -> Result<String, String> {
    let gate = VerifyGate::from_str("G3").map_err(|e| e)?;
    if gate != VerifyGate::G3 {
        return Err("parse != G3".into());
    }
    Ok("parsed G3".into())
}

fn probe_cargo_toml(root: &Path) -> Result<String, String> {
    if root.join("Cargo.toml").is_file() {
        Ok("Cargo.toml found".into())
    } else {
        Err("Cargo.toml missing".into())
    }
}

fn probe_ade_rules(root: &Path) -> Result<String, String> {
    let dir = root.join(".ade/rules");
    if !dir.is_dir() {
        return Err(".ade/rules missing".into());
    }
    let count = fs::read_dir(&dir).map_err(|e| e.to_string())?.count();
    if count == 0 {
        return Err(".ade/rules empty".into());
    }
    Ok(format!("{count} rules"))
}

fn probe_ade_skills(root: &Path) -> Result<String, String> {
    let dir = root.join(".ade/skills");
    if !dir.is_dir() {
        return Err(".ade/skills missing".into());
    }
    Ok(".ade/skills present".into())
}

fn probe_activate_known(root: &Path) -> Result<String, String> {
    let skills = SkillLoader::new(root)
        .load_all()
        .map_err(|e| e.to_string())?;
    let name = skills
        .first()
        .map(|s| s.name.clone())
        .ok_or_else(|| "no skills to activate".to_string())?;
    let skill = SkillLoader::new(root)
        .activate(&name)
        .map_err(|e| e.to_string())?;
    if skill.body.trim().is_empty() {
        return Err("activated skill body empty".into());
    }
    Ok(format!("activated {name}"))
}

fn probe_fs_write_effect() -> Result<String, String> {
    expect_effect("fs", "write_file", ToolEffect::WorkspaceWrite)
}

fn probe_fs_read_effect() -> Result<String, String> {
    expect_effect("fs", "read_file", ToolEffect::ReadOnly)
}

fn probe_shell_effect() -> Result<String, String> {
    expect_effect("shell", "run_command", ToolEffect::ProcessExecution)
}

fn probe_guided_improve() -> Result<String, String> {
    let scratch = std::env::temp_dir().join(format!("ade-gold-improve-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
    let wins = guided::mark_win(&scratch, GuidedWinId::ImproveAde).map_err(|e| e.to_string())?;
    let _ = fs::remove_dir_all(&scratch);
    if !wins.improve_ade {
        return Err("improve win not marked".into());
    }
    Ok("guided improve win persisted".into())
}

fn probe_guided_load() -> Result<String, String> {
    let scratch = std::env::temp_dir().join(format!("ade-gold-load-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
    let wins = guided::load_wins(&scratch).map_err(|e| e.to_string())?;
    let _ = fs::remove_dir_all(&scratch);
    if wins.completed_count() != 0 {
        return Err("expected empty wins".into());
    }
    Ok("empty guided wins".into())
}

fn probe_context_total() -> Result<String, String> {
    let budget = ContextBudget::default_daily();
    let total = budget.total_prompt_allowance();
    if total == 0 {
        return Err("total allowance zero".into());
    }
    Ok(format!("total_prompt_allowance={total}"))
}

fn probe_start_prompt_len() -> Result<String, String> {
    let text = StartPromptBuilder::new().build();
    if text.len() < 40 {
        return Err("T0 too short".into());
    }
    Ok(format!("T0 len={}", text.len()))
}

fn probe_agents_md_body(root: &Path) -> Result<String, String> {
    let raw = fs::read_to_string(root.join("AGENTS.md")).map_err(|e| e.to_string())?;
    if raw.trim().is_empty() {
        return Err("AGENTS.md empty".into());
    }
    Ok(format!("AGENTS.md {} chars", raw.len()))
}

fn probe_handoff_summary() -> Result<String, String> {
    let capsule = HandoffCapsule::new("gold probe", "agent");
    let summary = capsule.prompt_summary(200);
    if summary.is_empty() {
        return Err("empty handoff summary".into());
    }
    Ok("handoff summary ok".into())
}

fn probe_observe_mutating_flag() -> Result<String, String> {
    if AutonomyLevel::Observe.allows_mutating_tools() {
        return Err("Observe allows mutating".into());
    }
    Ok("Observe no mutating".into())
}

fn probe_propose_mutating_flag() -> Result<String, String> {
    if AutonomyLevel::Propose.allows_mutating_tools() {
        return Err("Propose allows mutating".into());
    }
    Ok("Propose no mutating".into())
}

fn probe_act_mutating_flag() -> Result<String, String> {
    if !AutonomyLevel::Act.allows_mutating_tools() {
        return Err("Act blocks mutating".into());
    }
    Ok("Act mutating ok".into())
}

fn probe_automate_mutating_flag() -> Result<String, String> {
    if !AutonomyLevel::Automate.allows_mutating_tools() {
        return Err("Automate blocks mutating".into());
    }
    Ok("Automate mutating ok".into())
}

fn probe_gate_id_g0() -> Result<String, String> {
    if VerifyGate::G0.id() != "G0" {
        return Err("G0 id mismatch".into());
    }
    Ok("G0 id".into())
}

fn probe_money_nan() -> Result<String, String> {
    match Money::try_from_usd_f64(f64::NAN) {
        Err(_) => Ok("rejected NaN".into()),
        Ok(_) => Err("accepted NaN".into()),
    }
}

fn probe_git_push_effect() -> Result<String, String> {
    expect_effect("git", "push", ToolEffect::ExternalWrite)
}

fn probe_catalog_mentions(root: &Path) -> Result<String, String> {
    let skills = SkillLoader::new(root)
        .load_all()
        .map_err(|e| e.to_string())?;
    let name = skills
        .first()
        .map(|s| s.name.clone())
        .ok_or_else(|| "no skills".to_string())?;
    let prompt = SkillLoader::new(root)
        .prompt_context("hello", 4_000)
        .map_err(|e| e.to_string())?;
    if !prompt.contains(&name) {
        return Err(format!("catalog missing {name}"));
    }
    Ok(format!("catalog mentions {name}"))
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
            report.total >= 40,
            "expected ≥40 gold tasks, got {}",
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
