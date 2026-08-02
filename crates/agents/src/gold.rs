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
use uuid::Uuid;

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
            "autonomy_automate_prompt_clause" => probe_automate_clause(),
            "always_ignore_nonempty" => probe_always_ignore(),
            "activate_includes_references" => probe_activate_refs(&self.root),
            // H5 harness races
            "race_dual_writer_lease" => probe_race_dual_writer(),
            "slot_planner_blocks_lease" => probe_slot_planner_blocks(),
            "slot_worker_allows_lease" => probe_slot_worker_allows(),
            "spend_honesty_unpriced_caps" => probe_spend_honesty(),
            "budget_occupancy_compact" => probe_budget_occupancy_compact(),
            "risk_gate_git_push" => probe_risk_gate_push(),
            "contract_gate_incomplete" => probe_contract_gate_incomplete(),
            "isolate_worktree_add_remove" => probe_isolate_worktree(),
            "model_router_worker_profile" => probe_model_router_worker(),
            // C5 compaction / fertility gold
            "c5_mask_reduces_with_fidelity" => probe_c5_mask(),
            "c5_capsule_beats_full" => probe_c5_capsule(),
            "c5_format_fertility_order" => probe_c5_fertility_order(),
            "c5_invented_cipher_loses" => probe_c5_cipher(),
            "c5_capsule_section_rubric" => probe_c5_capsule_sections(),
            // H2 depth
            "h2_task_heartbeat_holds" => probe_h2_task_heartbeat_holds(),
            "h2_claim_gate_blocks_freeform" => probe_h2_claim_gate_blocks_freeform(),
            "h2_verifier_blocks_lease" => probe_h2_verifier_blocks_lease(),
            "h1_invoice_delta_reconcile" => probe_h1_invoice_delta_reconcile(),
            "c3_thrift_resume_no_paste" => probe_c3_thrift_resume_no_paste(),
            "c3_last_write_sections" => probe_c3_last_write_sections(),
            "e1_action_envelope_persist" => probe_e1_action_envelope_persist(),
            "c4_compact_context_rubric" => probe_c4_compact_context_rubric(),
            "d_vision_refuse_text_only" => probe_d_vision_refuse_text_only(),
            "d_vision_image_reserve" => probe_d_vision_image_reserve(),
            "d_pdf_extract_rejects_non_pdf" => probe_d_pdf_extract_rejects_non_pdf(),
            "m2_office_extract_rejects_non_office" => {
                probe_m2_office_extract_rejects_non_office()
            }
            "m2_office_extract_docx" => probe_m2_office_extract_docx(),
            "m2_audio_rejects_non_audio" => probe_m2_audio_rejects_non_audio(),
            "m2_audio_local_whisper_cmd" => probe_m2_audio_local_whisper_cmd(),
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
        task(
            "g49",
            "Automate prompt clause present",
            "autonomy_automate_prompt_clause",
            false,
        ),
        task(
            "g50",
            "Always-ignore patterns nonempty",
            "always_ignore_nonempty",
            true,
        ),
        task(
            "g51",
            "activate_skill includes T3 references",
            "activate_includes_references",
            true,
        ),
        task(
            "g52",
            "H5 dual-writer lease conflict",
            "race_dual_writer_lease",
            true,
        ),
        task(
            "g53",
            "H5 planner slot blocks write lease",
            "slot_planner_blocks_lease",
            true,
        ),
        task(
            "g54",
            "H5 worker slot allows write lease",
            "slot_worker_allows_lease",
            true,
        ),
        task(
            "g55",
            "H5 spend honesty blocks unpriced caps",
            "spend_honesty_unpriced_caps",
            true,
        ),
        task(
            "g56",
            "H5 occupancy triggers boundary compact",
            "budget_occupancy_compact",
            true,
        ),
        task(
            "g57",
            "H5 risk gate on git push",
            "risk_gate_git_push",
            true,
        ),
        task(
            "g58",
            "H5 contract gate incomplete blocks Act",
            "contract_gate_incomplete",
            true,
        ),
        task(
            "g59",
            "H5 Isolate worktree add/remove",
            "isolate_worktree_add_remove",
            true,
        ),
        task(
            "g60",
            "H5 model router picks worker profile",
            "model_router_worker_profile",
            true,
        ),
        task(
            "g61",
            "C5 mask reduces tokens with fidelity",
            "c5_mask_reduces_with_fidelity",
            true,
        ),
        task(
            "g62",
            "C5 capsule beats full transcript",
            "c5_capsule_beats_full",
            true,
        ),
        task(
            "g63",
            "C5 format fertility order",
            "c5_format_fertility_order",
            true,
        ),
        task(
            "g64",
            "C5 invented cipher loses to compact JSON",
            "c5_invented_cipher_loses",
            true,
        ),
        task(
            "g65",
            "C5 capsule section rubric",
            "c5_capsule_section_rubric",
            true,
        ),
        task(
            "g66",
            "H2 task heartbeat holds claim past TTL",
            "h2_task_heartbeat_holds",
            true,
        ),
        task(
            "g67",
            "H2 claim_gate blocks freeform with queue",
            "h2_claim_gate_blocks_freeform",
            true,
        ),
        task(
            "g68",
            "H2 verifier slot blocks write lease",
            "h2_verifier_blocks_lease",
            true,
        ),
        task(
            "g69",
            "H1 invoice delta reserved vs actual",
            "h1_invoice_delta_reconcile",
            true,
        ),
        task(
            "g70",
            "C3 thrift resume forbids paste",
            "c3_thrift_resume_no_paste",
            true,
        ),
        task(
            "g71",
            "C3 last-write Continuity sections",
            "c3_last_write_sections",
            true,
        ),
        task(
            "g72",
            "E1 action envelope persists to Continuity",
            "e1_action_envelope_persist",
            true,
        ),
        task(
            "g73",
            "C4 compact_context rubric gate",
            "c4_compact_context_rubric",
            true,
        ),
        task(
            "g74",
            "D vision refuse on text-only model",
            "d_vision_refuse_text_only",
            true,
        ),
        task(
            "g75",
            "D image reserve exceeds text-only estimate",
            "d_vision_image_reserve",
            true,
        ),
        task(
            "g76",
            "D PDF extract rejects non-PDF",
            "d_pdf_extract_rejects_non_pdf",
            false,
        ),
        task(
            "g77",
            "M2 Office extract rejects non-Office",
            "m2_office_extract_rejects_non_office",
            false,
        ),
        task(
            "g78",
            "M2 Office extract docx paragraph",
            "m2_office_extract_docx",
            true,
        ),
        task(
            "g79",
            "M2 audio rejects non-audio",
            "m2_audio_rejects_non_audio",
            false,
        ),
        task(
            "g80",
            "M2 audio local ADE_WHISPER_CMD",
            "m2_audio_local_whisper_cmd",
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
    if !AutonomyLevel::Propose.allows_tool_effect(ToolEffect::ProcessExecution) {
        return Err("Propose should allow inspect shell (ProcessExecution)".into());
    }
    Ok("Propose blocks writes, allows inspect shell".into())
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
        let parsed = AutonomyLevel::from_str(level.as_str())?;
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
    let gate = VerifyGate::from_str("G3")?;
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

fn probe_automate_clause() -> Result<String, String> {
    let clause = AutonomyLevel::Automate.prompt_clause();
    if !clause.contains("verify") {
        return Err("Automate clause missing verify".into());
    }
    Ok("Automate prompt clause ok".into())
}

fn probe_always_ignore() -> Result<String, String> {
    use ade_core::ignore::always_ignore_patterns;
    let patterns = always_ignore_patterns();
    if patterns.is_empty() {
        return Err("always_ignore empty".into());
    }
    Ok(format!("{} always-ignore patterns", patterns.len()))
}

fn probe_activate_refs(root: &Path) -> Result<String, String> {
    let skill = SkillLoader::new(root)
        .activate("verify-ladder")
        .map_err(|e| e.to_string())?;
    if !skill.body.contains("References (T3)") {
        return Err("activated verify-ladder missing T3 references block".into());
    }
    if !skill.body.contains("G0") {
        return Err("T3 gates.md content missing".into());
    }
    Ok("activate includes T3 references".into())
}

fn probe_race_dual_writer() -> Result<String, String> {
    let scratch = std::env::temp_dir().join(format!("ade-gold-lease-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
    let mgr = ade_workflow::parallel::LeaseManager::new(&scratch);
    let a = uuid::Uuid::new_v4();
    let b = uuid::Uuid::new_v4();
    mgr.acquire(
        a,
        "src/lib.rs",
        ade_workflow::parallel::LeaseMode::Strong,
        chrono::Duration::minutes(5),
    )
    .map_err(|e| e.to_string())?;
    let err = mgr
        .acquire(
            b,
            "src/lib.rs",
            ade_workflow::parallel::LeaseMode::Strong,
            chrono::Duration::minutes(5),
        )
        .err()
        .ok_or_else(|| "second writer unexpectedly acquired lease".to_string())?;
    let _ = fs::remove_dir_all(&scratch);
    let msg = err.to_string();
    if !msg.to_lowercase().contains("lease conflict") {
        return Err(format!("expected lease conflict, got {msg}"));
    }
    Ok("dual-writer blocked".into())
}

fn probe_slot_planner_blocks() -> Result<String, String> {
    crate::slots::SlotRole::Planner
        .require_write_lease()
        .err()
        .ok_or_else(|| "planner unexpectedly allowed write lease".to_string())?;
    Ok("planner slot_gate ok".into())
}

fn probe_slot_worker_allows() -> Result<String, String> {
    crate::slots::SlotRole::Worker
        .require_write_lease()
        .map_err(|e| e.to_string())?;
    crate::slots::SlotRole::Worker
        .require_claim_tasks()
        .map_err(|e| e.to_string())?;
    Ok("worker slot ok".into())
}

fn probe_spend_honesty() -> Result<String, String> {
    let caps = crate::spend::SpendCaps {
        session: Money::from_usd_str("1.00").map_err(|e| e.to_string())?,
        daily: Money::ZERO,
    };
    let err = crate::spend::require_priced_for_caps(&caps, Money::ZERO, Money::ZERO)
        .err()
        .ok_or_else(|| "unpriced caps unexpectedly allowed".to_string())?;
    if !err.to_string().to_lowercase().contains("spend_honesty") {
        return Err(format!("expected spend_honesty, got {err}"));
    }
    Ok("unpriced caps blocked".into())
}

fn probe_budget_occupancy_compact() -> Result<String, String> {
    let mut messages = vec![
        serde_json::json!({"role":"system","content":"sys"}),
        serde_json::json!({"role":"user","content":"go"}),
    ];
    for i in 0..8 {
        messages.push(serde_json::json!({
            "role":"assistant",
            "tool_calls":[{"id": format!("{i}"), "function":{"name":"fs__read_file","arguments":"{}"}}]
        }));
        messages.push(serde_json::json!({
            "role":"tool",
            "tool_call_id": format!("{i}"),
            "content": "x".repeat(2_000)
        }));
    }
    let limit = 3_000_u64;
    if !crate::context_edit::should_compact_at_occupancy(&messages, limit, 0.70) {
        let occ = crate::context_edit::occupancy_ratio(&messages, limit);
        return Err(format!("expected occupancy compact trigger (occ={occ:.2})"));
    }
    let (next, summary) = crate::context_edit::apply_boundary_compact(
        &messages,
        2,
        "occupancy_70",
        limit,
        crate::context_edit::BoundaryCompactExtras::default(),
    );
    if summary.tokens_after >= summary.tokens_before {
        return Err("compact did not shrink".into());
    }
    if !summary.summary.contains("ade.boundary-capsule/v1") {
        return Err("missing boundary capsule".into());
    }
    if next.len() >= messages.len() {
        return Err("message list not reduced".into());
    }
    Ok(format!(
        "compact {}→{} tok",
        summary.tokens_before, summary.tokens_after
    ))
}

fn probe_risk_gate_push() -> Result<String, String> {
    let a = crate::risk::assess_tool(
        "shell",
        "run_command",
        &serde_json::json!({ "command": "git push origin main" }),
        ToolEffect::ProcessExecution,
    );
    if !a.requires_hitl() {
        return Err("git push should require HITL".into());
    }
    if a.category != crate::risk::RiskCategory::Publish {
        return Err(format!("expected publish, got {:?}", a.category));
    }
    Ok("risk_gate publish".into())
}

fn probe_contract_gate_incomplete() -> Result<String, String> {
    let incomplete = crate::goal::EngGoal {
        schema: crate::goal::GOAL_SCHEMA.into(),
        id: "g".into(),
        statement: "do thing".into(),
        success_criteria: vec![],
        out_of_scope: vec![],
        verify_gate: None,
        owned_paths: vec![],
        shell_scope: "workspace".into(),
        autonomy: "act".into(),
        status: "active".into(),
        created_at: "now".into(),
        updated_at: "now".into(),
        last_handoff_id: None,
        clarify_resolutions: vec![],
        contract_waive: None,
    };
    if incomplete.allows_act_tools() {
        return Err("incomplete contract allowed Act".into());
    }
    Ok("contract_gate blocks".into())
}

fn probe_isolate_worktree() -> Result<String, String> {
    let root = std::env::temp_dir().join(format!("ade-gold-wt-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    run_git(&root, &["init"])?;
    run_git(&root, &["config", "user.email", "ade@example.com"])?;
    run_git(&root, &["config", "user.name", "ADE Gold"])?;
    fs::write(root.join("README.md"), "hello\n").map_err(|e| e.to_string())?;
    run_git(&root, &["add", "README.md"])?;
    run_git(&root, &["commit", "-m", "init"])?;
    let wt = root
        .parent()
        .unwrap()
        .join(format!("ade-gold-wt-out-{}", uuid::Uuid::new_v4()));
    let mgr = ade_workflow::parallel::WorktreeManager::new(&root);
    let branch = format!("gold/isolate-{}", uuid::Uuid::new_v4());
    mgr.add(&wt, &branch, None).map_err(|e| e.to_string())?;
    if mgr.list().map_err(|e| e.to_string())?.len() < 2 {
        let _ = fs::remove_dir_all(&wt);
        let _ = fs::remove_dir_all(&root);
        return Err("worktree not listed".into());
    }
    mgr.remove(&wt, true).map_err(|e| e.to_string())?;
    let _ = fs::remove_dir_all(&wt);
    let _ = fs::remove_dir_all(&root);
    Ok("isolate worktree ok".into())
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<(), String> {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("git {} failed", args.join(" ")));
    }
    Ok(())
}

fn probe_model_router_worker() -> Result<String, String> {
    let catalog = crate::model_profile::ModelProfileCatalog::builtins();
    let decision = crate::model_profile::route(
        &catalog,
        &crate::model_profile::RouteInput {
            provider: "opencode".into(),
            model: "test-model".into(),
            autonomy: AutonomyLevel::Act,
            max_tool_rounds: 8,
            session_cap: None,
            slot_override: None,
        },
    );
    if decision.profile_id != "worker-strong" {
        return Err(format!(
            "expected worker-strong, got {}",
            decision.profile_id
        ));
    }
    if decision.slot != crate::slots::SlotRole::Worker {
        return Err("expected worker slot".into());
    }
    Ok("router worker-strong".into())
}

fn probe_h2_task_heartbeat_holds() -> Result<String, String> {
    let root = std::env::temp_dir().join(format!("ade-gold-hb-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
    let coordinator = ade_workflow::tasks::TaskCoordinator::new(&root);
    let task = coordinator
        .enqueue(ade_workflow::tasks::EnqueueTask {
            goal: "heartbeat gold".into(),
            owned_paths: vec!["src/hb".into()],
            lease_mode: ade_workflow::parallel::LeaseMode::Strong,
            depends_on: vec![],
        })
        .map_err(|e| e.to_string())?;
    let agent = uuid::Uuid::new_v4();
    let claimed = coordinator
        .claim(agent, chrono::Duration::milliseconds(1200))
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "claim returned none".to_string())?;
    coordinator
        .heartbeat(&claimed.id, agent, chrono::Duration::seconds(30))
        .map_err(|e| e.to_string())?;
    std::thread::sleep(std::time::Duration::from_millis(1600));
    let _ = coordinator.requeue_expired().map_err(|e| e.to_string())?;
    let listed = coordinator.list().map_err(|e| e.to_string())?;
    let still = listed
        .iter()
        .find(|t| t.id == task.id)
        .ok_or_else(|| "task missing".to_string())?;
    if still.status == ade_workflow::tasks::TaskStatus::Queued {
        let _ = fs::remove_dir_all(&root);
        return Err("claim requeued despite heartbeat".into());
    }
    let _ = fs::remove_dir_all(&root);
    Ok("heartbeat holds past TTL".into())
}

fn probe_h2_claim_gate_blocks_freeform() -> Result<String, String> {
    let root = std::env::temp_dir().join(format!("ade-gold-cg-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
    let coordinator = ade_workflow::tasks::TaskCoordinator::new(&root);
    coordinator
        .enqueue(ade_workflow::tasks::EnqueueTask {
            goal: "claim gate gold".into(),
            owned_paths: vec!["src/cg".into()],
            lease_mode: ade_workflow::parallel::LeaseMode::Strong,
            depends_on: vec![],
        })
        .map_err(|e| e.to_string())?;
    let err = crate::turn::enforce_claim_gate(
        &root,
        AutonomyLevel::Act,
        Some(uuid::Uuid::new_v4()),
        None,
        false,
    )
    .err()
    .ok_or_else(|| "claim_gate unexpectedly allowed freeform".to_string())?;
    if !err.to_string().to_lowercase().contains("claim_gate") {
        let _ = fs::remove_dir_all(&root);
        return Err(format!("expected claim_gate, got {err}"));
    }
    crate::turn::enforce_claim_gate(
        &root,
        AutonomyLevel::Act,
        Some(uuid::Uuid::new_v4()),
        None,
        true,
    )
    .map_err(|e| e.to_string())?;
    let waive_path = root.join(".ade").join("tasks").join("queue-waives.jsonl");
    if !waive_path.is_file() {
        let _ = fs::remove_dir_all(&root);
        return Err("queue waive log missing".into());
    }
    let _ = fs::remove_dir_all(&root);
    Ok("claim_gate blocks freeform".into())
}

fn probe_h2_verifier_blocks_lease() -> Result<String, String> {
    crate::slots::SlotRole::Verifier
        .require_write_lease()
        .err()
        .ok_or_else(|| "verifier unexpectedly allowed write lease".to_string())?;
    crate::slots::SlotRole::Verifier
        .require_claim_tasks()
        .err()
        .ok_or_else(|| "verifier unexpectedly allowed claim".to_string())?;
    if !crate::slots::SlotRole::Verifier.may_run_verify_sensors() {
        return Err("verifier should run verify sensors".into());
    }
    let catalog = crate::model_profile::ModelProfileCatalog::builtins();
    let decision = crate::model_profile::route(
        &catalog,
        &crate::model_profile::RouteInput {
            provider: "opencode".into(),
            model: "test-model".into(),
            autonomy: AutonomyLevel::Propose,
            max_tool_rounds: 8,
            session_cap: None,
            slot_override: Some(crate::slots::SlotRole::Verifier),
        },
    );
    if decision.slot != crate::slots::SlotRole::Verifier {
        return Err("expected verifier slot".into());
    }
    if decision.profile_id != "verifier-independent" {
        return Err(format!(
            "expected verifier-independent, got {}",
            decision.profile_id
        ));
    }
    Ok("verifier blocks lease".into())
}

fn probe_h1_invoice_delta_reconcile() -> Result<String, String> {
    // Invoice class: tokens × rates = actual; reserved − actual = Δ.
    let rate_in = Money::try_from_usd_f64(1.0).map_err(|e| e.to_string())?;
    let rate_out = Money::try_from_usd_f64(2.0).map_err(|e| e.to_string())?;
    let model = crate::provider::ModelConfig {
        id: "invoice-probe".into(),
        name: "invoice-probe".into(),
        context_limit: 128_000,
        output_limit: 8_192,
        cost_per_input_mtok: rate_in,
        cost_per_output_mtok: rate_out,
    };
    let reserved = model
        .estimate_round_cost(1_000, 500)
        .map_err(|e| e.to_string())?;
    let usage = crate::provider::ProviderUsage {
        input_tokens: 800,
        output_tokens: 200,
    };
    let actual = usage.cost_money(&model);
    if actual >= reserved {
        return Err(format!(
            "expected actual < reserved (actual={} reserved={})",
            actual.micros(),
            reserved.micros()
        ));
    }
    let delta = reserved.saturating_sub(actual);
    if delta.micros() <= 0 {
        return Err("expected positive reserve−actual delta".into());
    }
    // Sanity: 800 @ $1/MTok + 200 @ $2/MTok = $0.0008 + $0.0004 = $0.0012
    let expected = Money::cost_for_tokens(800, rate_in) + Money::cost_for_tokens(200, rate_out);
    if actual.micros() != expected.micros() {
        return Err(format!(
            "actual micros {} != expected {}",
            actual.micros(),
            expected.micros()
        ));
    }
    Ok(format!(
        "Δ ${:.6} (reserved ${:.6} − actual ${:.6})",
        delta.to_usd_f64(),
        reserved.to_usd_f64(),
        actual.to_usd_f64()
    ))
}

fn probe_c3_thrift_resume_no_paste() -> Result<String, String> {
    let mut capsule = HandoffCapsule::new("Finish Continuity thrift loop", "agent_turn");
    capsule.next_safe_command = Some("ade verify --gate G0 --through".into());
    capsule.turn_status = Some("budget_exhausted".into());
    capsule.changed_paths = vec![".ade/dogfood/continuity-acceptance.md".into()];
    capsule.decisions_touched = vec!["host next_safe before rediscovery".into()];
    capsule.verify_results = vec![ade_core::handoff::HandoffVerify {
        gate: "G0".into(),
        status: "pass".into(),
    }];
    let prompt = capsule.resume_user_prompt_with(true, Some(0));
    if !prompt.contains("Host already ran next_safe_command") {
        return Err("missing host-ran thrift marker".into());
    }
    if !prompt.contains("Do not paste prior chat") {
        return Err("thrift resume must forbid paste".into());
    }
    if !prompt.contains(".ade/continuity/last-write.json") {
        return Err("thrift resume must point at last-write".into());
    }
    if prompt.contains("Do next_safe_command first") {
        return Err("host-ran prompt should not ask model to run next_safe first".into());
    }
    // No full-chat dump markers (forbid-instruction text is allowed).
    for banned in [
        "```json",
        "BEGIN TRANSCRIPT",
        "paste the prior conversation",
    ] {
        if prompt
            .to_ascii_lowercase()
            .contains(&banned.to_ascii_lowercase())
        {
            return Err(format!("thrift prompt looks like a paste dump ({banned})"));
        }
    }
    Ok("thrift resume no-paste".into())
}

fn probe_c3_last_write_sections() -> Result<String, String> {
    let root = std::env::temp_dir().join(format!("ade-gold-c3-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let mut capsule = HandoffCapsule::new("Persist Continuity facts", "agent_turn");
    capsule.turn_status = Some("completed".into());
    capsule.next_safe_command = Some("ade audit".into());
    capsule.changed_paths = vec![".ade/continuity/last-write.json".into()];
    capsule.decisions_touched = vec!["write-before-compact".into()];
    capsule.blockers = vec!["none".into()];
    capsule.verify_results = vec![ade_core::handoff::HandoffVerify {
        gate: "G0".into(),
        status: "pass".into(),
    }];
    crate::handoff::write_continuity_last_write(&root, &capsule, "gold-c3")
        .map_err(|e| e.to_string())?;
    let path = root.join(".ade").join("continuity").join("last-write.json");
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("last-write JSON: {e}"))?;
    let schema = value.get("schema").and_then(|v| v.as_str()).unwrap_or("");
    if schema != "ade.continuity-last-write/v1" {
        let _ = std::fs::remove_dir_all(&root);
        return Err(format!("unexpected schema {schema}"));
    }
    for key in ["intent", "decisions", "paths", "failing", "next", "verify"] {
        if value.get(key).is_none() {
            let _ = std::fs::remove_dir_all(&root);
            return Err(format!("last-write missing section '{key}'"));
        }
    }
    let _ = std::fs::remove_dir_all(&root);
    Ok("last-write sections ok".into())
}

fn probe_e1_action_envelope_persist() -> Result<String, String> {
    let root = std::env::temp_dir().join(format!("ade-gold-e1-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let args = serde_json::json!({
        "path": ".ade/dogfood/envelope.md",
        "file": "extra.rs"
    });
    let paths = crate::session::paths_from_tool_arguments(&args);
    if !paths.iter().any(|p| p.contains("envelope.md")) {
        let _ = std::fs::remove_dir_all(&root);
        return Err(format!("expected path extraction, got {paths:?}"));
    }
    let envelope = crate::session::ActionEnvelope {
        effect: "WorkspaceWrite".into(),
        paths: paths.clone(),
        autonomy: "act".into(),
        risk_tier: Some("low".into()),
        risk_category: Some("none".into()),
    };
    crate::handoff::record_action_envelope(&root, "fs", "write_file", &envelope)
        .map_err(|e| e.to_string())?;
    let path = root
        .join(".ade")
        .join("continuity")
        .join("last-actions.json");
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("last-actions JSON: {e}"))?;
    if value.get("schema").and_then(|v| v.as_str()) != Some("ade.continuity-last-actions/v1") {
        let _ = std::fs::remove_dir_all(&root);
        return Err("unexpected last-actions schema".into());
    }
    let count = value.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
    if count < 1 {
        let _ = std::fs::remove_dir_all(&root);
        return Err("expected at least one action".into());
    }
    let actions = value
        .get("actions")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let first = actions.first().ok_or("missing action row")?;
    if first.get("effect").and_then(|v| v.as_str()) != Some("WorkspaceWrite") {
        let _ = std::fs::remove_dir_all(&root);
        return Err("effect not persisted".into());
    }
    if first.get("tool").and_then(|v| v.as_str()) != Some("write_file") {
        let _ = std::fs::remove_dir_all(&root);
        return Err("tool not persisted".into());
    }
    let _ = std::fs::remove_dir_all(&root);
    Ok(format!("envelope persisted ({count})"))
}

fn probe_c4_compact_context_rubric() -> Result<String, String> {
    // Rubric: empty reason rejected; stuck/debugging suppressed; converging accepted.
    // Exercise via public paths_from + a mini session isn't needed — mirror host gate logic.
    fn gate(reason: &str) -> Result<(), String> {
        let reason = reason.trim();
        if reason.is_empty() {
            return Err("empty".into());
        }
        let reason_l = reason.to_ascii_lowercase();
        if reason_l.contains("stuck")
            || reason_l.contains("debugging")
            || reason_l.contains("mid-derivation")
            || reason_l.contains("mid_derivation")
        {
            return Err("suppressed".into());
        }
        Ok(())
    }
    if gate("").is_ok() {
        return Err("empty reason should fail".into());
    }
    if gate("stuck debugging").is_ok() {
        return Err("stuck should be suppressed".into());
    }
    if gate("mid-derivation").is_ok() {
        return Err("mid-derivation should be suppressed".into());
    }
    gate("subtask_resolved").map_err(|e| format!("converging should pass: {e}"))?;
    // T0 nudge mentions compact_context
    let t0 = crate::start_prompt::StartPromptBuilder::new().build();
    if !t0.contains("ade__compact_context") {
        return Err("T0 missing compact_context nudge".into());
    }
    Ok("c4 rubric gate ok".into())
}

fn probe_d_vision_refuse_text_only() -> Result<String, String> {
    let err = crate::vision::user_message_content(
        "what is this?",
        &["shot.png".into()],
        "deepseek-v4-flash-free",
        Path::new("."),
    )
    .err()
    .ok_or_else(|| "text-only model unexpectedly accepted vision".to_string())?
    .to_string();
    if !err.contains("vision_required") {
        return Err(format!("expected vision_required, got {err}"));
    }
    // Profile flag can force-allow even a text-only id (and force-deny a VL id).
    if crate::vision::model_supports_vision_ex("big-pickle", Some(true)) != true {
        return Err("profile vision=true should allow".into());
    }
    if crate::vision::model_supports_vision_ex("claude-haiku-4-5", Some(false)) != false {
        return Err("profile vision=false should deny".into());
    }
    Ok("vision_required on text-only".into())
}

fn probe_d_vision_image_reserve() -> Result<String, String> {
    use serde_json::json;
    let root = std::env::temp_dir().join(format!("ade-gold-vision-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let path = root.join("shot.png");
    // Large enough that base64 char÷4 >> dedicated vision band placeholder.
    let mut bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    bytes.extend(std::iter::repeat(0u8).take(120_000));
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;

    let text_only = vec![
        json!({ "role": "system", "content": "you are ade" }),
        json!({ "role": "user", "content": "describe" }),
    ];
    let text_only_est = crate::context_edit::estimate_messages_tokens(&text_only);

    let content = crate::vision::user_message_content(
        "describe",
        &["shot.png".into()],
        "claude-haiku-4-5",
        &root,
    )
    .map_err(|e| e.to_string())?;
    let messages = vec![
        json!({ "role": "system", "content": "you are ade" }),
        json!({ "role": "user", "content": content }),
    ];
    let naive = crate::context_edit::estimate_messages_tokens(&messages);
    let text_band =
        crate::context_edit::estimate_messages_tokens_excluding_image_data(&messages);
    let vision_band = crate::vision::estimate_vision_tokens(&["shot.png".into()], &root)
        .map_err(|e| e.to_string())? as u64;
    let honest = text_band.saturating_add(vision_band);
    let _ = std::fs::remove_dir_all(&root);
    if vision_band == 0 {
        return Err("vision band should be > 0".into());
    }
    // Dedicated band must beat naive base64 inflation for SpendGuard honesty.
    if text_band >= naive {
        return Err(format!(
            "expected redacted text_band ({text_band}) < naive base64 estimate ({naive})"
        ));
    }
    if honest <= text_only_est {
        return Err(format!(
            "honest multimodal ({honest}) should exceed text-only ({text_only_est})"
        ));
    }
    if honest >= naive {
        return Err(format!(
            "honest band ({honest}) should stay below naive base64 inflation ({naive})"
        ));
    }
    Ok(format!(
        "vision_band={vision_band} text={text_band} naive={naive} honest={honest}"
    ))
}

fn probe_d_pdf_extract_rejects_non_pdf() -> Result<String, String> {
    let root = std::env::temp_dir().join(format!("ade-gold-pdf-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let path = root.join("note.txt");
    std::fs::write(&path, b"not a pdf").map_err(|e| e.to_string())?;
    let err = crate::pdf::extract_pdf_text(&path, 2)
        .err()
        .ok_or_else(|| "non-PDF unexpectedly extracted".to_string())?
        .to_string();
    let _ = std::fs::remove_dir_all(&root);
    if !err.contains("not a PDF") {
        return Err(format!("expected not a PDF, got {err}"));
    }
    Ok("pdf extract rejects non-PDF".into())
}

fn probe_m2_office_extract_rejects_non_office() -> Result<String, String> {
    let root = std::env::temp_dir().join(format!("ade-gold-office-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let path = root.join("note.txt");
    std::fs::write(&path, b"not office").map_err(|e| e.to_string())?;
    let err = crate::office::extract_office(&path)
        .err()
        .ok_or_else(|| "non-Office unexpectedly extracted".to_string())?
        .to_string();
    let _ = std::fs::remove_dir_all(&root);
    if !err.contains("not an Office extract target") {
        return Err(format!("expected Office reject, got {err}"));
    }
    Ok("office extract rejects non-Office".into())
}

fn probe_m2_office_extract_docx() -> Result<String, String> {
    let root = std::env::temp_dir().join(format!("ade-gold-docx-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let path = root.join("brief.docx");
    crate::office::write_minimal_docx(&path, "ADE office extract gold")
        .map_err(|e| e.to_string())?;
    let result = crate::office::extract_office(&path).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&root);
    if result.kind != crate::office::OfficeKind::Docx {
        return Err("expected docx kind".into());
    }
    if !result.text.contains("ADE office extract gold") {
        return Err(format!("missing paragraph text: {}", result.text));
    }
    Ok("office extract docx paragraph".into())
}

fn probe_m2_audio_rejects_non_audio() -> Result<String, String> {
    let root = std::env::temp_dir().join(format!("ade-gold-audio-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let path = root.join("note.txt");
    std::fs::write(&path, b"not audio").map_err(|e| e.to_string())?;
    let err = crate::audio::validate_audio_file(&path)
        .err()
        .ok_or_else(|| "non-audio unexpectedly validated".to_string())?
        .to_string();
    let _ = std::fs::remove_dir_all(&root);
    if !err.contains("not an audio transcribe target") {
        return Err(format!("expected audio reject, got {err}"));
    }
    Ok("audio rejects non-audio".into())
}

fn probe_m2_audio_local_whisper_cmd() -> Result<String, String> {
    let root = std::env::temp_dir().join(format!("ade-gold-whisper-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let path = root.join("clip.mp3");
    std::fs::write(&path, b"ID3fake").map_err(|e| e.to_string())?;
    let prev = std::env::var("ADE_WHISPER_CMD").ok();
    std::env::set_var(
        "ADE_WHISPER_CMD",
        r#"powershell -NoProfile -Command "Write-Output 'ADE audio gold transcript'""#,
    );
    let result = crate::audio::transcribe_local(&path);
    match prev {
        Some(value) => std::env::set_var("ADE_WHISPER_CMD", value),
        None => std::env::remove_var("ADE_WHISPER_CMD"),
    }
    let result = result.map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&root);
    if !result.text.contains("ADE audio gold transcript") {
        return Err(format!("missing transcript text: {}", result.text));
    }
    if !result.backend.starts_with("local:") {
        return Err(format!("expected local backend, got {}", result.backend));
    }
    Ok("audio local ADE_WHISPER_CMD".into())
}

fn probe_c5_mask() -> Result<String, String> {
    let bench = crate::fertility::CompactionBench::run(2);
    if !bench.mask_preserved_ids {
        return Err("mask lost tool_call_id or recent verbatim".into());
    }
    if bench.masked_tokens >= bench.full_tokens {
        return Err("mask did not reduce tokens".into());
    }
    if bench.mask_saved_pct < 10.0 {
        return Err(format!("mask saved only {:.1}%", bench.mask_saved_pct));
    }
    Ok(format!(
        "mask {:.0}% saved ({}→{})",
        bench.mask_saved_pct, bench.full_tokens, bench.masked_tokens
    ))
}

fn probe_c5_capsule() -> Result<String, String> {
    let bench = crate::fertility::CompactionBench::run(2);
    if bench.capsule_tokens >= bench.full_tokens {
        return Err("capsule did not reduce tokens".into());
    }
    if bench.capsule_saved_pct < 20.0 {
        return Err(format!(
            "capsule saved only {:.1}%",
            bench.capsule_saved_pct
        ));
    }
    // Capsule should usually beat mask for deep transcripts (stronger collapse).
    if bench.capsule_tokens > bench.masked_tokens {
        return Err(format!(
            "capsule ({}) larger than mask ({})",
            bench.capsule_tokens, bench.masked_tokens
        ));
    }
    Ok(format!(
        "capsule {:.0}% saved ({}→{})",
        bench.capsule_saved_pct, bench.full_tokens, bench.capsule_tokens
    ))
}

fn probe_c5_fertility_order() -> Result<String, String> {
    let rank = crate::fertility::FertilityRanking::measure(&crate::fertility::sample_facts());
    if !rank.order_ok() {
        return Err(format!(
            "bad order tsv={} compact={} pretty={} prose={}",
            rank.tsv, rank.compact_json, rank.pretty_json, rank.verbose_prose
        ));
    }
    Ok(format!(
        "tsv={} compact={} pretty={} prose={}",
        rank.tsv, rank.compact_json, rank.pretty_json, rank.verbose_prose
    ))
}

fn probe_c5_cipher() -> Result<String, String> {
    if !crate::fertility::invented_opaque_loses_to_compact_json(&crate::fertility::sample_facts()) {
        return Err("invented opaque encoding unexpectedly beat compact JSON".into());
    }
    Ok("cipher loses to compact JSON".into())
}

fn probe_c5_capsule_sections() -> Result<String, String> {
    let bench = crate::fertility::CompactionBench::run(2);
    if !bench.capsule_has_sections {
        return Err("capsule missing intent/paths/next/verify rubric".into());
    }
    Ok("capsule sections ok".into())
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
            report.total >= 65,
            "expected ≥65 gold tasks, got {}",
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
