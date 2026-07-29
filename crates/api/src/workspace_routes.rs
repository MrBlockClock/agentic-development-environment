use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;

use crate::router::{ApiResult, ApiState};

#[derive(Debug, Serialize)]
pub struct WorkspaceEntry {
    pub name: String,
    pub path: String,
    pub is_current: bool,
    pub is_ade_source: bool,
    pub is_default: bool,
    pub has_agents: bool,
    pub has_recipe: bool,
}

#[derive(Debug, Serialize)]
pub struct WorkspaceList {
    pub current: String,
    pub entries: Vec<WorkspaceEntry>,
    pub ade_source_root: Option<String>,
}

/// Read-only workspace list for browser preview (current serve root only).
/// Open / create / switch still need Desktop.
pub fn routes() -> Router<ApiState> {
    Router::new().route("/list", get(list_workspaces))
}

async fn list_workspaces(State(state): State<ApiState>) -> ApiResult<WorkspaceList> {
    let root = state.workspace_root();
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace")
        .to_string();
    let has_agents = root.join("AGENTS.md").is_file();
    let has_recipe = root.join(".ade").join("recipe.json").is_file();
    Ok(Json(WorkspaceList {
        current: root.display().to_string(),
        entries: vec![WorkspaceEntry {
            name,
            path: root.display().to_string(),
            is_current: true,
            is_ade_source: false,
            is_default: false,
            has_agents,
            has_recipe,
        }],
        ade_source_root: None,
    }))
}
