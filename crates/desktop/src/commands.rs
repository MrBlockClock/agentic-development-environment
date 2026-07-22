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
use tauri::{ipc::Channel, AppHandle, Manager, State, Url, WebviewUrl, WebviewWindowBuilder};

pub struct AppState {
    workspace_root: RwLock<PathBuf>,
    pub mcp: McpHost,
    pub key_vault: Arc<dyn ade_db::secrets::ProviderKeyVault>,
    pub pty: crate::pty::PtyHub,
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
        if !is_ade_workspace(&canonical) {
            return Err(
                "folder is not an ADE workspace yet (missing AGENTS.md). Use Create / Adopt first."
                    .into(),
            );
        }
        *self
            .workspace_root
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = canonical.clone();
        remember_workspace(&canonical)?;
        Ok(canonical)
    }

    pub fn ade_source_root() -> Option<PathBuf> {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .map(PathBuf::from)
            .and_then(|path| path.canonicalize().ok())
            .filter(|path| is_ade_workspace(path))
    }

    pub fn discover() -> Self {
        let configured = std::env::var("ADE_WORKSPACE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .or_else(load_preferred_workspace);
        let current = std::env::current_dir()
            .ok()
            .filter(|root| is_ade_workspace(root));
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
        match ade_db::secrets::import_opencode_auth_gaps(key_vault.as_ref(), "local") {
            Ok(imported) if !imported.is_empty() => {
                tracing::info!(
                    providers = ?imported,
                    "filled missing provider keys from OpenCode auth.json (values not logged)"
                );
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, "OpenCode auth gap-fill skipped");
            }
        }
        if is_ade_workspace(&workspace_root) {
            let _ = remember_workspace(&workspace_root);
        }
        Self {
            workspace_root: RwLock::new(workspace_root),
            mcp: McpHost::new(),
            key_vault,
            pty: crate::pty::PtyHub::default(),
        }
    }
}

fn is_ade_workspace(root: &Path) -> bool {
    root.is_dir() && root.join("AGENTS.md").is_file()
}

fn preferred_workspace_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|base| base.join("ade").join("workspace-root.txt"))
}

fn recent_workspaces_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|base| base.join("ade").join("recent-workspaces.json"))
}

fn load_preferred_workspace() -> Option<PathBuf> {
    let path = preferred_workspace_path()?;
    let raw = std::fs::read_to_string(path).ok()?;
    let root = PathBuf::from(raw.trim());
    if is_ade_workspace(&root) {
        Some(root)
    } else {
        None
    }
}

fn persist_preferred_workspace(root: &Path) -> Result<(), String> {
    let path = preferred_workspace_path()
        .ok_or_else(|| "LOCALAPPDATA is not set; cannot persist preferred workspace".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(&path, root.display().to_string()).map_err(|error| error.to_string())
}

fn load_recent_workspaces() -> Vec<PathBuf> {
    let Some(path) = recent_workspaces_path() else {
        return Vec::new();
    };
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(entries) = serde_json::from_str::<Vec<String>>(&raw) else {
        return Vec::new();
    };
    entries
        .into_iter()
        .map(PathBuf::from)
        .filter(|root| is_ade_workspace(root))
        .collect()
}

fn push_recent_workspace(root: &Path) -> Result<(), String> {
    let path = recent_workspaces_path()
        .ok_or_else(|| "LOCALAPPDATA is not set; cannot persist recent workspaces".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut recent = load_recent_workspaces();
    recent.retain(|item| !same_path(item, root));
    recent.insert(0, root.to_path_buf());
    recent.truncate(12);
    let encoded = serde_json::to_string_pretty(
        &recent
            .iter()
            .map(|item| item.display().to_string())
            .collect::<Vec<_>>(),
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(path, encoded).map_err(|error| error.to_string())
}

fn remember_workspace(root: &Path) -> Result<(), String> {
    persist_preferred_workspace(root)?;
    push_recent_workspace(root)
}

fn workspace_display_name(root: &Path) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace")
        .to_string()
}

fn minimal_agents_md(project_name: &str, root: &Path) -> String {
    format!(
        r#"# {project_name} — Agent Contract

Created by ADE Desktop (adopt / create workspace).

## Authority Order

1. Law/security/human direction
2. CI, tests, schemas
3. This AGENTS.md
4. `.ade/rules/` scoped rules
5. Task/issue acceptance criteria
6. Provider/adapter files
7. Chat memory

## Golden Path

- **Root:** `{root}`
- **Runtime:** local
- Pick a stack under **Recipes** so ADE can pin toolchain + verify gates.

## Notes

- Environment Audit diagnoses setup gaps for this folder.
- Home is where you ask ADE to work in this environment.
"#,
        project_name = project_name,
        root = root.display()
    )
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
    pub has_recipe: bool,
    pub has_provider_key: bool,
    pub audit: AuditReport,
    pub plan: PlanReport,
    pub handoff: ade_agents::handoff::HandoffMetrics,
    pub leases: Vec<ade_workflow::parallel::PathLease>,
    pub tasks: Vec<ade_workflow::tasks::AgentTask>,
    pub rebuild_lock_warnings: Vec<String>,
    /// Last gate evidence from Checks / run_verify (persisted under .ade/verify/).
    pub last_verify: Vec<VerifyResult>,
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
    // Keep Environment / Plan Map disk artifacts aligned with the live audit.
    let _ = ade_workflow::plan_enforcement::PlanEnforcer::save_plan(&workspace_root, &plan);
    let handoff = ade_agents::handoff::HandoffManager::new(&workspace_root)
        .metrics()
        .map_err(|error| error.to_string())?;
    let leases = ade_workflow::parallel::LeaseManager::new(&workspace_root)
        .list()
        .map_err(|error| error.to_string())?;
    let tasks = ade_workflow::tasks::TaskCoordinator::new(&workspace_root)
        .list()
        .map_err(|error| error.to_string())?;
    let has_recipe = workspace_root.join(".ade").join("recipe.json").is_file();
    let has_provider_key = any_provider_key_configured(state.key_vault.as_ref());
    let last_verify = load_last_verify(&workspace_root);
    Ok(DashboardSnapshot {
        workspace_root: workspace_root.display().to_string(),
        is_dogfood,
        ade_source_root: ade_source.map(|path| path.display().to_string()),
        has_recipe,
        has_provider_key,
        audit,
        plan,
        handoff,
        leases,
        tasks,
        rebuild_lock_warnings: rebuild_lock_warnings(),
        last_verify,
    })
}

fn any_provider_key_configured(vault: &dyn ade_db::secrets::ProviderKeyVault) -> bool {
    const PROVIDERS: &[&str] = &[
        "opencode",
        "freellm",
        "openai",
        "anthropic",
        "openrouter",
        "azure-openai",
    ];
    PROVIDERS
        .iter()
        .any(|provider| vault.contains("local", provider).unwrap_or(false))
}

#[derive(Serialize)]
pub struct WorkspaceEntry {
    pub path: String,
    pub name: String,
    pub has_agents: bool,
    pub has_recipe: bool,
    pub is_current: bool,
    pub is_ade_source: bool,
}

#[derive(Serialize)]
pub struct WorkspaceList {
    pub current: String,
    pub entries: Vec<WorkspaceEntry>,
    pub ade_source_root: Option<String>,
}

#[tauri::command]
pub fn open_ade_on_itself(state: State<'_, AppState>) -> Result<DogfoodOpenResult, String> {
    let source = AppState::ade_source_root()
        .ok_or_else(|| "could not locate ADE source root from this Desktop build".to_string())?;
    let current = state.workspace_root();
    if same_path(&source, &current) {
        remember_workspace(&source)?;
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

#[tauri::command]
pub fn list_workspaces(state: State<'_, AppState>) -> Result<WorkspaceList, String> {
    let current = state.workspace_root();
    let ade_source = AppState::ade_source_root();
    let mut paths: Vec<PathBuf> = Vec::new();
    paths.push(current.clone());
    for recent in load_recent_workspaces() {
        if !paths.iter().any(|item| same_path(item, &recent)) {
            paths.push(recent);
        }
    }
    if let Some(source) = &ade_source {
        if !paths.iter().any(|item| same_path(item, source)) {
            paths.push(source.clone());
        }
    }
    let entries = paths
        .into_iter()
        .map(|path| {
            let has_agents = path.join("AGENTS.md").is_file();
            let has_recipe = path.join(".ade").join("recipe.json").is_file();
            WorkspaceEntry {
                name: workspace_display_name(&path),
                is_current: same_path(&path, &current),
                is_ade_source: ade_source
                    .as_ref()
                    .is_some_and(|source| same_path(source, &path)),
                has_agents,
                has_recipe,
                path: path.display().to_string(),
            }
        })
        .collect();
    Ok(WorkspaceList {
        current: current.display().to_string(),
        entries,
        ade_source_root: ade_source.map(|path| path.display().to_string()),
    })
}

#[tauri::command]
pub fn open_workspace(
    state: State<'_, AppState>,
    path: String,
) -> Result<DogfoodOpenResult, String> {
    let root = PathBuf::from(path.trim());
    if !root.is_dir() {
        return Err(format!("not a directory: {}", root.display()));
    }
    if !is_ade_workspace(&root) {
        return Err(
            "folder is not an ADE workspace yet (missing AGENTS.md). Use Adopt / Create first."
                .into(),
        );
    }
    let root = state.set_workspace_root(root)?;
    Ok(DogfoodOpenResult {
        workspace_root: root.display().to_string(),
        already_dogfood: AppState::ade_source_root()
            .is_some_and(|source| same_path(&source, &root)),
    })
}

/// Adopt a folder as an ADE workspace: write AGENTS.md (unless present), ensure `.ade/`, then open.
#[tauri::command]
pub fn create_workspace(
    state: State<'_, AppState>,
    path: String,
    project_name: Option<String>,
    force: Option<bool>,
) -> Result<DogfoodOpenResult, String> {
    let root = PathBuf::from(path.trim());
    if !root.exists() {
        std::fs::create_dir_all(&root).map_err(|error| format!("create folder: {error}"))?;
    }
    if !root.is_dir() {
        return Err(format!("not a directory: {}", root.display()));
    }
    let canonical = root
        .canonicalize()
        .map_err(|error| format!("canonicalize: {error}"))?;
    let name = project_name
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| workspace_display_name(&canonical));
    let agents = canonical.join("AGENTS.md");
    let force = force.unwrap_or(false);
    if agents.is_file() && !force {
        // Already a workspace — just open.
    } else {
        std::fs::write(&agents, minimal_agents_md(&name, &canonical))
            .map_err(|error| format!("write AGENTS.md: {error}"))?;
    }
    let ade_dir = canonical.join(".ade");
    if !ade_dir.is_dir() {
        std::fs::create_dir_all(&ade_dir).map_err(|error| format!("create .ade: {error}"))?;
    }
    let root = state.set_workspace_root(canonical)?;
    Ok(DogfoodOpenResult {
        workspace_root: root.display().to_string(),
        already_dogfood: AppState::ade_source_root()
            .is_some_and(|source| same_path(&source, &root)),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderVaultRow {
    pub provider: String,
    pub configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCodeImportResult {
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
    pub detail: String,
}

/// List known BYOK providers and whether a vault credential exists (never returns secrets).
#[tauri::command]
pub fn key_status_all(
    state: State<'_, AppState>,
    profile: Option<String>,
) -> Result<Vec<ProviderVaultRow>, String> {
    let profile = resolve_profile(profile)?;
    const PROVIDERS: &[&str] = &[
        "opencode",
        "freellm",
        "openai",
        "anthropic",
        "openrouter",
        "azure-openai",
    ];
    let mut rows = Vec::with_capacity(PROVIDERS.len());
    for provider in PROVIDERS {
        let configured = state
            .key_vault
            .contains(&profile, provider)
            .map_err(|error| error.to_string())?;
        rows.push(ProviderVaultRow {
            provider: (*provider).to_string(),
            configured,
        });
    }
    Ok(rows)
}

/// Import API keys from OpenCode Desktop's auth.json into the ADE OS vault.
/// Secrets never leave the desktop process / never return to the UI.
#[tauri::command]
pub fn key_import_opencode_auth(
    state: State<'_, AppState>,
    profile: Option<String>,
) -> Result<OpenCodeImportResult, String> {
    let profile = resolve_profile(profile)?;
    let path = ade_db::secrets::find_opencode_auth_path().ok_or_else(|| {
        "OpenCode auth.json not found (tried ~/.local/share/opencode, %APPDATA%/opencode, %LOCALAPPDATA%/opencode)"
            .to_string()
    })?;
    let imported =
        ade_db::secrets::import_opencode_auth_file(state.key_vault.as_ref(), &profile, &path, true)
            .map_err(|error| error.to_string())?;
    let skipped = Vec::<String>::new();
    Ok(OpenCodeImportResult {
        detail: format!(
            "Read {} · imported/updated {} (force sync)",
            path.display(),
            imported.len()
        ),
        imported,
        skipped,
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
        // Desktop dogfood always uses the local vault profile so Keys ↔ Agent match.
        None => Ok("local".into()),
    }
}

#[tauri::command]
pub async fn run_audit(state: State<'_, AppState>) -> Result<AuditReport, String> {
    Ok(AuditRunner::new(state.workspace_root()).run(AuditMode::EvaluateExisting))
}

#[tauri::command]
pub async fn run_plan(state: State<'_, AppState>) -> Result<PlanReport, String> {
    let audit = AuditRunner::new(state.workspace_root()).run(AuditMode::EvaluateExisting);
    let plan = PlanBuilder::new().build(&audit);
    ade_workflow::plan_enforcement::PlanEnforcer::save_plan(state.workspace_root(), &plan)
        .map_err(|error| error.to_string())?;
    Ok(plan)
}

#[tauri::command]
pub async fn run_execute(
    state: State<'_, AppState>,
    approved: bool,
    recipe: Option<String>,
) -> Result<ExecuteReport, String> {
    let audit = AuditRunner::new(state.workspace_root()).run(AuditMode::EvaluateExisting);
    let plan = PlanBuilder::new().build(&audit);
    let report = ade_workflow::executor::PhaseExecutor::with_root(state.workspace_root())
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
    ade_agents::handoff::HandoffManager::new(state.workspace_root())
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
    let runner = VerifyRunner::with_root(state.workspace_root());
    let results = if through {
        runner.run_through(gate).await
    } else {
        vec![runner.run_gate(gate).await]
    };
    save_last_verify(&state.workspace_root(), &results);
    let manager = ade_agents::handoff::HandoffManager::new(state.workspace_root());
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

fn last_verify_path(workspace: &std::path::Path) -> std::path::PathBuf {
    workspace.join(".ade").join("verify").join("last.json")
}

fn save_last_verify(workspace: &std::path::Path, results: &[VerifyResult]) {
    let path = last_verify_path(workspace);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(raw) = serde_json::to_vec_pretty(results) {
        let _ = std::fs::write(path, raw);
    }
}

fn load_last_verify(workspace: &std::path::Path) -> Vec<VerifyResult> {
    let path = last_verify_path(workspace);
    let Ok(raw) = std::fs::read_to_string(path) else {
        return vec![];
    };
    serde_json::from_str(&raw).unwrap_or_default()
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
    ade_agents::authority::AuthorityEnforcer::load(state.workspace_root(), Vec::<String>::new())
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
    preferred_shell_cwd: Option<String>,
    // G4: optional isolated checkout; leases/PLAN stay on primary via coordination_root.
    execution_root: Option<String>,
    on_event: Channel<AgentEvent>,
) -> Result<(), String> {
    if prompt.trim().is_empty() {
        return Err("prompt cannot be empty".into());
    }
    let profile = match profile {
        Some(profile) if !profile.trim().is_empty() => profile.trim().to_ascii_lowercase(),
        _ => "local".into(),
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
    let primary_root = state.workspace_root();
    let owned_paths = resolve_turn_owned_paths(
        &primary_root,
        autonomy,
        approve_owned_paths.unwrap_or(false),
        owned_paths.unwrap_or_default(),
    )?;
    let execution_root = match execution_root
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(path) => {
            let candidate = std::path::PathBuf::from(path);
            if !candidate.is_dir() {
                return Err(format!(
                    "execution_root is not a directory: {}",
                    candidate.display()
                ));
            }
            candidate
        }
        None => primary_root.clone(),
    };

    let lease_agent = match lease_agent_id {
        Some(agent) => {
            let agent_id = uuid::Uuid::parse_str(&agent)
                .map_err(|error| format!("invalid lease agent UUID: {error}"))?;
            // Frontend may miss bootstrap paths; claim leases here so Act prepare
            // does not fail "owned_path not covered by an active writable lease".
            if approve_owned_paths.unwrap_or(false) && !owned_paths.is_empty() {
                let leases = ade_workflow::parallel::LeaseManager::new(&primary_root);
                for path in &owned_paths {
                    match leases.acquire(
                        agent_id,
                        path,
                        ade_workflow::parallel::LeaseMode::Strong,
                        chrono::Duration::seconds(300),
                    ) {
                        Ok(_) => {}
                        Err(error)
                            if error.to_string().to_lowercase().contains("already holds") => {}
                        Err(error) => return Err(error.to_string()),
                    }
                }
            }
            Some(agent_id)
        }
        None => None,
    };

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
        workspace_root: execution_root.clone(),
        owned_paths,
        handoff_chars: 1_500,
    })
    .mcp(state.mcp.clone())
    .ledger(ledger)
    .spend_caps(spend_caps)
    .key_vault(Arc::clone(&state.key_vault))
    .autonomy(autonomy)
    .max_tool_rounds(max_steps.unwrap_or(32) as usize)
    .max_tokens(max_tokens)
    .verify_on_complete(verify_on_complete)
    .preferred_shell_cwd(preferred_shell_cwd);
    if execution_root != primary_root {
        builder = builder.coordination_root(primary_root);
    }
    if let Some(agent_id) = lease_agent {
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
pub fn rank_recipes(
    answers: ade_core::recipe_fit::FitAnswers,
) -> Vec<ade_core::recipe_fit::ScoredRecipe> {
    ade_core::recipe_fit::rank_builtin_recipes(&answers)
}

#[tauri::command]
pub fn list_rules(
    state: State<'_, AppState>,
) -> Result<Vec<ade_agents::authority::RuleFileInfo>, String> {
    let _ = ade_core::guidance::ensure_guidance_dirs();
    ade_agents::authority::list_rule_files(state.workspace_root())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_skills(
    state: State<'_, AppState>,
) -> Result<Vec<ade_agents::skills::SkillDefinition>, String> {
    let _ = ade_core::guidance::ensure_guidance_dirs();
    ade_agents::skills::SkillLoader::new(state.workspace_root())
        .load_all()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_guidance_profiles(
    state: State<'_, AppState>,
) -> Result<Vec<ade_core::guidance::GuidanceProfile>, String> {
    ade_core::guidance::load_profiles(&state.workspace_root()).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_active_guidance_profile() -> Result<Option<String>, String> {
    Ok(ade_core::guidance::read_active_profile_id())
}

#[tauri::command]
pub fn set_active_guidance_profile(id: Option<String>) -> Result<Option<String>, String> {
    ade_core::guidance::write_active_profile_id(id.as_deref())
        .map_err(|error| error.to_string())?;
    Ok(ade_core::guidance::read_active_profile_id())
}

#[tauri::command]
pub fn run_global_audit(
    state: State<'_, AppState>,
) -> Result<ade_core::guidance::GlobalAuditReport, String> {
    let root = state.workspace_root();
    Ok(ade_core::guidance::run_global_audit(Some(root.as_path())))
}

#[tauri::command]
pub fn guided_wins_status(
    state: State<'_, AppState>,
) -> Result<ade_core::guided::GuidedWinsState, String> {
    ade_core::guided::load_wins(state.workspace_root()).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn guided_understand_project(
    state: State<'_, AppState>,
) -> Result<ade_core::guided::UnderstandResult, String> {
    ade_core::guided::write_understand_project(state.workspace_root())
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
    ade_core::guided::mark_win(state.workspace_root(), win).map_err(|error| error.to_string())
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
    ade_core::scaffold::RecipeScaffold::plan(state.workspace_root(), &recipe, &context, force)
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
    ade_core::scaffold::RecipeScaffold::apply(state.workspace_root(), &recipe, &context, force)
        .map_err(|error| error.to_string())
}

const BROWSER_WINDOW_LABEL: &str = "ade-browser";

fn parse_browser_url(raw: &str) -> Result<Url, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("url is required".into());
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let parsed = Url::parse(&with_scheme).map_err(|error| format!("invalid url: {error}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("only http and https urls are allowed".into());
    }
    Ok(parsed)
}

/// Open (or focus + navigate) a dedicated Chromium/WebView2 window for browsing.
#[tauri::command]
pub fn open_browser_window(app: AppHandle, url: String) -> Result<String, String> {
    let parsed = parse_browser_url(&url)?;
    if let Some(window) = app.get_webview_window(BROWSER_WINDOW_LABEL) {
        window
            .navigate(parsed.clone())
            .map_err(|error| format!("navigate browser: {error}"))?;
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(parsed.to_string());
    }
    WebviewWindowBuilder::new(
        &app,
        BROWSER_WINDOW_LABEL,
        WebviewUrl::External(parsed.clone()),
    )
    .title("ADE Browser")
    .inner_size(1180.0, 820.0)
    .resizable(true)
    .build()
    .map_err(|error| format!("open browser: {error}"))?;
    Ok(parsed.to_string())
}

#[tauri::command]
pub fn browser_window_url(app: AppHandle) -> Result<Option<String>, String> {
    let Some(window) = app.get_webview_window(BROWSER_WINDOW_LABEL) else {
        return Ok(None);
    };
    window
        .url()
        .map(|url| Some(url.to_string()))
        .map_err(|error| format!("browser url: {error}"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PtySpawnResult {
    pub session_id: String,
    pub cwd: String,
}

#[tauri::command]
pub fn pty_spawn(
    state: State<'_, AppState>,
    cols: u16,
    rows: u16,
    on_event: Channel<crate::pty::PtyEvent>,
) -> Result<PtySpawnResult, String> {
    let cwd = state.workspace_root();
    let session_id = state.pty.spawn(&cwd, cols, rows, on_event)?;
    Ok(PtySpawnResult {
        session_id,
        cwd: cwd.display().to_string(),
    })
}

#[tauri::command]
pub fn pty_write(
    state: State<'_, AppState>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    state.pty.write(&session_id, &data)
}

#[tauri::command]
pub fn pty_resize(
    state: State<'_, AppState>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    state.pty.resize(&session_id, cols, rows)
}

#[tauri::command]
pub fn pty_kill(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    state.pty.kill(&session_id)
}

#[tauri::command]
pub fn open_system_terminal(state: State<'_, AppState>) -> Result<String, String> {
    let cwd = state.workspace_root();
    let cwd_display = cwd.display().to_string();

    #[cfg(windows)]
    {
        let ps = format!(
            "Set-Location -LiteralPath '{}'",
            cwd_display.replace('\'', "''")
        );
        std::process::Command::new("cmd")
            .args([
                "/C",
                "start",
                "ADE Terminal",
                "powershell.exe",
                "-NoExit",
                "-Command",
                &ps,
            ])
            .spawn()
            .map_err(|error| format!("open system terminal: {error}"))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-a", "Terminal", cwd.as_os_str()])
            .spawn()
            .map_err(|error| format!("open system terminal: {error}"))?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let candidates = [
            "x-terminal-emulator",
            "gnome-terminal",
            "konsole",
            "xfce4-terminal",
        ];
        let mut opened = false;
        for bin in candidates {
            let result = if bin == "gnome-terminal" || bin == "xfce4-terminal" {
                std::process::Command::new(bin)
                    .arg(format!("--working-directory={}", cwd_display))
                    .spawn()
            } else if bin == "konsole" {
                std::process::Command::new(bin)
                    .args(["--workdir", &cwd_display])
                    .spawn()
            } else {
                std::process::Command::new(bin)
                    .arg(format!("--working-directory={cwd_display}"))
                    .spawn()
            };
            if result.is_ok() {
                opened = true;
                break;
            }
        }
        if !opened {
            return Err(
                "no system terminal found (tried x-terminal-emulator, gnome-terminal, konsole, xfce4-terminal)"
                    .into(),
            );
        }
    }

    Ok(cwd_display)
}

#[tauri::command]
pub fn chat_load(state: State<'_, AppState>) -> Result<ade_agents::chat::ChatThread, String> {
    ade_agents::chat::ChatStore::new(state.workspace_root())
        .load()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn chat_save(
    state: State<'_, AppState>,
    turns: Vec<ade_agents::chat::ChatTurn>,
) -> Result<ade_agents::chat::ChatThread, String> {
    ade_agents::chat::ChatStore::new(state.workspace_root())
        .save(turns)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn chat_clear(state: State<'_, AppState>) -> Result<ade_agents::chat::ChatThread, String> {
    ade_agents::chat::ChatStore::new(state.workspace_root())
        .clear()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn goal_list(state: State<'_, AppState>) -> Result<Vec<ade_agents::goal::EngGoal>, String> {
    ade_agents::goal::GoalStore::new(state.workspace_root())
        .list()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn goal_active(
    state: State<'_, AppState>,
) -> Result<Option<ade_agents::goal::EngGoal>, String> {
    ade_agents::goal::GoalStore::new(state.workspace_root())
        .load_active()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn goal_create(
    state: State<'_, AppState>,
    input: ade_agents::goal::GoalCreateInput,
) -> Result<ade_agents::goal::EngGoal, String> {
    ade_agents::goal::GoalStore::new(state.workspace_root())
        .create(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn goal_set_active(
    state: State<'_, AppState>,
    id: String,
) -> Result<ade_agents::goal::EngGoal, String> {
    ade_agents::goal::GoalStore::new(state.workspace_root())
        .set_active(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn goal_clear_active(state: State<'_, AppState>) -> Result<(), String> {
    ade_agents::goal::GoalStore::new(state.workspace_root())
        .clear_active()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn goal_mark_status(
    state: State<'_, AppState>,
    id: String,
    status: String,
) -> Result<ade_agents::goal::EngGoal, String> {
    ade_agents::goal::GoalStore::new(state.workspace_root())
        .mark_status(&id, &status)
        .map_err(|error| error.to_string())
}

fn parse_agent_uuid(agent_id: &str) -> Result<uuid::Uuid, String> {
    uuid::Uuid::parse_str(agent_id.trim()).map_err(|error| format!("invalid agent UUID: {error}"))
}

#[tauri::command]
pub fn lease_acquire(
    state: State<'_, AppState>,
    agent_id: String,
    path: String,
    mode: Option<String>,
    ttl_secs: Option<i64>,
) -> Result<ade_workflow::parallel::PathLease, String> {
    let agent = parse_agent_uuid(&agent_id)?;
    let mode = ade_workflow::parallel::LeaseMode::parse(mode.as_deref().unwrap_or("strong"))
        .map_err(|error| error.to_string())?;
    let ttl = chrono::Duration::seconds(ttl_secs.unwrap_or(300).max(30));
    ade_workflow::parallel::LeaseManager::new(state.workspace_root())
        .acquire(agent, path.trim(), mode, ttl)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn lease_release(state: State<'_, AppState>, lease_id: String) -> Result<bool, String> {
    ade_workflow::parallel::LeaseManager::new(state.workspace_root())
        .release(lease_id.trim())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn lease_renew(
    state: State<'_, AppState>,
    agent_id: String,
    lease_id: String,
    ttl_secs: Option<i64>,
) -> Result<ade_workflow::parallel::PathLease, String> {
    let agent = parse_agent_uuid(&agent_id)?;
    let ttl = chrono::Duration::seconds(ttl_secs.unwrap_or(300).max(30));
    ade_workflow::parallel::LeaseManager::new(state.workspace_root())
        .renew(agent, lease_id.trim(), ttl)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn task_enqueue(
    state: State<'_, AppState>,
    goal: String,
    owned_paths: Vec<String>,
    lease_mode: Option<String>,
    depends_on: Option<Vec<String>>,
) -> Result<ade_workflow::tasks::AgentTask, String> {
    let mode = ade_workflow::parallel::LeaseMode::parse(lease_mode.as_deref().unwrap_or("strong"))
        .map_err(|error| error.to_string())?;
    ade_workflow::tasks::TaskCoordinator::new(state.workspace_root())
        .enqueue(ade_workflow::tasks::EnqueueTask {
            goal,
            owned_paths,
            lease_mode: mode,
            depends_on: depends_on.unwrap_or_default(),
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn task_claim(
    state: State<'_, AppState>,
    agent_id: String,
    ttl_secs: Option<i64>,
) -> Result<Option<ade_workflow::tasks::AgentTask>, String> {
    let agent = parse_agent_uuid(&agent_id)?;
    let ttl = chrono::Duration::seconds(ttl_secs.unwrap_or(300).max(30));
    ade_workflow::tasks::TaskCoordinator::new(state.workspace_root())
        .claim(agent, ttl)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn task_claim_id(
    state: State<'_, AppState>,
    task_id: String,
    agent_id: String,
    ttl_secs: Option<i64>,
) -> Result<ade_workflow::tasks::AgentTask, String> {
    let agent = parse_agent_uuid(&agent_id)?;
    let ttl = chrono::Duration::seconds(ttl_secs.unwrap_or(300).max(30));
    ade_workflow::tasks::TaskCoordinator::new(state.workspace_root())
        .claim_id(task_id.trim(), agent, ttl)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn task_sync_from_plan(
    state: State<'_, AppState>,
) -> Result<Vec<ade_workflow::tasks::AgentTask>, String> {
    let Some(plan) =
        ade_workflow::plan_enforcement::PlanEnforcer::load_plan(state.workspace_root())
            .map_err(|error| error.to_string())?
    else {
        return Err("no PLAN on disk — run Plan from Environment first".into());
    };
    ade_workflow::tasks::TaskCoordinator::new(state.workspace_root())
        .sync_from_plan(&plan.phases)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn worktree_list(
    state: State<'_, AppState>,
) -> Result<Vec<ade_workflow::parallel::WorktreeInfo>, String> {
    ade_workflow::parallel::WorktreeManager::new(state.workspace_root())
        .list()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn worktree_provision_for_task(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<ade_workflow::parallel::WorktreeInfo, String> {
    let task_id = task_id.trim();
    if task_id.is_empty()
        || task_id.len() > 64
        || !task_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err("invalid task id for worktree".into());
    }
    let root = state.workspace_root();
    let path = root.join(".ade").join("worktrees").join(task_id);
    if path.is_dir() {
        // Reuse existing provision for this task.
        let listed = ade_workflow::parallel::WorktreeManager::new(&root)
            .list()
            .map_err(|error| error.to_string())?;
        if let Some(existing) = listed.into_iter().find(|item| {
            std::path::Path::new(&item.path) == path
                || std::path::Path::new(&item.path)
                    .canonicalize()
                    .ok()
                    .is_some_and(|canon| path.canonicalize().ok().is_some_and(|want| want == canon))
        }) {
            return Ok(existing);
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let short: String = task_id.chars().take(8).collect();
    let branch = format!("ade/task-{short}");
    ade_workflow::parallel::WorktreeManager::new(&root)
        .add(&path, &branch, None)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn worktree_remove(
    state: State<'_, AppState>,
    path: String,
    force: Option<bool>,
) -> Result<(), String> {
    ade_workflow::parallel::WorktreeManager::new(state.workspace_root())
        .remove(std::path::Path::new(path.trim()), force.unwrap_or(true))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn task_start(
    state: State<'_, AppState>,
    task_id: String,
    agent_id: String,
) -> Result<ade_workflow::tasks::AgentTask, String> {
    let agent = parse_agent_uuid(&agent_id)?;
    ade_workflow::tasks::TaskCoordinator::new(state.workspace_root())
        .start(task_id.trim(), agent)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn task_heartbeat(
    state: State<'_, AppState>,
    task_id: String,
    agent_id: String,
    ttl_secs: Option<i64>,
) -> Result<ade_workflow::tasks::AgentTask, String> {
    let agent = parse_agent_uuid(&agent_id)?;
    let ttl = chrono::Duration::seconds(ttl_secs.unwrap_or(300).max(30));
    ade_workflow::tasks::TaskCoordinator::new(state.workspace_root())
        .heartbeat(task_id.trim(), agent, ttl)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn task_complete(
    state: State<'_, AppState>,
    task_id: String,
    agent_id: String,
) -> Result<ade_workflow::tasks::AgentTask, String> {
    let agent = parse_agent_uuid(&agent_id)?;
    ade_workflow::tasks::TaskCoordinator::new(state.workspace_root())
        .complete(task_id.trim(), agent)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn task_fail(
    state: State<'_, AppState>,
    task_id: String,
    agent_id: String,
    failure: String,
) -> Result<ade_workflow::tasks::AgentTask, String> {
    let agent = parse_agent_uuid(&agent_id)?;
    ade_workflow::tasks::TaskCoordinator::new(state.workspace_root())
        .fail(task_id.trim(), agent, failure)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn handoff_resume(
    state: State<'_, AppState>,
    id: Option<String>,
) -> Result<ade_agents::handoff::HandoffResume, String> {
    let manager = ade_agents::handoff::HandoffManager::new(state.workspace_root());
    match id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        None | Some("latest") => manager.resume_latest().map_err(|error| error.to_string()),
        Some(capsule_id) => manager
            .resume_by_id(capsule_id)
            .map_err(|error| error.to_string()),
    }
}

#[derive(Serialize)]
pub struct EnsureIgnoreResult {
    pub repaired: Vec<String>,
    pub detail: String,
}

#[tauri::command]
pub fn ensure_ignore_surfaces(state: State<'_, AppState>) -> Result<EnsureIgnoreResult, String> {
    let root = state.workspace_root();
    let before = ade_core::ignore::check_alignment(&root);
    ade_core::ignore::ensure_bootstrap_ignores(&root).map_err(|error| error.to_string())?;
    let after = ade_core::ignore::check_alignment(&root);
    let repaired = before
        .iter()
        .filter(|row| {
            matches!(
                row.status,
                ade_core::ignore::IgnoreStatus::Missing | ade_core::ignore::IgnoreStatus::Drifted
            ) && matches!(
                row.surface.as_str(),
                ".gitignore" | ".cursorignore" | ".dockerignore"
            )
        })
        .filter(|row| {
            after.iter().any(|next| {
                next.surface == row.surface
                    && matches!(next.status, ade_core::ignore::IgnoreStatus::Synced)
            })
        })
        .map(|row| row.surface.clone())
        .collect::<Vec<_>>();
    Ok(EnsureIgnoreResult {
        detail: if repaired.is_empty() {
            "Ignore surfaces already aligned".into()
        } else {
            format!("Updated {} ignore surface(s)", repaired.len())
        },
        repaired,
    })
}

#[derive(Serialize)]
pub struct SpendSummary {
    pub daily_usd: f64,
    pub daily_cap_usd: f64,
    pub session_cap_usd: f64,
    pub period_key: String,
}

#[tauri::command]
pub async fn spend_summary(
    state: State<'_, AppState>,
    session_cap_usd: Option<f64>,
    daily_cap_usd: Option<f64>,
) -> Result<SpendSummary, String> {
    let root = state.workspace_root();
    let workspace = root.display().to_string();
    let caps = ade_agents::spend::SpendCaps {
        session: ade_core::money::Money::try_from_usd_f64(session_cap_usd.unwrap_or(1.0))
            .map_err(|error| error.to_string())?,
        daily: ade_core::money::Money::try_from_usd_f64(daily_cap_usd.unwrap_or(10.0))
            .map_err(|error| error.to_string())?,
    };
    let period_key = ade_agents::spend::SpendPeriod::Day.key(uuid::Uuid::nil());
    let config = ade_core::config::AdeConfig::load().map_err(|error| error.to_string())?;
    let db = ade_db::repo::AdeDatabase::open(&ade_db::repo::DbConfig::from_ade_config(&config))
        .await
        .map_err(|error| error.to_string())?;
    let ledger = ade_db::usage_ledger::UsageLedgerStore::new(
        db.connect().map_err(|error| error.to_string())?,
    );
    let daily = ledger
        .active_spend("workspace", &period_key, &workspace)
        .await
        .map_err(|error| error.to_string())?;
    Ok(SpendSummary {
        daily_usd: daily.to_usd_f64(),
        daily_cap_usd: caps.daily.to_usd_f64(),
        session_cap_usd: caps.session.to_usd_f64(),
        period_key,
    })
}

#[tauri::command]
pub async fn spend_ledger_recent(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<ade_db::usage_ledger::LedgerEntryView>, String> {
    let workspace = state.workspace_root().display().to_string();
    let config = ade_core::config::AdeConfig::load().map_err(|error| error.to_string())?;
    let db = ade_db::repo::AdeDatabase::open(&ade_db::repo::DbConfig::from_ade_config(&config))
        .await
        .map_err(|error| error.to_string())?;
    let ledger = ade_db::usage_ledger::UsageLedgerStore::new(
        db.connect().map_err(|error| error.to_string())?,
    );
    ledger
        .recent_for_workspace(&workspace, limit.unwrap_or(40))
        .await
        .map_err(|error| error.to_string())
}

const WORKSPACE_TEXT_MAX_BYTES: u64 = 1_048_576;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTextFile {
    pub path: String,
    pub absolute: String,
    pub content: String,
    pub bytes: u64,
    pub language_hint: String,
}

fn language_hint_for(path: &Path) -> String {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "rs" => "rust",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "typescript",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "javascript",
        "json" => "json",
        "md" | "mdx" => "markdown",
        "toml" => "toml",
        "yml" | "yaml" => "yaml",
        "ps1" => "powershell",
        "py" => "python",
        "css" => "css",
        "html" | "htm" => "html",
        "sh" | "bash" => "shell",
        "sql" => "sql",
        "xml" => "xml",
        _ => "plaintext",
    }
    .into()
}

/// Resolve a workspace-relative or absolute path under the attached root.
/// Blocks SensitivePathPolicy secrets / always-ignore paths and path escapes.
fn resolve_workspace_text_path(
    root: &Path,
    path: &str,
    must_exist: bool,
) -> Result<(PathBuf, String), String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("path required".into());
    }
    let root = root
        .canonicalize()
        .map_err(|error| format!("canonicalize workspace: {error}"))?;
    let raw = PathBuf::from(trimmed);
    let absolute = if raw.is_absolute() {
        raw
    } else {
        root.join(raw)
    };

    let resolved = if must_exist || absolute.exists() {
        absolute
            .canonicalize()
            .map_err(|error| format!("resolve path: {error}"))?
    } else {
        let parent = absolute
            .parent()
            .ok_or_else(|| "path has no parent".to_string())?;
        let name = absolute
            .file_name()
            .ok_or_else(|| "path has no file name".to_string())?;
        if !parent.exists() {
            return Err(format!(
                "parent folder does not exist: {}",
                parent.display()
            ));
        }
        let parent = parent
            .canonicalize()
            .map_err(|error| format!("canonicalize parent: {error}"))?;
        parent.join(name)
    };

    if !resolved.starts_with(&root) {
        return Err("path escapes workspace root".into());
    }
    let rel = resolved
        .strip_prefix(&root)
        .map_err(|_| "path escapes workspace root".to_string())?;
    let rel_str = rel.to_string_lossy().replace('\\', "/");
    if rel_str.is_empty() {
        return Err("path must be a file under the workspace".into());
    }
    if ade_core::ignore::SensitivePathPolicy::path_is_blocked(&rel_str) {
        return Err(format!(
            "path blocked by SensitivePathPolicy: {rel_str}"
        ));
    }
    Ok((resolved, rel_str))
}

#[tauri::command]
pub fn workspace_read_text(
    state: State<'_, AppState>,
    path: String,
) -> Result<WorkspaceTextFile, String> {
    let root = state.workspace_root();
    let (absolute, rel) = resolve_workspace_text_path(&root, &path, true)?;
    if !absolute.is_file() {
        return Err(format!("not a file: {rel}"));
    }
    let meta = std::fs::metadata(&absolute).map_err(|error| error.to_string())?;
    if meta.len() > WORKSPACE_TEXT_MAX_BYTES {
        return Err(format!(
            "file too large ({} bytes; max {WORKSPACE_TEXT_MAX_BYTES})",
            meta.len()
        ));
    }
    let content = std::fs::read_to_string(&absolute).map_err(|error| error.to_string())?;
    Ok(WorkspaceTextFile {
        path: rel,
        absolute: absolute.display().to_string(),
        bytes: meta.len(),
        language_hint: language_hint_for(&absolute),
        content,
    })
}

#[tauri::command]
pub fn workspace_write_text(
    state: State<'_, AppState>,
    path: String,
    content: String,
) -> Result<WorkspaceTextFile, String> {
    if content.len() as u64 > WORKSPACE_TEXT_MAX_BYTES {
        return Err(format!(
            "content too large ({} bytes; max {WORKSPACE_TEXT_MAX_BYTES})",
            content.len()
        ));
    }
    let root = state.workspace_root();
    let (absolute, rel) = resolve_workspace_text_path(&root, &path, false)?;
    if absolute.is_dir() {
        return Err(format!("path is a directory: {rel}"));
    }
    std::fs::write(&absolute, content.as_bytes()).map_err(|error| error.to_string())?;
    let bytes = content.len() as u64;
    Ok(WorkspaceTextFile {
        path: rel,
        absolute: absolute.display().to_string(),
        bytes,
        language_hint: language_hint_for(&absolute),
        content,
    })
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
        // Persist a PLAN artifact when possible, but do not block Apply when the
        // audit has nothing to own (e.g. Home/Desktop shell goals). Empty scope
        // still allows Act shell (human dial); fs__write_file stays denied unless
        // bootstrap adds activation paths below.
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

    // Always merge activation write targets (stale 0-phase PLAN must not block recipe pin).
    for path in ade_core::plan::bootstrap_apply_owned_paths(workspace_root) {
        if !paths.iter().any(|existing| existing == &path) {
            paths.push(path);
        }
    }

    paths.sort();
    paths.dedup();
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
    fn act_with_approval_bootstraps_recipe_owned_path_when_missing() {
        let root = std::env::temp_dir().join(format!("ade-own-empty-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("AGENTS.md"), "# contract\n").unwrap();
        let paths = resolve_turn_owned_paths(
            &root,
            ade_agents::autonomy::AutonomyLevel::Act,
            true,
            vec![],
        )
        .expect("Apply with empty PLAN owned_paths must not hard-fail");
        assert!(
            paths.iter().any(|path| path == ".ade/recipe.json"),
            "missing recipe must become an Apply write target: {paths:?}"
        );
        assert!(
            ade_workflow::plan_enforcement::PlanEnforcer::plan_path(&root).is_file(),
            "still persist a PLAN artifact when approving Apply"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_text_path_blocks_secrets_and_escapes() {
        let root = std::env::temp_dir().join(format!("ade-editor-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src").join("ok.rs"), "fn main() {}\n").unwrap();
        std::fs::write(root.join(".env"), "SECRET=1\n").unwrap();

        let ok = resolve_workspace_text_path(&root, "src/ok.rs", true).unwrap();
        assert_eq!(ok.1, "src/ok.rs");

        let blocked = resolve_workspace_text_path(&root, ".env", true);
        assert!(blocked.is_err(), "secrets must be blocked");

        let escape = resolve_workspace_text_path(&root, "../outside.txt", false);
        assert!(escape.is_err(), "path escape must fail");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_text_round_trip() {
        let root = std::env::temp_dir().join(format!("ade-editor-rt-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join(".ade")).unwrap();
        let (absolute, rel) =
            resolve_workspace_text_path(&root, ".ade/editor-spike.md", false).unwrap();
        assert_eq!(rel, ".ade/editor-spike.md");
        std::fs::write(&absolute, "# hello\n").unwrap();
        let (read_abs, read_rel) =
            resolve_workspace_text_path(&root, ".ade/editor-spike.md", true).unwrap();
        assert_eq!(read_rel, rel);
        assert_eq!(read_abs, absolute);
        let body = std::fs::read_to_string(&read_abs).unwrap();
        assert_eq!(body, "# hello\n");
        let _ = std::fs::remove_dir_all(root);
    }
}
