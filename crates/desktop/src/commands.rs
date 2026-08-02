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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use tauri::{ipc::Channel, AppHandle, Manager, State, Url, WebviewUrl, WebviewWindowBuilder};

pub struct AppState {
    workspace_root: RwLock<PathBuf>,
    pub mcp: McpHost,
    pub key_vault: Arc<dyn ade_db::secrets::ProviderKeyVault>,
    pub pty: crate::pty::PtyHub,
    /// Cancel flag for the in-flight `run_agent_turn` (if any).
    pub turn_cancel: RwLock<Option<Arc<AtomicBool>>>,
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
                "folder is not an ADE workspace yet (missing AGENTS.md). Use New workspace or Adopt first."
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
        // First-run / no preferred: land in Default scratch, not ADE source.
        // Dogfooders still use "Open ADE source" or a remembered preferred root.
        let workspace_root = configured
            .or(current)
            .or_else(|| ensure_default_workspace().ok())
            .or_else(Self::ade_source_root)
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
            turn_cancel: RwLock::new(None),
        }
    }
}

fn is_ade_workspace(root: &Path) -> bool {
    root.is_dir() && root.join("AGENTS.md").is_file()
}

/// Machine-local ADE home (`%LOCALAPPDATA%/ade` or `~/.local/share/ade`).
fn ade_machine_home() -> Option<PathBuf> {
    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        return Some(PathBuf::from(base).join("ade"));
    }
    user_home_dir().map(|home| home.join(".local").join("share").join("ade"))
}

fn preferred_workspace_path() -> Option<PathBuf> {
    ade_machine_home().map(|base| base.join("workspace-root.txt"))
}

fn recent_workspaces_path() -> Option<PathBuf> {
    ade_machine_home().map(|base| base.join("recent-workspaces.json"))
}

/// Built-in scratch workspace: `%LOCALAPPDATA%/ade/workspaces/Default`.
fn default_workspace_path() -> Option<PathBuf> {
    ade_machine_home().map(|base| base.join("workspaces").join("Default"))
}

fn is_default_workspace(root: &Path) -> bool {
    default_workspace_path().is_some_and(|default| same_path(&default, root))
}

fn default_agents_md(root: &Path) -> String {
    format!(
        r#"# Default — Scratch workspace

This is ADE's built-in starting folder. Use it to try chats, paste files, and explore before you create a real project.

## Golden Path

- **Root:** `{root}`
- For a new app/demo website: call **workspace__create_named**, then write files under **Apply** in the next turn — do not dump full project blueprints into chat.
- Open a local server with **browser__open** and `http://localhost:PORT`.
- Or use **New project…** / **Open folder…** under Workspaces.

## Notes

- Chat and `.ade/` state here are local to this scratch pad.
- Switching to Default never deletes your other project folders.
"#,
        root = root.display()
    )
}

/// Ensure the Default scratch workspace exists (AGENTS.md + `.ade/` identity).
fn ensure_default_workspace() -> Result<PathBuf, String> {
    let root = default_workspace_path()
        .ok_or_else(|| "could not resolve Default workspace path".to_string())?;
    if !root.exists() {
        std::fs::create_dir_all(&root)
            .map_err(|error| format!("create Default workspace: {error}"))?;
    }
    if !root.is_dir() {
        return Err(format!(
            "Default workspace path is not a directory: {}",
            root.display()
        ));
    }
    let agents = root.join("AGENTS.md");
    if !agents.is_file() {
        std::fs::write(&agents, default_agents_md(&root))
            .map_err(|error| format!("write Default AGENTS.md: {error}"))?;
    }
    ensure_workspace_identity(&root, "Default")?;
    Ok(root.canonicalize().unwrap_or(root))
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

fn user_home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

fn user_desktop_dir() -> Option<PathBuf> {
    let home = user_home_dir()?;
    let desktop = home.join("Desktop");
    if desktop.is_dir() {
        Some(desktop)
    } else {
        None
    }
}

/// Default parent for New workspace: Desktop when present, else home.
fn default_workspace_parent() -> Option<PathBuf> {
    user_desktop_dir().or_else(user_home_dir)
}

/// Folder name only — no path separators, no `..`, Windows-illegal chars stripped.
fn sanitize_workspace_name(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("workspace name is required".into());
    }
    if trimmed.len() > 64 {
        return Err("workspace name must be 64 characters or fewer".into());
    }
    if trimmed == "." || trimmed == ".." {
        return Err("workspace name cannot be '.' or '..'".into());
    }
    if trimmed.contains(['/', '\\', ':']) || trimmed.contains("..") {
        return Err("workspace name cannot contain path separators or '..'".into());
    }
    let cleaned: String = trimmed
        .chars()
        .map(|ch| match ch {
            '<' | '>' | '"' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.');
    if cleaned.is_empty() {
        return Err("workspace name is empty after sanitizing".into());
    }
    Ok(cleaned.to_string())
}

/// Persist workspace identity for attach + future multi-agent orchestrator binding.
fn ensure_workspace_identity(root: &Path, project_name: &str) -> Result<(), String> {
    let ade_dir = root.join(".ade");
    if !ade_dir.is_dir() {
        std::fs::create_dir_all(&ade_dir).map_err(|error| format!("create .ade: {error}"))?;
    }
    let identity_path = ade_dir.join("workspace.json");
    if identity_path.is_file() {
        return Ok(());
    }
    let payload = serde_json::json!({
        "schema": "ade.workspace/v1",
        "id": uuid::Uuid::new_v4().to_string(),
        "name": project_name,
        "created_at": chrono::Utc::now().to_rfc3339(),
        "created_by": "desktop",
    });
    std::fs::write(
        &identity_path,
        serde_json::to_string_pretty(&payload).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("write workspace.json: {error}"))?;

    // Primary session slot — Orchestrator can later add worker entries beside this.
    let session_dir = ade_dir.join("session");
    std::fs::create_dir_all(&session_dir)
        .map_err(|error| format!("create .ade/session: {error}"))?;
    let active_session = session_dir.join("active.json");
    if !active_session.is_file() {
        let session = serde_json::json!({
            "schema": "ade.session/v1",
            "session_id": uuid::Uuid::new_v4().to_string(),
            "role": "primary",
            "workspace_id": payload.get("id").and_then(|v| v.as_str()),
            "created_at": chrono::Utc::now().to_rfc3339(),
        });
        std::fs::write(
            &active_session,
            serde_json::to_string_pretty(&session).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("write session/active.json: {error}"))?;
    }
    Ok(())
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
    pub is_default: bool,
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
        is_default: is_default_workspace(&workspace_root),
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
    ade_db::secrets::KNOWN_PROVIDERS
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
    pub is_default: bool,
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
pub fn open_default_workspace(state: State<'_, AppState>) -> Result<DogfoodOpenResult, String> {
    let root = ensure_default_workspace()?;
    let root = state.set_workspace_root(root)?;
    Ok(DogfoodOpenResult {
        workspace_root: root.display().to_string(),
        already_dogfood: false,
    })
}

#[tauri::command]
pub fn list_workspaces(state: State<'_, AppState>) -> Result<WorkspaceList, String> {
    let current = state.workspace_root();
    let ade_source = AppState::ade_source_root();
    let default_root = ensure_default_workspace().ok();
    let mut paths: Vec<PathBuf> = Vec::new();
    paths.push(current.clone());
    if let Some(default) = &default_root {
        if !paths.iter().any(|item| same_path(item, default)) {
            paths.push(default.clone());
        }
    }
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
                name: if is_default_workspace(&path) {
                    "Default".to_string()
                } else {
                    workspace_display_name(&path)
                },
                is_current: same_path(&path, &current),
                is_ade_source: ade_source
                    .as_ref()
                    .is_some_and(|source| same_path(source, &path)),
                is_default: is_default_workspace(&path),
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
            "folder is not an ADE workspace yet (missing AGENTS.md). Use New workspace or Adopt first."
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceCreateDefaults {
    pub parent: String,
    pub desktop: Option<String>,
    pub home: Option<String>,
}

/// Parent folder defaults for New workspace (Desktop when available).
#[tauri::command]
pub fn workspace_create_defaults() -> Result<WorkspaceCreateDefaults, String> {
    let home = user_home_dir().map(|path| path.display().to_string());
    let desktop = user_desktop_dir().map(|path| path.display().to_string());
    let parent = default_workspace_parent()
        .map(|path| path.display().to_string())
        .or_else(|| home.clone())
        .ok_or_else(|| "could not resolve home/Desktop for new workspaces".to_string())?;
    Ok(WorkspaceCreateDefaults {
        parent,
        desktop,
        home,
    })
}

/// Create `{parent}/{name}`, write AGENTS.md + identity, attach, return root.
#[tauri::command]
pub fn create_named_workspace(
    state: State<'_, AppState>,
    name: String,
    parent: Option<String>,
    force: Option<bool>,
) -> Result<DogfoodOpenResult, String> {
    let name = sanitize_workspace_name(&name)?;
    let parent_path = match parent
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(raw) => PathBuf::from(raw),
        None => default_workspace_parent()
            .ok_or_else(|| "could not resolve Desktop/home parent folder".to_string())?,
    };
    if !parent_path.exists() {
        std::fs::create_dir_all(&parent_path)
            .map_err(|error| format!("create parent folder: {error}"))?;
    }
    if !parent_path.is_dir() {
        return Err(format!(
            "parent is not a directory: {}",
            parent_path.display()
        ));
    }
    let root = parent_path.join(&name);
    if root.exists() && !force.unwrap_or(false) {
        if is_ade_workspace(&root) {
            return Err(format!(
                "workspace already exists at {}. Use Open or Adopt, or choose another name.",
                root.display()
            ));
        }
        if root.is_dir() {
            // Empty-ish folder: adopt into it via create_workspace path below.
        } else {
            return Err(format!(
                "path exists and is not a folder: {}",
                root.display()
            ));
        }
    }
    create_workspace(state, root.display().to_string(), Some(name), force)
}

/// Adopt a folder as an ADE workspace: write AGENTS.md (unless present), ensure `.ade/`, then open.
/// Also creates the folder when `path` does not exist (used by Create / named create).
#[tauri::command]
pub fn create_workspace(
    state: State<'_, AppState>,
    path: String,
    project_name: Option<String>,
    force: Option<bool>,
) -> Result<DogfoodOpenResult, String> {
    let root = PathBuf::from(path.trim());
    if root.as_os_str().is_empty() {
        return Err("workspace path is required".into());
    }
    if !root.exists() {
        std::fs::create_dir_all(&root).map_err(|error| format!("create folder: {error}"))?;
    }
    if !root.is_dir() {
        return Err(format!("not a directory: {}", root.display()));
    }
    let canonical = root
        .canonicalize()
        .map_err(|error| format!("canonicalize: {error}"))?;
    let name = match project_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(raw) => {
            sanitize_workspace_name(raw).unwrap_or_else(|_| workspace_display_name(&canonical))
        }
        None => workspace_display_name(&canonical),
    };
    let agents = canonical.join("AGENTS.md");
    let force = force.unwrap_or(false);
    if agents.is_file() && !force {
        // Already a workspace — just open / refresh identity.
    } else {
        std::fs::write(&agents, minimal_agents_md(&name, &canonical))
            .map_err(|error| format!("write AGENTS.md: {error}"))?;
    }
    ensure_workspace_identity(&canonical, &name)?;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvKeyCandidate {
    pub provider: String,
    pub env_var: String,
}

/// List known BYOK providers and whether a vault credential exists (never returns secrets).
#[tauri::command]
pub fn key_status_all(
    state: State<'_, AppState>,
    profile: Option<String>,
) -> Result<Vec<ProviderVaultRow>, String> {
    let profile = resolve_profile(profile)?;
    let mut rows = Vec::with_capacity(ade_db::secrets::KNOWN_PROVIDERS.len());
    for provider in ade_db::secrets::KNOWN_PROVIDERS {
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

/// Which free/BYOK env vars are present (names only — never secret values).
#[tauri::command]
pub fn key_env_candidates() -> Result<Vec<EnvKeyCandidate>, String> {
    Ok(ade_db::secrets::list_env_key_candidates()
        .into_iter()
        .map(|(provider, env_var)| EnvKeyCandidate { provider, env_var })
        .collect())
}

/// Import free/BYOK keys from process env into the OS vault (explicit user action).
#[tauri::command]
pub fn key_import_env(
    state: State<'_, AppState>,
    profile: Option<String>,
    force: Option<bool>,
    provider: Option<String>,
) -> Result<OpenCodeImportResult, String> {
    let profile = resolve_profile(profile)?;
    let force = force.unwrap_or(false);
    let only = provider
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase());
    let before: Vec<(String, String)> = ade_db::secrets::list_env_key_candidates()
        .into_iter()
        .filter(|(id, _)| only.as_ref().is_none_or(|want| want == id))
        .collect();
    let imported = ade_db::secrets::import_env_provider_keys_explicit(
        state.key_vault.as_ref(),
        &profile,
        force,
        only.as_deref(),
    )
    .map_err(|error| error.to_string())?;
    let before_empty = before.is_empty();
    let skipped: Vec<String> = before
        .into_iter()
        .map(|(provider, _)| provider)
        .filter(|provider| !imported.contains(provider))
        .collect();
    Ok(OpenCodeImportResult {
        detail: if imported.is_empty() {
            if before_empty {
                "No provider keys found in environment (set GROQ_API_KEY, FREELMAPI_KEY, …)".into()
            } else {
                "Env keys already in vault (pass force to overwrite)".into()
            }
        } else {
            format!(
                "Imported {} provider key(s) from environment",
                imported.len()
            )
        },
        imported,
        skipped,
    })
}

/// Activate a keyless free gateway by storing a local sentinel (never a real secret).
#[tauri::command]
pub fn key_activate_keyless(
    state: State<'_, AppState>,
    profile: Option<String>,
    provider: String,
) -> Result<ProviderKeyStatus, String> {
    let profile = resolve_profile(profile)?;
    ade_db::secrets::activate_keyless_provider(state.key_vault.as_ref(), &profile, &provider)
        .map_err(|error| error.to_string())?;
    key_status_for(state.key_vault.as_ref(), &profile, provider.trim())
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
    recipe_id: Option<String>,
    vault_provider: Option<String>,
    vault_env_keys: Option<Vec<String>>,
) -> Result<(), String> {
    ade_agents::mcp::authorize_mcp_connect(
        recipe_id.as_deref(),
        &name,
        &command,
        &args,
        approved,
    )
    .map_err(|error| error.to_string())?;

    let mut env = std::collections::BTreeMap::new();
    if let Some(provider) = vault_provider
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let keys = vault_env_keys.unwrap_or_default();
        if !keys.is_empty() {
            let profile = resolve_profile(None)?;
            let secret = state
                .key_vault
                .get(&profile, provider)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| {
                    format!("Save a {provider} token under Integrations before Connect MCP")
                })?;
            for key in keys {
                let trimmed = key.trim();
                if !trimmed.is_empty() {
                    env.insert(trimmed.to_string(), secret.clone());
                }
            }
        }
    }
    // Authorization above is the trust root; never pass through client bool alone.
    state
        .mcp
        .connect_server(McpServerConfig {
            name,
            command,
            args,
            env,
            approved: true,
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

/// Bundled turn request — keeps Channel as a sibling IPC arg so bools like
/// `allowUnpriced` cannot be confused with the Channel map payload.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunAgentTurnArgs {
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
    /// G4: optional isolated checkout; leases/PLAN stay on primary via coordination_root.
    execution_root: Option<String>,
    allow_unpriced: Option<bool>,
    approved_risk_categories: Option<Vec<String>>,
    approved_risk_tiers: Option<Vec<String>>,
    claimed_task_id: Option<String>,
    waive_queue: Option<bool>,
    slot_override: Option<String>,
    /// Absolute or workspace-relative image paths for multimodal content.
    image_paths: Option<Vec<String>>,
}

#[tauri::command]
pub async fn run_agent_turn(
    state: State<'_, AppState>,
    args: RunAgentTurnArgs,
    on_event: Channel<AgentEvent>,
) -> Result<(), String> {
    let RunAgentTurnArgs {
        prompt,
        provider,
        base_url,
        model,
        input_cost_per_mtok,
        output_cost_per_mtok,
        context_limit,
        output_limit,
        session_cap_usd,
        daily_cap_usd,
        profile,
        lease_agent_id,
        autonomy,
        max_steps,
        max_tokens,
        verify_on_complete,
        verify_gate,
        approve_owned_paths,
        owned_paths,
        preferred_shell_cwd,
        execution_root,
        allow_unpriced,
        approved_risk_categories,
        approved_risk_tiers,
        claimed_task_id,
        waive_queue,
        slot_override,
        image_paths,
    } = args;

    if prompt.trim().is_empty() {
        return Err("prompt cannot be empty".into());
    }
    let profile = match profile {
        Some(profile) if !profile.trim().is_empty() => profile.trim().to_ascii_lowercase(),
        _ => "local".into(),
    };
    let mut autonomy = autonomy
        .as_deref()
        .unwrap_or("propose")
        .parse::<ade_agents::autonomy::AutonomyLevel>()
        .map_err(|error| error.to_string())?;
    let slot_override = match slot_override
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(raw) => {
            Some(ade_agents::slots::SlotRole::parse(raw).map_err(|error| error.to_string())?)
        }
        None => None,
    };
    if matches!(slot_override, Some(ade_agents::slots::SlotRole::Verifier))
        && autonomy.allows_mutating_tools()
    {
        autonomy = ade_agents::autonomy::AutonomyLevel::Propose;
    }
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
    ade_agents::spend::require_priced_for_caps_with_override(
        &spend_caps,
        input_cost,
        output_cost,
        allow_unpriced.unwrap_or(false),
    )
    .map_err(|error| error.to_string())?;

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
            // Skip when Verifier override (sensors-only).
            if approve_owned_paths.unwrap_or(false)
                && !owned_paths.is_empty()
                && !matches!(slot_override, Some(ade_agents::slots::SlotRole::Verifier))
            {
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
        image_paths: image_paths.unwrap_or_default(),
    })
    .mcp(state.mcp.clone())
    .ledger(ledger)
    .spend_caps(spend_caps)
    .key_vault(Arc::clone(&state.key_vault))
    .autonomy(autonomy)
    .max_tool_rounds(max_steps.unwrap_or(32) as usize)
    .max_tokens(max_tokens)
    .verify_on_complete(verify_on_complete)
    .preferred_shell_cwd(preferred_shell_cwd)
    .approved_risk_categories(approved_risk_categories.unwrap_or_default())
    .approved_risk_tiers(approved_risk_tiers.unwrap_or_default())
    .claimed_task_id(claimed_task_id)
    .waive_queue(waive_queue.unwrap_or(false))
    .slot_override(slot_override)
    .allow_unpriced(allow_unpriced.unwrap_or(false));
    if execution_root != primary_root {
        builder = builder.coordination_root(primary_root);
    }
    if let Some(agent_id) = lease_agent {
        if !matches!(slot_override, Some(ade_agents::slots::SlotRole::Verifier)) {
            builder = builder.lease_agent(agent_id);
        }
    }
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut slot = state
            .turn_cancel
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *slot = Some(Arc::clone(&cancel));
    }
    builder = builder.cancel_flag(Arc::clone(&cancel));
    let service = match builder.prepare().await {
        Ok(service) => service,
        Err(error) => {
            let mut slot = state
                .turn_cancel
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *slot = None;
            return Err(error.to_string());
        }
    };

    let mut events = service.start();
    while let Some(event) = events.recv().await {
        let terminal = matches!(
            event,
            AgentEvent::Failed { .. } | AgentEvent::Cancelled { .. } | AgentEvent::Completed { .. }
        );
        let send_result = on_event.send(event).map_err(|error| error.to_string());
        if terminal {
            let mut slot = state
                .turn_cancel
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *slot = None;
        }
        send_result?;
        if terminal {
            break;
        }
    }
    {
        let mut slot = state
            .turn_cancel
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *slot = None;
    }
    Ok(())
}

#[tauri::command]
pub fn cancel_agent_turn(state: State<'_, AppState>) -> bool {
    let guard = state
        .turn_cancel
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(flag) = guard.as_ref() {
        flag.store(true, Ordering::Relaxed);
        true
    } else {
        false
    }
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
    if trimmed.eq_ignore_ascii_case("about:blank") {
        return Url::parse("about:blank").map_err(|error| format!("invalid url: {error}"));
    }
    let local_host = trimmed.starts_with("localhost")
        || trimmed.starts_with("127.0.0.1")
        || trimmed.starts_with("[::1]");
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else if local_host {
        format!("http://{trimmed}")
    } else if trimmed.contains('.') {
        format!("https://{trimmed}")
    } else {
        format!(
            "https://www.google.com/search?q={}",
            urlencoding_fallback(trimmed)
        )
    };
    let parsed = Url::parse(&with_scheme).map_err(|error| format!("invalid url: {error}"))?;
    if !matches!(parsed.scheme(), "http" | "https" | "about") {
        return Err("only http, https, and about:blank are allowed".into());
    }
    Ok(parsed)
}

fn urlencoding_fallback(raw: &str) -> String {
    raw.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

fn resolve_browser_label(label: Option<String>) -> String {
    let trimmed = label.unwrap_or_default();
    let trimmed = trimmed.trim();
    if trimmed.is_empty() {
        return BROWSER_WINDOW_LABEL.to_string();
    }
    if trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        trimmed.to_string()
    } else {
        BROWSER_WINDOW_LABEL.to_string()
    }
}

fn main_window(app: &AppHandle) -> Result<tauri::Window, String> {
    if let Some(window) = app.get_window("main") {
        return Ok(window);
    }
    let webview = app
        .get_webview_window("main")
        .ok_or_else(|| "main window missing".to_string())?;
    Ok(webview.as_ref().window())
}

/// Embed (or update) a Chromium/WebView2 pane inside the main ADE window.
/// Async on purpose: sync commands run on the UI thread; `Window::add_child`
/// also schedules on that thread and would deadlock if called synchronously.
#[tauri::command]
pub async fn browser_embed(
    app: AppHandle,
    label: String,
    url: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<String, String> {
    let label = resolve_browser_label(Some(label));
    let parsed = parse_browser_url(&url)?;
    let width = width.max(32.0);
    let height = height.max(32.0);
    let position = tauri::LogicalPosition::new(x.max(0.0), y.max(0.0));
    let size = tauri::LogicalSize::new(width, height);

    // Close legacy separate-window browsers with the same label.
    if let Some(window) = app.get_webview_window(&label) {
        if window.label() == label && app.get_webview(&label).is_none() {
            let _ = window.close();
        }
    }

    if let Some(webview) = app.get_webview(&label) {
        webview
            .set_position(position)
            .map_err(|error| format!("browser position: {error}"))?;
        webview
            .set_size(size)
            .map_err(|error| format!("browser size: {error}"))?;
        webview
            .navigate(parsed.clone())
            .map_err(|error| format!("browser navigate: {error}"))?;
        let _ = webview.show();
        return Ok(parsed.to_string());
    }

    let parent = main_window(&app)?;
    let builder = tauri::webview::WebviewBuilder::new(&label, WebviewUrl::External(parsed.clone()));
    parent
        .add_child(builder, position, size)
        .map_err(|error| format!("embed browser: {error}"))?;
    Ok(parsed.to_string())
}

#[tauri::command]
pub fn browser_set_bounds(
    app: AppHandle,
    label: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let label = resolve_browser_label(Some(label));
    let Some(webview) = app.get_webview(&label) else {
        return Ok(());
    };
    webview
        .set_position(tauri::LogicalPosition::new(x.max(0.0), y.max(0.0)))
        .map_err(|error| format!("browser position: {error}"))?;
    webview
        .set_size(tauri::LogicalSize::new(width.max(32.0), height.max(32.0)))
        .map_err(|error| format!("browser size: {error}"))?;
    Ok(())
}

#[tauri::command]
pub fn browser_navigate(app: AppHandle, label: String, url: String) -> Result<String, String> {
    let label = resolve_browser_label(Some(label));
    let parsed = parse_browser_url(&url)?;
    let webview = app
        .get_webview(&label)
        .ok_or_else(|| "browser pane not found".to_string())?;
    webview
        .navigate(parsed.clone())
        .map_err(|error| format!("browser navigate: {error}"))?;
    Ok(parsed.to_string())
}

#[tauri::command]
pub fn browser_set_visible(app: AppHandle, label: String, visible: bool) -> Result<(), String> {
    let label = resolve_browser_label(Some(label));
    let Some(webview) = app.get_webview(&label) else {
        return Ok(());
    };
    if visible {
        webview
            .show()
            .map_err(|error| format!("browser show: {error}"))?;
    } else {
        webview
            .hide()
            .map_err(|error| format!("browser hide: {error}"))?;
    }
    Ok(())
}

/// Open (or focus + navigate) a dedicated Chromium/WebView2 window for browsing.
/// Kept for compatibility; prefer [`browser_embed`] for in-shell browsing.
#[tauri::command]
pub fn open_browser_window(
    app: AppHandle,
    url: String,
    label: Option<String>,
) -> Result<String, String> {
    let parsed = parse_browser_url(&url)?;
    let window_label = resolve_browser_label(label);
    if let Some(window) = app.get_webview_window(&window_label) {
        window
            .navigate(parsed.clone())
            .map_err(|error| format!("navigate browser: {error}"))?;
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(parsed.to_string());
    }
    WebviewWindowBuilder::new(&app, &window_label, WebviewUrl::External(parsed.clone()))
        .title("ADE Browser")
        .inner_size(1180.0, 820.0)
        .resizable(true)
        .build()
        .map_err(|error| format!("open browser: {error}"))?;
    Ok(parsed.to_string())
}

#[tauri::command]
pub fn browser_window_url(app: AppHandle, label: Option<String>) -> Result<Option<String>, String> {
    let window_label = resolve_browser_label(label.clone());
    if let Some(webview) = app.get_webview(&window_label) {
        return webview
            .url()
            .map(|url| Some(url.to_string()))
            .map_err(|error| format!("browser url: {error}"));
    }
    let Some(window) = app.get_webview_window(&window_label) else {
        return Ok(None);
    };
    window
        .url()
        .map(|url| Some(url.to_string()))
        .map_err(|error| format!("browser url: {error}"))
}

#[tauri::command]
pub fn close_browser_window(app: AppHandle, label: Option<String>) -> Result<(), String> {
    let window_label = resolve_browser_label(label);
    if let Some(webview) = app.get_webview(&window_label) {
        webview
            .close()
            .map_err(|error| format!("close browser: {error}"))?;
        return Ok(());
    }
    if let Some(window) = app.get_webview_window(&window_label) {
        window
            .close()
            .map_err(|error| format!("close browser: {error}"))?;
    }
    Ok(())
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

/// Z1/Z2: open the attached workspace in Zed (coding eyes). Soft shell stays `ade acp`.
#[tauri::command]
pub fn open_in_zed(state: State<'_, AppState>) -> Result<String, String> {
    let cwd = state.workspace_root();
    let cwd_display = cwd.display().to_string();

    #[cfg(windows)]
    {
        let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let candidates = [
            format!("{local}\\Programs\\Zed\\Zed.exe"),
            "zed".into(),
            "Zed.exe".into(),
        ];
        for bin in candidates {
            if bin.contains('\\') && !std::path::Path::new(&bin).is_file() {
                continue;
            }
            if std::process::Command::new(&bin)
                .arg(&cwd_display)
                .spawn()
                .is_ok()
            {
                return Ok(cwd_display);
            }
        }
        return Err("Zed not found. Install Zed or add it to PATH, then retry Open in Zed.".into());
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-a", "Zed", cwd.as_os_str()])
            .spawn()
            .map_err(|error| format!("open Zed: {error}"))?;
        return Ok(cwd_display);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        for bin in ["zed", "zeditor"] {
            if std::process::Command::new(bin)
                .arg(&cwd_display)
                .spawn()
                .is_ok()
            {
                return Ok(cwd_display);
            }
        }
        return Err("Zed not found on PATH (tried zed, zeditor)".into());
    }

    #[allow(unreachable_code)]
    Err("open_in_zed unsupported on this platform".into())
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

#[tauri::command]
pub fn goal_update_contract(
    state: State<'_, AppState>,
    id: String,
    success_criteria: Option<Vec<String>>,
    out_of_scope: Option<Vec<String>>,
    verify_gate: Option<Option<String>>,
    clarify_resolutions: Option<Vec<String>>,
) -> Result<ade_agents::goal::EngGoal, String> {
    ade_agents::goal::GoalStore::new(state.workspace_root())
        .update_contract(
            &id,
            success_criteria,
            out_of_scope,
            verify_gate,
            clarify_resolutions,
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn goal_waive_contract(
    state: State<'_, AppState>,
    id: String,
    reason: String,
) -> Result<ade_agents::goal::EngGoal, String> {
    ade_agents::goal::GoalStore::new(state.workspace_root())
        .waive_contract(&id, &reason)
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
    autonomy: Option<String>,
) -> Result<ade_workflow::parallel::PathLease, String> {
    let slot = match autonomy.as_deref() {
        Some(raw) => {
            let level = raw
                .parse::<ade_agents::autonomy::AutonomyLevel>()
                .map_err(|error| error.to_string())?;
            ade_agents::slots::SlotRole::from_autonomy(level)
        }
        None => ade_agents::slots::SlotRole::Worker,
    };
    slot.require_write_lease()
        .map_err(|error| error.to_string())?;
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
    autonomy: Option<String>,
) -> Result<Option<ade_workflow::tasks::AgentTask>, String> {
    let slot = match autonomy.as_deref() {
        Some(raw) => {
            let level = raw
                .parse::<ade_agents::autonomy::AutonomyLevel>()
                .map_err(|error| error.to_string())?;
            ade_agents::slots::SlotRole::from_autonomy(level)
        }
        None => ade_agents::slots::SlotRole::Worker,
    };
    slot.require_claim_tasks()
        .map_err(|error| error.to_string())?;
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
    autonomy: Option<String>,
) -> Result<ade_workflow::tasks::AgentTask, String> {
    let slot = match autonomy.as_deref() {
        Some(raw) => {
            let level = raw
                .parse::<ade_agents::autonomy::AutonomyLevel>()
                .map_err(|error| error.to_string())?;
            ade_agents::slots::SlotRole::from_autonomy(level)
        }
        None => ade_agents::slots::SlotRole::Worker,
    };
    slot.require_claim_tasks()
        .map_err(|error| error.to_string())?;
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

/// Resume Continuity from a handoff capsule.
/// When `host_run_next` is true (default), host-runs `ade …` next_safe_command
/// before building the thrift resume prompt. Editor handoff-diff should pass
/// `false` — it only needs capsule paths.
#[tauri::command]
pub async fn handoff_resume(
    state: State<'_, AppState>,
    id: Option<String>,
    host_run_next: Option<bool>,
) -> Result<ade_agents::handoff::HandoffResume, String> {
    let root = state.workspace_root();
    let manager = ade_agents::handoff::HandoffManager::new(&root);
    let capsule_id = id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let (id_label, capsule) = match capsule_id {
        None | Some("latest") => match manager.load_latest() {
            Ok(capsule) => ("latest".to_string(), capsule),
            Err(ade_core::error::AdeError::NotFound(_)) => {
                return Ok(ade_agents::handoff::HandoffResume {
                    available: false,
                    id: String::new(),
                    goal: String::new(),
                    next_safe_command: String::new(),
                    turn_status: None,
                    created_at: None,
                    blockers: Vec::new(),
                    changed_paths: Vec::new(),
                    resume_prompt: String::new(),
                    host_ran_next: false,
                    host_exit_code: None,
                });
            }
            Err(error) => return Err(error.to_string()),
        },
        Some(capsule_id) => {
            let capsule = manager
                .load_capsule(capsule_id)
                .map_err(|error| error.to_string())?;
            (capsule_id.to_string(), capsule)
        }
    };
    let next = capsule
        .next_safe_command
        .clone()
        .unwrap_or_else(|| "ade audit".into());
    let do_host = host_run_next.unwrap_or(true);
    let (host_ran_next, host_exit_code) = if do_host {
        let workspace = root.clone();
        let command = next.clone();
        tauri::async_runtime::spawn_blocking(move || {
            ade_agents::handoff::host_run_next_safe_command(
                &workspace,
                &command,
                std::time::Duration::from_secs(45),
            )
        })
        .await
        .map_err(|error| format!("host next_safe_command task failed: {error}"))?
    } else {
        (false, None)
    };
    Ok(manager.resume_from_capsule_with(&id_label, &capsule, host_ran_next, host_exit_code))
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
    /// Backward-compatible alias for used + reserved (what caps gate against).
    pub daily_usd: f64,
    /// Committed actuals for the workspace day (invoice-class used $).
    pub used_usd: f64,
    /// Still-open reserved estimates for the workspace day.
    pub reserved_usd: f64,
    /// daily_cap − active (floored at 0).
    pub remaining_usd: f64,
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
    let breakdown = ledger
        .active_spend_breakdown("workspace", &period_key, &workspace)
        .await
        .map_err(|error| error.to_string())?;
    let remaining = caps.daily.saturating_sub(breakdown.active);
    Ok(SpendSummary {
        daily_usd: breakdown.active.to_usd_f64(),
        used_usd: breakdown.used.to_usd_f64(),
        reserved_usd: breakdown.reserved.to_usd_f64(),
        remaining_usd: remaining.to_usd_f64(),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuityActionRow {
    pub at: Option<String>,
    pub server: String,
    pub tool: String,
    pub effect: String,
    pub paths: Vec<String>,
    pub autonomy: String,
    pub risk_tier: Option<String>,
    pub risk_category: Option<String>,
}

/// E1: recent authorized action envelopes from `.ade/continuity/last-actions.json`.
#[tauri::command]
pub fn continuity_actions_recent(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<ContinuityActionRow>, String> {
    let path = state
        .workspace_root()
        .join(".ade")
        .join("continuity")
        .join("last-actions.json");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("last-actions JSON: {e}"))?;
    let actions = value
        .get("actions")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let take = limit.unwrap_or(24) as usize;
    let start = actions.len().saturating_sub(take);
    Ok(actions[start..]
        .iter()
        .rev()
        .map(|item| ContinuityActionRow {
            at: item.get("at").and_then(|v| v.as_str()).map(str::to_string),
            server: item
                .get("server")
                .and_then(|v| v.as_str())
                .unwrap_or("-")
                .into(),
            tool: item
                .get("tool")
                .and_then(|v| v.as_str())
                .unwrap_or("-")
                .into(),
            effect: item
                .get("effect")
                .and_then(|v| v.as_str())
                .unwrap_or("-")
                .into(),
            paths: item
                .get("paths")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|p| p.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            autonomy: item
                .get("autonomy")
                .and_then(|v| v.as_str())
                .unwrap_or("-")
                .into(),
            risk_tier: item
                .get("riskTier")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            risk_category: item
                .get("riskCategory")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        })
        .collect())
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
        return Err(format!("path blocked by SensitivePathPolicy: {rel_str}"));
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTextDiff {
    pub path: String,
    pub absolute: String,
    pub original: String,
    pub modified: String,
    pub language_hint: String,
    /// `head` when original came from `git show HEAD:path`; `empty` when new/untracked.
    pub baseline: String,
}

fn git_head_text(root: &Path, rel: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["show", &format!("HEAD:{rel}")])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let body = String::from_utf8_lossy(&output.stdout).into_owned();
    if body.len() as u64 > WORKSPACE_TEXT_MAX_BYTES {
        return None;
    }
    Some(body)
}

/// Working-tree text vs git HEAD (or empty baseline for new/untracked files).
#[tauri::command]
pub fn workspace_text_diff(
    state: State<'_, AppState>,
    path: String,
) -> Result<WorkspaceTextDiff, String> {
    let root = state.workspace_root();
    let file = workspace_read_text(state, path)?;
    let (original, baseline) = match git_head_text(&root, &file.path) {
        Some(body) => (body, "head".to_string()),
        None => (String::new(), "empty".to_string()),
    };
    Ok(WorkspaceTextDiff {
        path: file.path,
        absolute: file.absolute,
        original,
        modified: file.content,
        language_hint: file.language_hint,
        baseline,
    })
}

const CHAT_INBOX_MAX_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedAttachment {
    pub name: String,
    pub path: String,
    pub absolute: String,
    pub bytes: u64,
    pub staged: bool,
    #[serde(default)]
    pub is_dir: bool,
}

fn sanitize_inbox_name(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "file".into()
    } else {
        cleaned
    }
}

fn chat_inbox_dir(root: &Path) -> Result<PathBuf, String> {
    let dir = root.join(".ade").join("inbox");
    std::fs::create_dir_all(&dir).map_err(|error| format!("create .ade/inbox: {error}"))?;
    Ok(dir)
}

fn path_under_root(root: &Path, candidate: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let Ok(cand) = candidate.canonicalize() else {
        return false;
    };
    cand.starts_with(&root)
}

/// Resolve extract/transcribe sources under the workspace (canonicalize + containment).
/// Outside-workspace files must be staged into `.ade/inbox/` first.
fn resolve_chat_media_source(root: &Path, source_path: &str) -> Result<(PathBuf, String), String> {
    let trimmed = source_path.trim();
    if trimmed.is_empty() {
        return Err("sourcePath required".into());
    }
    if trimmed.contains('\0') {
        return Err("sourcePath contains NUL".into());
    }
    let root = root
        .canonicalize()
        .map_err(|error| format!("canonicalize workspace: {error}"))?;
    let raw = PathBuf::from(trimmed);
    let absolute = if raw.is_absolute() {
        raw
    } else {
        root.join(&raw)
    };
    if !absolute.is_file() {
        return Err(format!("file not found: {trimmed}"));
    }
    let canonical = absolute
        .canonicalize()
        .map_err(|error| format!("resolve path: {error}"))?;
    if !canonical.starts_with(&root) {
        return Err(
            "path escapes workspace — Attach/stage the file into .ade/inbox first".into(),
        );
    }
    let rel = canonical
        .strip_prefix(&root)
        .map_err(|_| "path escapes workspace root".to_string())?;
    let rel_str = rel.to_string_lossy().replace('\\', "/");
    if ade_core::ignore::SensitivePathPolicy::path_is_blocked(&rel_str) {
        return Err(format!("path blocked by SensitivePathPolicy: {rel_str}"));
    }
    Ok((canonical, rel_str))
}

/// Stage a file for chat: keep workspace paths; copy outsiders into `.ade/inbox/`.
#[tauri::command]
pub fn chat_stage_path(
    state: State<'_, AppState>,
    source_path: String,
) -> Result<StagedAttachment, String> {
    let root = state.workspace_root();
    let source = PathBuf::from(source_path.trim());
    if source_path.trim().is_empty() {
        return Err("path required".into());
    }
    if !source.exists() {
        return Err(format!("path not found: {}", source.display()));
    }
    let meta = std::fs::metadata(&source).map_err(|error| error.to_string())?;
    let is_dir = meta.is_dir();
    if !is_dir && !meta.is_file() {
        return Err(format!("not a file or folder: {}", source.display()));
    }
    if !is_dir && meta.len() > CHAT_INBOX_MAX_BYTES {
        return Err(format!(
            "file too large ({} bytes; max {CHAT_INBOX_MAX_BYTES})",
            meta.len()
        ));
    }
    let name = source
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(if is_dir { "folder" } else { "file" })
        .to_string();
    if let Some(reason) = refuse_attachment_name(&name) {
        return Err(reason);
    }

    if path_under_root(&root, &source) {
        let absolute = source
            .canonicalize()
            .unwrap_or_else(|_| source.clone())
            .display()
            .to_string();
        let root_abs = root.canonicalize().unwrap_or_else(|_| root.clone());
        let path = PathBuf::from(&absolute)
            .strip_prefix(&root_abs)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| absolute.clone());
        return Ok(StagedAttachment {
            name,
            path,
            absolute,
            bytes: if is_dir { 0 } else { meta.len() },
            staged: false,
            is_dir,
        });
    }

    if is_dir {
        // Outside-workspace folders: attach as absolute path (no recursive copy).
        let absolute = source
            .canonicalize()
            .unwrap_or_else(|_| source.clone())
            .display()
            .to_string();
        return Ok(StagedAttachment {
            name,
            path: absolute.clone(),
            absolute,
            bytes: 0,
            staged: false,
            is_dir: true,
        });
    }

    let inbox = chat_inbox_dir(&root)?;
    let safe = sanitize_inbox_name(&name);
    let dest_name = format!("{}-{}", chrono_lite_stamp(), safe);
    let dest = inbox.join(&dest_name);
    std::fs::copy(&source, &dest).map_err(|error| format!("copy to inbox: {error}"))?;
    let absolute = dest.canonicalize().unwrap_or(dest).display().to_string();
    Ok(StagedAttachment {
        name,
        path: format!(".ade/inbox/{dest_name}"),
        absolute,
        bytes: meta.len(),
        staged: true,
        is_dir: false,
    })
}

/// Write pasted/dropped bytes into `.ade/inbox/` (no native path available).
#[tauri::command]
pub fn chat_stage_bytes(
    state: State<'_, AppState>,
    file_name: String,
    bytes: Vec<u8>,
) -> Result<StagedAttachment, String> {
    let root = state.workspace_root();
    if bytes.len() as u64 > CHAT_INBOX_MAX_BYTES {
        return Err(format!(
            "file too large ({} bytes; max {CHAT_INBOX_MAX_BYTES})",
            bytes.len()
        ));
    }
    let safe = sanitize_inbox_name(&file_name);
    if let Some(reason) = refuse_attachment_name(&safe) {
        return Err(reason);
    }
    let inbox = chat_inbox_dir(&root)?;
    let dest = inbox.join(format!("{}-{}", chrono_lite_stamp(), safe));
    std::fs::write(&dest, &bytes).map_err(|error| format!("write inbox: {error}"))?;
    let absolute = dest
        .canonicalize()
        .unwrap_or(dest.clone())
        .display()
        .to_string();
    let name = dest
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&safe)
        .to_string();
    Ok(StagedAttachment {
        name: safe.clone(),
        path: format!(".ade/inbox/{name}"),
        absolute,
        bytes: bytes.len() as u64,
        staged: true,
        is_dir: false,
    })
}

fn refuse_attachment_name(name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    if lower == ".env" || lower.starts_with(".env.") {
        return Some(format!("refused secret-looking file: {name}"));
    }
    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(
        ext.as_str(),
        "pem" | "key" | "p12" | "pfx" | "exe" | "dll" | "bat" | "cmd" | "msi" | "scr"
    ) {
        return Some(format!("refused blocked type (.{ext}): {name}"));
    }
    None
}

fn chrono_lite_stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{ms}")
}

/// Open a local path or URL with the OS default handler.
#[tauri::command]
pub fn chat_open_path(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("path required".into());
    }
    if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        #[cfg(windows)]
        {
            std::process::Command::new("cmd")
                .args(["/C", "start", "", trimmed])
                .spawn()
                .map_err(|error| error.to_string())?;
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(trimmed)
                .spawn()
                .map_err(|error| error.to_string())?;
            return Ok(());
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            std::process::Command::new("xdg-open")
                .arg(trimmed)
                .spawn()
                .map_err(|error| error.to_string())?;
            return Ok(());
        }
    }
    let raw = PathBuf::from(trimmed);
    let p = if raw.is_absolute() {
        raw
    } else {
        state.workspace_root().join(raw)
    };
    if !p.exists() {
        return Err(format!("path not found: {trimmed}"));
    }
    let open_target = p.display().to_string();
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &open_target])
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&open_target)
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&open_target)
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err("chat_open_path unsupported on this platform".into())
}

/// Extract first pages of a PDF into `.ade/inbox/*.extract.md` (explicit; never auto).
#[tauri::command]
pub fn chat_extract_pdf(
    state: State<'_, AppState>,
    source_path: String,
    max_pages: Option<usize>,
) -> Result<StagedAttachment, String> {
    let root = state.workspace_root();
    let (absolute, rel_str) = resolve_chat_media_source(&root, &source_path)?;
    let ext = absolute
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext != "pdf" {
        return Err(format!("not a .pdf file: {source_path}"));
    }
    let pages = max_pages.unwrap_or(ade_agents::pdf::DEFAULT_PDF_EXTRACT_PAGES);
    let result = ade_agents::pdf::extract_pdf_text(&absolute, pages).map_err(|e| e.to_string())?;
    let name = absolute
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document.pdf");
    let markdown = ade_agents::pdf::format_extract_markdown(name, &rel_str, &result);
    let inbox = chat_inbox_dir(&root)?;
    let safe = sanitize_inbox_name(&format!(
        "{}.extract.md",
        Path::new(name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("document")
    ));
    let dest_name = format!("{}-{}", chrono_lite_stamp(), safe);
    let dest = inbox.join(&dest_name);
    std::fs::write(&dest, markdown.as_bytes()).map_err(|error| format!("write inbox: {error}"))?;
    let bytes = std::fs::metadata(&dest)
        .map(|m| m.len())
        .unwrap_or(markdown.len() as u64);
    Ok(StagedAttachment {
        name: dest_name.clone(),
        path: format!(".ade/inbox/{dest_name}"),
        absolute: dest.display().to_string(),
        bytes,
        staged: true,
        is_dir: false,
    })
}

/// Transcribe audio into `.ade/inbox/*.transcript.md` (Debug/Advanced; never auto).
/// Prefers `ADE_WHISPER_CMD` when set; else Groq/OpenAI vault key + `/audio/transcriptions`.
#[tauri::command]
pub async fn chat_transcribe_audio(
    state: State<'_, AppState>,
    source_path: String,
    provider: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
) -> Result<StagedAttachment, String> {
    let root = state.workspace_root();
    let (absolute, rel_str) = resolve_chat_media_source(&root, &source_path)?;
    ade_agents::audio::validate_audio_file(&absolute).map_err(|e| e.to_string())?;

    let result = if ade_agents::audio::local_whisper_cmd_configured() {
        ade_agents::audio::transcribe_local(&absolute).map_err(|e| e.to_string())?
    } else {
        let profile = "local".to_string();
        let provider_id = resolve_whisper_provider(
            state.key_vault.as_ref(),
            &profile,
            provider.as_deref(),
        )?;
        let api_key = state
            .key_vault
            .get(&profile, &provider_id)
            .map_err(|e| e.to_string())?
            .filter(|k| !k.trim().is_empty())
            .ok_or_else(|| {
                format!(
                    "No vault key for '{provider_id}'. Save a Groq or OpenAI key under Keys, or set ADE_WHISPER_CMD for local whisper."
                )
            })?;
        let base = base_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| {
                ade_agents::audio::default_whisper_base_url(&provider_id).map(|s| s.to_string())
            })
            .ok_or_else(|| {
                format!("No Whisper base URL for provider '{provider_id}'")
            })?;
        let model_id = model
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                ade_agents::audio::default_whisper_model(&provider_id).to_string()
            });
        ade_agents::audio::transcribe_api(
            &absolute,
            &ade_agents::audio::TranscribeApiOpts {
                api_key,
                base_url: base,
                model: model_id,
                provider_label: provider_id,
            },
        )
        .await
        .map_err(|e| e.to_string())?
    };

    let name = absolute
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("audio.bin");
    let markdown =
        ade_agents::audio::format_transcript_markdown(name, &rel_str, &result);
    let inbox = chat_inbox_dir(&root)?;
    let safe = sanitize_inbox_name(&format!(
        "{}.transcript.md",
        Path::new(name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("audio")
    ));
    let dest_name = format!("{}-{}", chrono_lite_stamp(), safe);
    let dest = inbox.join(&dest_name);
    std::fs::write(&dest, markdown.as_bytes()).map_err(|error| format!("write inbox: {error}"))?;
    let bytes = std::fs::metadata(&dest)
        .map(|m| m.len())
        .unwrap_or(markdown.len() as u64);
    Ok(StagedAttachment {
        name: dest_name.clone(),
        path: format!(".ade/inbox/{dest_name}"),
        absolute: dest.display().to_string(),
        bytes,
        staged: true,
        is_dir: false,
    })
}

fn resolve_whisper_provider(
    vault: &dyn ade_db::secrets::ProviderKeyVault,
    profile: &str,
    requested: Option<&str>,
) -> Result<String, String> {
    if let Some(raw) = requested.map(str::trim).filter(|s| !s.is_empty()) {
        let id = raw.to_ascii_lowercase();
        if ade_agents::audio::default_whisper_base_url(&id).is_none() {
            return Err(format!(
                "Unsupported Whisper provider '{id}' (use groq or openai, or ADE_WHISPER_CMD)"
            ));
        }
        return Ok(id);
    }
    for id in ade_agents::audio::WHISPER_PROVIDER_PREFERENCE {
        if vault.contains(profile, id).unwrap_or(false) {
            return Ok((*id).to_string());
        }
    }
    Err(
        "No Groq/OpenAI key in the vault for Whisper. Save one under Keys, pick provider, or set ADE_WHISPER_CMD."
            .into(),
    )
}

/// Extract `.docx` / `.xlsx` text into `.ade/inbox/*.extract.md` (explicit; never auto).
#[tauri::command]
pub fn chat_extract_office(
    state: State<'_, AppState>,
    source_path: String,
) -> Result<StagedAttachment, String> {
    let root = state.workspace_root();
    let (absolute, rel_str) = resolve_chat_media_source(&root, &source_path)?;
    let kind = ade_agents::office::OfficeKind::from_path(&absolute).ok_or_else(|| {
        format!("not a .docx/.xlsx file: {source_path}")
    })?;
    let result =
        ade_agents::office::extract_office(&absolute).map_err(|e| e.to_string())?;
    let name = absolute
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(match kind {
            ade_agents::office::OfficeKind::Docx => "document.docx",
            ade_agents::office::OfficeKind::Xlsx => "workbook.xlsx",
        });
    let markdown =
        ade_agents::office::format_office_extract_markdown(name, &rel_str, &result);
    let inbox = chat_inbox_dir(&root)?;
    let safe = sanitize_inbox_name(&format!(
        "{}.extract.md",
        Path::new(name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("document")
    ));
    let dest_name = format!("{}-{}", chrono_lite_stamp(), safe);
    let dest = inbox.join(&dest_name);
    std::fs::write(&dest, markdown.as_bytes()).map_err(|error| format!("write inbox: {error}"))?;
    let bytes = std::fs::metadata(&dest)
        .map(|m| m.len())
        .unwrap_or(markdown.len() as u64);
    Ok(StagedAttachment {
        name: dest_name.clone(),
        path: format!(".ade/inbox/{dest_name}"),
        absolute: dest.display().to_string(),
        bytes,
        staged: true,
        is_dir: false,
    })
}

/// Fetch an http(s) URL into `.ade/inbox/fetch-*.md` (explicit unfurl; never auto).
#[tauri::command]
pub async fn chat_fetch_url(
    state: State<'_, AppState>,
    url: String,
) -> Result<StagedAttachment, String> {
    let trimmed = url.trim().to_string();
    if trimmed.is_empty() {
        return Err("url required".into());
    }
    if !(trimmed.starts_with("https://") || trimmed.starts_with("http://")) {
        return Err("only http(s) urls can be fetched".into());
    }
    let body = ade_agents::web::web_fetch(&trimmed)
        .await
        .map_err(|error| error.to_string())?;
    let root = state.workspace_root();
    let inbox = chat_inbox_dir(&root)?;
    let host = trimmed
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("page")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
        .collect::<String>();
    let safe = sanitize_inbox_name(&format!("{host}.md"));
    let dest_name = format!("fetch-{}-{}", chrono_lite_stamp(), safe);
    let dest = inbox.join(&dest_name);
    let markdown = format!("# Fetched\n\nSource: {trimmed}\n\n---\n\n{body}\n");
    std::fs::write(&dest, markdown.as_bytes()).map_err(|error| format!("write inbox: {error}"))?;
    let bytes = std::fs::metadata(&dest)
        .map(|m| m.len())
        .unwrap_or(markdown.len() as u64);
    Ok(StagedAttachment {
        name: dest_name.clone(),
        path: format!(".ade/inbox/{dest_name}"),
        absolute: dest.display().to_string(),
        bytes,
        staged: true,
        is_dir: false,
    })
}

const MENTION_SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".turbo",
    "coverage",
    "__pycache__",
    ".venv",
    "venv",
    "ade-target",
];

/// Shallow workspace path list for composer `@` mentions (capped, skip heavy dirs).
#[tauri::command]
pub fn workspace_mention_candidates(
    state: State<'_, AppState>,
    query: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<String>, String> {
    let root = state.workspace_root();
    let max = limit.unwrap_or(40).clamp(1, 80);
    let needle = query
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .replace('\\', "/");
    let mut out = Vec::new();
    fn walk(
        dir: &Path,
        root: &Path,
        depth: usize,
        needle: &str,
        out: &mut Vec<String>,
        max: usize,
    ) {
        if out.len() >= max || depth > 6 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut names: Vec<_> = entries.flatten().collect();
        names.sort_by_key(|e| e.file_name());
        for entry in names {
            if out.len() >= max {
                return;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') && name != ".ade" && name != ".github" {
                // Allow .ade / .github; skip other dotdirs at top levels.
                if path.is_dir() {
                    continue;
                }
            }
            if path.is_dir() {
                if MENTION_SKIP_DIRS.iter().any(|s| name.eq_ignore_ascii_case(s)) {
                    continue;
                }
                walk(&path, root, depth + 1, needle, out, max);
                continue;
            }
            let Ok(rel) = path.strip_prefix(root) else {
                continue;
            };
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if ade_core::ignore::SensitivePathPolicy::path_is_blocked(&rel_str) {
                continue;
            }
            if !needle.is_empty() && !rel_str.to_ascii_lowercase().contains(needle) {
                continue;
            }
            out.push(rel_str);
        }
    }
    // Prefer a few roots first so AGENTS.md / README surface early.
    for seed in ["AGENTS.md", "README.md", "Cargo.toml", "package.json"] {
        let p = root.join(seed);
        if p.is_file() {
            let rel = seed.to_string();
            if needle.is_empty() || rel.to_ascii_lowercase().contains(&needle) {
                out.push(rel);
            }
        }
    }
    walk(&root, &root, 0, &needle, &mut out, max);
    out.sort();
    out.dedup();
    if out.len() > max {
        out.truncate(max);
    }
    Ok(out)
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
        let root = state.workspace_root();
        // Preferred / Default scratch are valid ADE workspaces (AGENTS.md) but may
        // not be the monorepo — do not require Cargo.toml.
        assert!(
            is_ade_workspace(&root),
            "expected AGENTS.md workspace, got {}",
            root.display()
        );
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

    #[test]
    fn sanitize_workspace_name_rejects_paths() {
        assert!(sanitize_workspace_name("").is_err());
        assert!(sanitize_workspace_name("..").is_err());
        assert!(sanitize_workspace_name("a/b").is_err());
        assert!(sanitize_workspace_name("a\\b").is_err());
        assert_eq!(
            sanitize_workspace_name(" BoxingLove ").unwrap(),
            "BoxingLove"
        );
        assert_eq!(sanitize_workspace_name("Love<>Game").unwrap(), "Love__Game");
    }

    #[test]
    fn ensure_workspace_identity_writes_schema_files() {
        let root = std::env::temp_dir().join(format!("ade-ws-id-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        ensure_workspace_identity(&root, "BoxingLove").unwrap();
        let identity = std::fs::read_to_string(root.join(".ade").join("workspace.json")).unwrap();
        assert!(identity.contains("ade.workspace/v1"));
        assert!(identity.contains("BoxingLove"));
        let session =
            std::fs::read_to_string(root.join(".ade").join("session").join("active.json")).unwrap();
        assert!(session.contains("ade.session/v1"));
        assert!(session.contains("primary"));
        // Idempotent
        ensure_workspace_identity(&root, "Other").unwrap();
        let identity2 = std::fs::read_to_string(root.join(".ade").join("workspace.json")).unwrap();
        assert!(identity2.contains("BoxingLove"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn ensure_default_workspace_creates_agents_and_identity() {
        // Uses real machine home when available; skip soft if path helpers fail.
        let Ok(root) = ensure_default_workspace() else {
            return;
        };
        assert!(root.join("AGENTS.md").is_file());
        assert!(root.join(".ade").join("workspace.json").is_file());
        assert!(is_default_workspace(&root));
        // Idempotent
        let again = ensure_default_workspace().unwrap();
        assert!(same_path(&root, &again));
    }
}
