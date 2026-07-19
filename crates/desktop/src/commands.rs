use ade_agents::mcp::{McpHost, McpServerConfig, McpToolCallResult, McpToolInfo};
use ade_agents::session::AgentEvent;
use ade_core::audit::{AuditMode, AuditReport, AuditRunner};
use ade_core::execute::{ExecuteOptions, ExecuteReport, ExecuteRunner};
use ade_core::plan::{PlanBuilder, PlanReport};
use ade_core::recipe::StackRecipe;
use ade_core::verify::{VerifyGate, VerifyResult};
use ade_workflow::verify::VerifyRunner;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{ipc::Channel, State};

pub struct AppState {
    pub workspace_root: PathBuf,
    pub mcp: McpHost,
    pub key_vault: Arc<dyn ade_db::secrets::ProviderKeyVault>,
}

impl AppState {
    pub fn discover() -> Self {
        let configured = std::env::var("ADE_WORKSPACE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from);
        let current = std::env::current_dir()
            .ok()
            .filter(|root| root.join("Cargo.toml").is_file() && root.join("AGENTS.md").is_file());
        let source_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .map(PathBuf::from);
        let workspace_root = configured
            .or(current)
            .or(source_root)
            .unwrap_or_else(|| PathBuf::from("."))
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from("."));
        Self {
            workspace_root,
            mcp: McpHost::new(),
            key_vault: Arc::new(ade_db::secrets::NativeProviderKeyVault),
        }
    }
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
    pub audit: AuditReport,
    pub plan: PlanReport,
    pub handoff: ade_agents::handoff::HandoffMetrics,
    pub leases: Vec<ade_workflow::parallel::PathLease>,
    pub tasks: Vec<ade_workflow::tasks::AgentTask>,
}

#[tauri::command]
pub async fn get_dashboard(state: State<'_, AppState>) -> Result<DashboardSnapshot, String> {
    let audit = AuditRunner::new(&state.workspace_root).run(AuditMode::EvaluateExisting);
    let plan = PlanBuilder::new().build(&audit);
    let handoff = ade_agents::handoff::HandoffManager::new(&state.workspace_root)
        .metrics()
        .map_err(|error| error.to_string())?;
    let leases = ade_workflow::parallel::LeaseManager::new(&state.workspace_root)
        .list()
        .map_err(|error| error.to_string())?;
    let tasks = ade_workflow::tasks::TaskCoordinator::new(&state.workspace_root)
        .list()
        .map_err(|error| error.to_string())?;
    Ok(DashboardSnapshot {
        workspace_root: state.workspace_root.display().to_string(),
        audit,
        plan,
        handoff,
        leases,
        tasks,
    })
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
            workspace_root: state.workspace_root.clone(),
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
    Ok(AuditRunner::new(&state.workspace_root).run(AuditMode::EvaluateExisting))
}

#[tauri::command]
pub async fn run_plan(state: State<'_, AppState>) -> Result<PlanReport, String> {
    let audit = AuditRunner::new(&state.workspace_root).run(AuditMode::EvaluateExisting);
    Ok(PlanBuilder::new().build(&audit))
}

#[tauri::command]
pub async fn run_execute(
    state: State<'_, AppState>,
    approved: bool,
    recipe: Option<String>,
) -> Result<ExecuteReport, String> {
    let audit = AuditRunner::new(&state.workspace_root).run(AuditMode::EvaluateExisting);
    let plan = PlanBuilder::new().build(&audit);
    let report = ExecuteRunner::new(&state.workspace_root)
        .run(
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
    capsule.branch = current_branch(&state.workspace_root);
    ade_agents::handoff::HandoffManager::new(&state.workspace_root)
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
    let runner = VerifyRunner::with_root(&state.workspace_root);
    let results = if through {
        runner.run_through(gate).await
    } else {
        vec![runner.run_gate(gate).await]
    };
    let manager = ade_agents::handoff::HandoffManager::new(&state.workspace_root);
    let mut capsule = manager.load_latest().unwrap_or_else(|_| {
        ade_core::handoff::HandoffCapsule::new(
            "Continue after workspace verification",
            "evaluate_existing",
        )
    });
    capsule.branch = current_branch(&state.workspace_root);
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
    ade_agents::authority::AuthorityEnforcer::load(&state.workspace_root, Vec::<String>::new())
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

    // Desktop chat starts read-only. A later explicit plan-approval action can
    // pass owned_paths; merely generating a PLAN never grants write authority.
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
        workspace_root: state.workspace_root.clone(),
        owned_paths: vec![],
        handoff_chars: 1_500,
    })
    .mcp(state.mcp.clone())
    .ledger(ledger)
    .spend_caps(spend_caps)
    .key_vault(Arc::clone(&state.key_vault));
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
pub fn preview_recipe_scaffold(
    state: State<'_, AppState>,
    recipe: String,
    project_name: Option<String>,
    force: bool,
) -> Result<Vec<ade_core::scaffold::ScaffoldFilePlan>, String> {
    let recipe = ade_core::recipe::builtin_recipe(&recipe).map_err(|error| error.to_string())?;
    let name = project_name.unwrap_or_else(|| {
        state
            .workspace_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("project")
            .to_string()
    });
    let context = ade_core::agents_contract::AgentsContractContext::new(name)
        .with_root(state.workspace_root.display().to_string());
    ade_core::scaffold::RecipeScaffold::plan(&state.workspace_root, &recipe, &context, force)
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
            .workspace_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("project")
            .to_string()
    });
    let context = ade_core::agents_contract::AgentsContractContext::new(name)
        .with_root(state.workspace_root.display().to_string());
    ade_core::scaffold::RecipeScaffold::apply(&state.workspace_root, &recipe, &context, force)
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

#[cfg(test)]
mod tests {
    use super::*;
    use ade_db::secrets::{InMemoryProviderKeyVault, ProviderKeyVault};

    #[test]
    fn discovers_a_workspace_root() {
        let state = AppState::discover();
        assert!(state.workspace_root.join("Cargo.toml").is_file());
        assert!(state.workspace_root.join("AGENTS.md").is_file());
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
}
