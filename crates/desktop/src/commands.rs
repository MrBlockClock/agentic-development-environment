use ade_agents::mcp::{McpHost, McpServerConfig, McpToolCallResult, McpToolInfo};
use ade_agents::session::AgentEvent;
use ade_core::audit::{AuditMode, AuditReport, AuditRunner};
use ade_core::execute::{ExecuteOptions, ExecuteReport};
use ade_core::plan::{PlanBuilder, PlanReport};
use ade_core::recipe::StackRecipe;
use ade_core::verify::{VerifyGate, VerifyResult};
use ade_workflow::verify::VerifyRunner;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tauri::{ipc::Channel, State};

pub struct AppState {
    workspace_root: RwLock<PathBuf>,
    pub mcp: McpHost,
    pub key_vault: Arc<dyn ade_db::secrets::ProviderKeyVault>,
}

impl AppState {
    pub fn workspace_root(&self) -> PathBuf {
        self.workspace_root
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn set_workspace_root(&self, root: impl AsRef<Path>) -> Result<PathBuf, String> {
        let canonical = root
            .as_ref()
            .canonicalize()
            .map_err(|error| format!("canonicalize workspace: {error}"))?;
        if !canonical.join("AGENTS.md").is_file() || !canonical.join("Cargo.toml").is_file() {
            return Err(
                "workspace must contain AGENTS.md and Cargo.toml (ADE contract root)".into(),
            );
        }
        *self
            .workspace_root
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = canonical.clone();
        persist_preferred_workspace(&canonical)?;
        Ok(canonical)
    }

    pub fn ade_source_root() -> Option<PathBuf> {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .map(PathBuf::from)
            .and_then(|path| path.canonicalize().ok())
            .filter(|path| path.join("AGENTS.md").is_file() && path.join("Cargo.toml").is_file())
    }

    pub fn discover() -> Self {
        let configured = std::env::var("ADE_WORKSPACE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .or_else(load_preferred_workspace);
        let current = std::env::current_dir()
            .ok()
            .filter(|root| root.join("Cargo.toml").is_file() && root.join("AGENTS.md").is_file());
        let source_root = Self::ade_source_root();
        let workspace_root = configured
            .or(current)
            .or(source_root)
            .unwrap_or_else(|| PathBuf::from("."))
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from("."));
        // Load .env so ADE_IMPORT_ENV_KEYS + provider keys are available.
        let _ = ade_core::config::AdeConfig::load();
        let key_vault: Arc<dyn ade_db::secrets::ProviderKeyVault> =
            Arc::new(ade_db::secrets::NativeProviderKeyVault);
        match ade_db::secrets::import_env_provider_keys(key_vault.as_ref(), "local") {
            Ok(imported) if !imported.is_empty() => {
                tracing::info!(
                    providers = ?imported,
                    "imported provider keys from environment into OS vault (values not logged)"
                );
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, "env provider key import skipped");
            }
        }
        Self {
            workspace_root: RwLock::new(workspace_root),
            mcp: McpHost::new(),
            key_vault,
        }
    }
}

fn preferred_workspace_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|base| base.join("ade").join("workspace-root.txt"))
}

fn load_preferred_workspace() -> Option<PathBuf> {
    let path = preferred_workspace_path()?;
    let raw = std::fs::read_to_string(path).ok()?;
    let root = PathBuf::from(raw.trim());
    if root.join("AGENTS.md").is_file() && root.join("Cargo.toml").is_file() {
        Some(root)
    } else {
        None
    }
}

fn persist_preferred_workspace(root: &Path) -> Result<(), String> {
    let path = preferred_workspace_path()
        .ok_or_else(|| "LOCALAPPDATA is not set; cannot persist dogfood workspace".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(&path, root.display().to_string()).map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderKeyStatus {
    pub profile: String,
    pub provider: String,
    pub configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderKeyDeleteResult {
    pub profile: String,
    pub provider: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderKeySmokeResult {
    pub profile: String,
    pub provider: String,
    pub status: String,
    pub detail: String,
}

#[derive(Serialize)]
pub struct DashboardSnapshot {
    pub workspace_root: String,
    pub is_dogfood: bool,
    pub ade_source_root: Option<String>,
    pub audit: AuditReport,
    pub plan: PlanReport,
    pub handoff: ade_agents::handoff::HandoffMetrics,
    pub leases: Vec<ade_workflow::parallel::PathLease>,
    pub tasks: Vec<ade_workflow::tasks::AgentTask>,
    pub rebuild_lock_warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct DogfoodOpenResult {
    pub workspace_root: String,
    pub already_dogfood: bool,
}

#[tauri::command]
pub async fn get_dashboard(state: State<'_, AppState>) -> Result<DashboardSnapshot, String> {
    let workspace_root = state.workspace_root();
    let ade_source = AppState::ade_source_root();
    let is_dogfood = ade_source
        .as_ref()
        .is_some_and(|source| same_path(source, &workspace_root));
    let audit = AuditRunner::new(&workspace_root).run(AuditMode::EvaluateExisting);
    let plan = PlanBuilder::new().build(&audit);
    let handoff = ade_agents::handoff::HandoffManager::new(&workspace_root)
        .metrics()
        .map_err(|error| error.to_string())?;
    let leases = ade_workflow::parallel::LeaseManager::new(&workspace_root)
        .list()
        .map_err(|error| error.to_string())?;
    let tasks = ade_workflow::tasks::TaskCoordinator::new(&workspace_root)
        .list()
        .map_err(|error| error.to_string())?;
    Ok(DashboardSnapshot {
        workspace_root: workspace_root.display().to_string(),
        is_dogfood,
        ade_source_root: ade_source.map(|path| path.display().to_string()),
        audit,
        plan,
        handoff,
        leases,
        tasks,
        rebuild_lock_warnings: rebuild_lock_warnings(),
    })
}

#[tauri::command]
pub fn open_ade_on_itself(state: State<'_, AppState>) -> Result<DogfoodOpenResult, String> {
    let source = AppState::ade_source_root()
        .ok_or_else(|| "could not locate ADE source root from this Desktop build".to_string())?;
    let current = state.workspace_root();
    if same_path(&source, &current) {
        persist_preferred_workspace(&source)?;
        return Ok(DogfoodOpenResult {
            workspace_root: source.display().to_string(),
            already_dogfood: true,
        });
    }
    let root = state.set_workspace_root(source)?;
    Ok(DogfoodOpenResult {
        workspace_root: root.display().to_string(),
        already_dogfood: false,
    })
}

fn same_path(left: &Path, right: &Path) -> bool {
    left == right
        || left.canonicalize().ok().as_ref() == right.canonicalize().ok().as_ref()
        || left
            .to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
}

#[tauri::command]
pub fn key_status(
    state: State<'_, AppState>,
    provider: String,
    profile: Option<String>,
) -> Result<ProviderKeyStatus, String> {
    let profile = resolve_profile(profile)?;
    key_status_for(state.key_vault.as_ref(), &profile, &provider)
}

#[tauri::command]
pub fn key_set(
    state: State<'_, AppState>,
    provider: String,
    profile: Option<String>,
    secret: String,
) -> Result<ProviderKeyStatus, String> {
    let profile = resolve_profile(profile)?;
    state
        .key_vault
        .set(&profile, &provider, &secret)
        .map_err(|error| error.to_string())?;
    // Do not return, serialize, log, or retain the secret.
    Ok(ProviderKeyStatus {
        profile,
        provider: provider.trim().to_ascii_lowercase(),
        configured: true,
    })
}

#[tauri::command]
pub fn key_delete(
    state: State<'_, AppState>,
    provider: String,
    profile: Option<String>,
) -> Result<ProviderKeyDeleteResult, String> {
    let profile = resolve_profile(profile)?;
    let deleted = state
        .key_vault
        .delete(&profile, &provider)
        .map_err(|error| error.to_string())?;
    Ok(ProviderKeyDeleteResult {
        profile,
        provider: provider.trim().to_ascii_lowercase(),
        deleted,
    })
}

/// Safe provider smoke preflight. It checks only key presence and deliberately
/// skips when absent; live validation remains an explicit, potentially billable action.
#[tauri::command]
pub fn key_smoke(
    state: State<'_, AppState>,
    provider: String,
    profile: Option<String>,
) -> Result<ProviderKeySmokeResult, String> {
    let profile = resolve_profile(profile)?;
    let status = key_status_for(state.key_vault.as_ref(), &profile, &provider)?;
    Ok(if status.configured {
        ProviderKeySmokeResult {
            profile: status.profile,
            provider: status.provider,
            status: "ready".into(),
            detail: "credential is configured; no provider request was made".into(),
        }
    } else {
        ProviderKeySmokeResult {
            profile: status.profile,
            provider: status.provider,
            status: "skipped".into(),
            detail: "credential is absent; smoke test skipped safely".into(),
        }
    })
}

/// Explicitly billable provider validation. The caller must approve the request
/// and supply current pricing so the hard cap is checked before network I/O.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn key_live_smoke(
    state: State<'_, AppState>,
    provider: String,
    profile: Option<String>,
    base_url: String,
    model: String,
    input_cost_per_mtok: f64,
    output_cost_per_mtok: f64,
    max_cost_usd: f64,
    approve_cost: bool,
) -> Result<ProviderKeySmokeResult, String> {
    if !approve_cost {
        return Err("explicit cost approval is required for a live provider smoke".into());
    }
    if model.trim().is_empty() {
        return Err("an exact model id is required for a live provider smoke".into());
    }

    let profile = resolve_profile(profile)?;
    let report = ade_agents::smoke::run_live_agent_smoke_with_vault(
        ade_agents::smoke::LiveSmokeSpec {
            workspace_root: state.workspace_root(),
            profile: profile.clone(),
            provider: provider.clone(),
            base_url,
            model: model.trim().to_string(),
            input_cost_per_mtok: ade_core::money::Money::try_from_usd_f64(input_cost_per_mtok)
                .map_err(|error| error.to_string())?,
            output_cost_per_mtok: ade_core::money::Money::try_from_usd_f64(output_cost_per_mtok)
                .map_err(|error| error.to_string())?,
            context_limit: 8_192,
            output_limit: 16,
            max_cost: ade_core::money::Money::try_from_usd_f64(max_cost_usd)
                .map_err(|error| error.to_string())?,
        },
        Arc::clone(&state.key_vault),
    )
    .await
    .map_err(|error| error.to_string())?;

    let status = match report.status {
        ade_agents::smoke::LiveSmokeStatus::Passed => "passed",
        ade_agents::smoke::LiveSmokeStatus::Failed => "failed",
        ade_agents::smoke::LiveSmokeStatus::Skipped => "skipped",
    };
    Ok(ProviderKeySmokeResult {
        profile,
        provider: provider.trim().to_ascii_lowercase(),
        status: status.into(),
        detail: format!(
            "{} ({} input + {} output tokens, ${})",
            report.detail,
            report.input_tokens,
            report.output_tokens,
            ade_core::money::Money::from_micros(report.cost_micros).format_usd()
        ),
    })
}

fn key_status_for(
    vault: &dyn ade_db::secrets::ProviderKeyVault,
    profile: &str,
    provider: &str,
) -> Result<ProviderKeyStatus, String> {
    let configured = vault
        .contains(profile, provider)
        .map_err(|error| error.to_string())?;
    Ok(ProviderKeyStatus {
        profile: profile.to_string(),
        provider: provider.trim().to_ascii_lowercase(),
        configured,
    })
}

fn resolve_profile(profile: Option<String>) -> Result<String, String> {
    match profile.filter(|value| !value.trim().is_empty()) {
        Some(profile) => Ok(profile.trim().to_ascii_lowercase()),
        None => ade_core::config::AdeConfig::load()
            .map(|config| config.environment.to_string())
            .map_err(|error| error.to_string()),
    }
}

#[tauri::command]
pub async fn run_audit(state: State<'_, AppState>) -> Result<AuditReport, String> {
    Ok(AuditRunner::new(&state.workspace_root()).run(AuditMode::EvaluateExisting))
}

#[tauri::command]
pub async fn run_plan(state: State<'_, AppState>) -> Result<PlanReport, String> {
    let audit = AuditRunner::new(&state.workspace_root()).run(AuditMode::EvaluateExisting);
    let plan = PlanBuilder::new().build(&audit);
    ade_workflow::plan_enforcement::PlanEnforcer::save_plan(&state.workspace_root(), &plan)
        .map_err(|error| error.to_string())?;
    Ok(plan)
}

#[tauri::command]
pub async fn run_execute(
    state: State<'_, AppState>,
    approved: bool,
    recipe: Option<String>,
) -> Result<ExecuteReport, String> {
    let audit = AuditRunner::new(&state.workspace_root()).run(AuditMode::EvaluateExisting);
    let plan = PlanBuilder::new().build(&audit);
    let report = ade_workflow::executor::PhaseExecutor::with_root(&state.workspace_root())
        .execute(
            &plan,
            &ExecuteOptions {
                approved,
                recipe_id: recipe.unwrap_or_else(|| "rust-api-turso".into()),
                phase_ids: vec![],
            },
        )
        .map_err(|error| error.to_string())?;
    let mut capsule =
        ade_core::handoff::HandoffCapsule::from_execute("Continue approved ADE plan", &report);
    capsule.branch = current_branch(&state.workspace_root());
    ade_agents::handoff::HandoffManager::new(&state.workspace_root())
        .save_capsule(&capsule)
        .map_err(|error| error.to_string())?;
    Ok(report)
}

#[tauri::command]
pub async fn run_verify(
    state: State<'_, AppState>,
    gate: String,
    through: bool,
) -> Result<Vec<VerifyResult>, String> {
    let gate: VerifyGate = gate.parse()?;
    let runner = VerifyRunner::with_root(&state.workspace_root());
    let results = if through {
        runner.run_through(gate).await
    } else {
        vec![runner.run_gate(gate).await]
    };
    let manager = ade_agents::handoff::HandoffManager::new(&state.workspace_root());
    let mut capsule = manager.load_latest().unwrap_or_else(|_| {
        ade_core::handoff::HandoffCapsule::new(
            "Continue after workspace verification",
            "evaluate_existing",
        )
    });
    capsule.branch = current_branch(&state.workspace_root());
    capsule.apply_verify_results(&results);
    manager
        .save_capsule(&capsule)
        .map_err(|error| error.to_string())?;
    Ok(results)
}

#[tauri::command]
pub async fn mcp_connect(
    state: State<'_, AppState>,
    name: String,
    command: String,
    args: Vec<String>,
    approved: bool,
) -> Result<(), String> {
    state
        .mcp
        .connect_server(McpServerConfig {
            name,
            command,
            args,
            approved,
        })
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn mcp_list_servers(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    state
        .mcp
        .connected_servers()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn mcp_list_tools(state: State<'_, AppState>) -> Result<Vec<McpToolInfo>, String> {
    state
        .mcp
        .list_tools()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn mcp_disconnect(state: State<'_, AppState>, name: String) -> Result<bool, String> {
    state
        .mcp
        .disconnect_server(&name)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn mcp_call_tool(
    state: State<'_, AppState>,
    server: String,
    tool: String,
    arguments: serde_json::Value,
) -> Result<McpToolCallResult, String> {
    ade_agents::authority::AuthorityEnforcer::load(&state.workspace_root(), Vec::<String>::new())
        .and_then(|policy| policy.authorize_human_tool(&server, &tool, &arguments))
        .map_err(|error| error.to_string())?;
    state
        .mcp
        .call_tool(&server, &tool, arguments)
        .await
        .map_err(|error| error.to_string())
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn run_agent_turn(
    state: State<'_, AppState>,
    prompt: String,
    provider: String,
    base_url: String,
    model: String,
    input_cost_per_mtok: f64,
    output_cost_per_mtok: f64,
    context_limit: Option<u64>,
    output_limit: Option<u64>,
    session_cap_usd: Option<f64>,
    daily_cap_usd: Option<f64>,
    profile: Option<String>,
    lease_agent_id: Option<String>,
    autonomy: Option<String>,
    max_steps: Option<u32>,
    max_tokens: Option<u64>,
    verify_on_complete: Option<bool>,
    verify_gate: Option<String>,
    approve_owned_paths: Option<bool>,
    owned_paths: Option<Vec<String>>,
    on_event: Channel<AgentEvent>,
) -> Result<(), String> {
    if prompt.trim().is_empty() {
        return Err("prompt cannot be empty".into());
    }
    let profile = match profile {
        Some(profile) => profile,
        None => ade_core::config::AdeConfig::load()
            .map(|config| config.environment.to_string())
            .unwrap_or_else(|_| "local".into()),
    };
    let autonomy = autonomy
        .as_deref()
        .unwrap_or("propose")
        .parse::<ade_agents::autonomy::AutonomyLevel>()
        .map_err(|error| error.to_string())?;
    let input_cost = ade_core::money::Money::try_from_usd_f64(input_cost_per_mtok)
        .map_err(|error| error.to_string())?;
    let output_cost = ade_core::money::Money::try_from_usd_f64(output_cost_per_mtok)
        .map_err(|error| error.to_string())?;
    let priced =
        input_cost > ade_core::money::Money::ZERO || output_cost > ade_core::money::Money::ZERO;
    let context_limit = if priced {
        context_limit.unwrap_or(128_000)
    } else {
        0
    };
    let output_limit = if priced {
        output_limit.unwrap_or(16_384)
    } else {
        0
    };
    let mut spend_caps = ade_agents::spend::SpendCaps::from_env();
    if let Some(value) = session_cap_usd {
        spend_caps.session =
            ade_core::money::Money::try_from_usd_f64(value).map_err(|error| error.to_string())?;
    }
    if let Some(value) = daily_cap_usd {
        spend_caps.daily =
            ade_core::money::Money::try_from_usd_f64(value).map_err(|error| error.to_string())?;
    }

    let parsed_verify_gate = match verify_gate.as_deref() {
        Some(raw) => Some(
            raw.parse::<VerifyGate>()
                .map_err(|error| error.to_string())?,
        ),
        None => None,
    };
    let verify_on_complete = match verify_on_complete {
        Some(true) => Some(parsed_verify_gate.unwrap_or(VerifyGate::G3)),
        Some(false) if !autonomy.requires_verify_on_complete() => None,
        _ if autonomy.requires_verify_on_complete() => {
            Some(parsed_verify_gate.unwrap_or(VerifyGate::G3))
        }
        _ => parsed_verify_gate,
    };

    // Observe/Propose stay read-only. Act/Automate get write scope only when the
    // human explicitly approves PLAN owned_paths (generating a PLAN alone never
    // grants authority).
    let owned_paths = resolve_turn_owned_paths(
        &state.workspace_root(),
        autonomy,
        approve_owned_paths.unwrap_or(false),
        owned_paths.unwrap_or_default(),
    )?;
    let config = ade_core::config::AdeConfig::load().map_err(|error| error.to_string())?;
    let db = ade_db::repo::AdeDatabase::open(&ade_db::repo::DbConfig::from_ade_config(&config))
        .await
        .map_err(|error| error.to_string())?;
    let ledger = ade_db::usage_ledger::UsageLedgerStore::new(
        db.connect().map_err(|error| error.to_string())?,
    );
    let mut builder = ade_agents::turn::AgentTurnBuilder::new(ade_agents::turn::AgentTurnSpec {
        prompt,
        provider,
        base_url,
        model,
        input_cost_per_mtok: input_cost,
        output_cost_per_mtok: output_cost,
        context_limit,
        output_limit,
        profile,
        workspace_root: state.workspace_root(),
        owned_paths,
        handoff_chars: 1_500,
    })
    .mcp(state.mcp.clone())
    .ledger(ledger)
    .spend_caps(spend_caps)
    .key_vault(Arc::clone(&state.key_vault))
    .autonomy(autonomy)
    .max_tool_rounds(max_steps.unwrap_or(8) as usize)
    .max_tokens(max_tokens)
    .verify_on_complete(verify_on_complete);
    if let Some(agent) = lease_agent_id {
        let agent_id = uuid::Uuid::parse_str(&agent)
            .map_err(|error| format!("invalid lease agent UUID: {error}"))?;
        builder = builder.lease_agent(agent_id);
    }
    let service = builder.prepare().await.map_err(|error| error.to_string())?;

    let mut events = service.start();
    while let Some(event) = events.recv().await {
        let terminal = matches!(
            event,
            AgentEvent::Failed { .. } | AgentEvent::Cancelled { .. } | AgentEvent::Completed { .. }
        );
        on_event.send(event).map_err(|error| error.to_string())?;
        if terminal {
            break;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn list_recipes() -> Vec<StackRecipe> {
    ade_core::recipe::builtin_recipes()
}

#[tauri::command]
pub fn list_rules(
    state: State<'_, AppState>,
) -> Result<Vec<ade_agents::authority::RuleFileInfo>, String> {
    ade_agents::authority::list_rule_files(&state.workspace_root())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_skills(
    state: State<'_, AppState>,
) -> Result<Vec<ade_agents::skills::SkillDefinition>, String> {
    ade_agents::skills::SkillLoader::new(&state.workspace_root())
        .load_all()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn guided_wins_status(
    state: State<'_, AppState>,
) -> Result<ade_core::guided::GuidedWinsState, String> {
    ade_core::guided::load_wins(&state.workspace_root()).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn guided_understand_project(
    state: State<'_, AppState>,
) -> Result<ade_core::guided::UnderstandResult, String> {
    ade_core::guided::write_understand_project(&state.workspace_root())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn guided_mark_win(
    state: State<'_, AppState>,
    win: String,
) -> Result<ade_core::guided::GuidedWinsState, String> {
    let win = match win.trim().to_ascii_lowercase().as_str() {
        "understand" => ade_core::guided::GuidedWinId::Understand,
        "verify" => ade_core::guided::GuidedWinId::Verify,
        "improve_ade" | "improve-ade" | "improve" => ade_core::guided::GuidedWinId::ImproveAde,
        other => return Err(format!("unknown guided win '{other}'")),
    };
    ade_core::guided::mark_win(&state.workspace_root(), win).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn preview_recipe_scaffold(
    state: State<'_, AppState>,
    recipe: String,
    project_name: Option<String>,
    force: bool,
) -> Result<Vec<ade_core::scaffold::ScaffoldFilePlan>, String> {
    let recipe = ade_core::recipe::builtin_recipe(&recipe).map_err(|error| error.to_string())?;
    let name = project_name.unwrap_or_else(|| {
        state
            .workspace_root()
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("project")
            .to_string()
    });
    let context = ade_core::agents_contract::AgentsContractContext::new(name)
        .with_root(state.workspace_root().display().to_string());
    ade_core::scaffold::RecipeScaffold::plan(&state.workspace_root(), &recipe, &context, force)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn initialize_recipe(
    state: State<'_, AppState>,
    recipe: String,
    project_name: Option<String>,
    force: bool,
) -> Result<ade_core::scaffold::ScaffoldResult, String> {
    let recipe = ade_core::recipe::builtin_recipe(&recipe).map_err(|error| error.to_string())?;
    let name = project_name.unwrap_or_else(|| {
        state
            .workspace_root()
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("project")
            .to_string()
    });
    let context = ade_core::agents_contract::AgentsContractContext::new(name)
        .with_root(state.workspace_root().display().to_string());
    ade_core::scaffold::RecipeScaffold::apply(&state.workspace_root(), &recipe, &context, force)
        .map_err(|error| error.to_string())
}

fn current_branch(root: &std::path::Path) -> Option<String> {
    std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|branch| branch.trim().to_string())
        .filter(|branch| !branch.is_empty())
}

fn rebuild_lock_warnings() -> Vec<String> {
    let mut warnings = Vec::new();
    for name in ["ade-desktop-app", "ade-desktop-app.exe", "ade", "ade.exe"] {
        if process_appears_running(name) {
            warnings.push(format!(
                "{name} is running — `cargo build -p ade-desktop-app` may hit access denied / os error 5. Stop the process or exclude that package."
            ));
            break;
        }
    }
    warnings
}

fn process_appears_running(name: &str) -> bool {
    #[cfg(windows)]
    {
        let stem = name.trim_end_matches(".exe");
        let Ok(output) = std::process::Command::new("tasklist")
            .args(["/FI", &format!("IMAGENAME eq {stem}.exe"), "/NH"])
            .output()
        else {
            return false;
        };
        let stdout = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
        stdout.contains(&format!("{stem}.exe"))
    }
    #[cfg(not(windows))]
    {
        let stem = name.trim_end_matches(".exe");
        std::process::Command::new("pgrep")
            .arg("-x")
            .arg(stem)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

/// Resolve write scope for a desktop agent turn.
///
/// Observe/Propose → always empty. Act/Automate → empty unless the human
/// approved; then use provided paths or fall back to the saved/current PLAN.
fn resolve_turn_owned_paths(
    workspace_root: &std::path::Path,
    autonomy: ade_agents::autonomy::AutonomyLevel,
    approve_owned_paths: bool,
    provided: Vec<String>,
) -> Result<Vec<String>, String> {
    if !autonomy.allows_mutating_tools() {
        return Ok(vec![]);
    }
    if !approve_owned_paths {
        return Ok(vec![]);
    }

    let mut paths: Vec<String> = provided
        .into_iter()
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .collect();

    if paths.is_empty() {
        if let Some(plan) = ade_workflow::plan_enforcement::PlanEnforcer::load_plan(workspace_root)
            .map_err(|error| error.to_string())?
        {
            paths = plan
                .phases
                .into_iter()
                .flat_map(|phase| phase.owned_paths)
                .collect();
        }
    }

    if paths.is_empty() {
        // Persist a fresh plan so risky-path enforcement has an artifact, then
        // use its owned_paths as the write scope.
        let audit = AuditRunner::new(workspace_root).run(AuditMode::EvaluateExisting);
        let plan = PlanBuilder::new().build(&audit);
        ade_workflow::plan_enforcement::PlanEnforcer::save_plan(workspace_root, &plan)
            .map_err(|error| error.to_string())?;
        paths = plan
            .phases
            .into_iter()
            .flat_map(|phase| phase.owned_paths)
            .collect();
    } else if ade_workflow::plan_enforcement::PlanEnforcer::load_plan(workspace_root)
        .map_err(|error| error.to_string())?
        .is_none()
    {
        // Human approved explicit paths; still need a PLAN artifact for risky work.
        let audit = AuditRunner::new(workspace_root).run(AuditMode::EvaluateExisting);
        let plan = PlanBuilder::new().build(&audit);
        ade_workflow::plan_enforcement::PlanEnforcer::save_plan(workspace_root, &plan)
            .map_err(|error| error.to_string())?;
    }

    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        return Err("approve owned paths requires a PLAN with owned_paths — run Plan first".into());
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ade_db::secrets::{InMemoryProviderKeyVault, ProviderKeyVault};

    #[test]
    fn discovers_a_workspace_root() {
        let state = AppState::discover();
        assert!(state.workspace_root().join("Cargo.toml").is_file());
        assert!(state.workspace_root().join("AGENTS.md").is_file());
    }

    #[test]
    fn key_status_never_returns_secret_material() {
        let vault = InMemoryProviderKeyVault::default();
        vault.set("local", "openai", "super-secret-value").unwrap();
        let status = key_status_for(&vault, "local", "openai").unwrap();
        assert!(status.configured);
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.contains("super-secret-value"));
        assert!(!json.contains("\"secret\""));
    }

    #[test]
    fn key_smoke_preflight_skips_when_absent() {
        let vault = InMemoryProviderKeyVault::default();
        let status = key_status_for(&vault, "local", "openai").unwrap();
        assert!(!status.configured);
    }

    #[test]
    fn propose_never_gets_owned_paths_even_if_approved() {
        let root = std::env::temp_dir().join(format!("ade-own-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let paths = resolve_turn_owned_paths(
            &root,
            ade_agents::autonomy::AutonomyLevel::Propose,
            true,
            vec!["crates/agents".into()],
        )
        .unwrap();
        assert!(paths.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn act_without_approval_stays_empty() {
        let root = std::env::temp_dir().join(format!("ade-own2-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let paths = resolve_turn_owned_paths(
            &root,
            ade_agents::autonomy::AutonomyLevel::Act,
            false,
            vec!["crates/agents".into()],
        )
        .unwrap();
        assert!(paths.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn act_with_approval_uses_provided_paths() {
        let root = std::env::temp_dir().join(format!("ade-own3-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("AGENTS.md"), "# contract\n").unwrap();
        let paths = resolve_turn_owned_paths(
            &root,
            ade_agents::autonomy::AutonomyLevel::Act,
            true,
            vec!["crates/agents".into(), "crates/agents".into()],
        )
        .unwrap();
        assert_eq!(paths, vec!["crates/agents".to_string()]);
        assert!(
            ade_workflow::plan_enforcement::PlanEnforcer::plan_path(&root).is_file(),
            "approving writes should persist a PLAN artifact"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
