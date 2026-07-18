use ade_agents::mcp::{McpHost, McpServerConfig, McpToolCallResult, McpToolInfo};
use ade_core::audit::{AuditMode, AuditReport, AuditRunner};
use ade_core::execute::{ExecuteOptions, ExecuteReport, ExecuteRunner};
use ade_core::plan::{PlanBuilder, PlanReport};
use ade_core::verify::{VerifyGate, VerifyResult};
use ade_workflow::verify::VerifyRunner;
use serde::Serialize;
use std::path::PathBuf;
use tauri::State;

pub struct AppState {
    pub workspace_root: PathBuf,
    pub mcp: McpHost,
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
        }
    }
}

#[derive(Serialize)]
pub struct DashboardSnapshot {
    pub workspace_root: String,
    pub audit: AuditReport,
    pub plan: PlanReport,
}

#[tauri::command]
pub async fn get_dashboard(state: State<'_, AppState>) -> Result<DashboardSnapshot, String> {
    let audit = AuditRunner::new(&state.workspace_root).run(AuditMode::EvaluateExisting);
    let plan = PlanBuilder::new().build(&audit);
    Ok(DashboardSnapshot {
        workspace_root: state.workspace_root.display().to_string(),
        audit,
        plan,
    })
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
    ExecuteRunner::new(&state.workspace_root)
        .run(
            &plan,
            &ExecuteOptions {
                approved,
                recipe_id: recipe.unwrap_or_else(|| "rust-api-turso".into()),
                phase_ids: vec![],
            },
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn run_verify(
    state: State<'_, AppState>,
    gate: String,
    through: bool,
) -> Result<Vec<VerifyResult>, String> {
    let gate: VerifyGate = gate.parse()?;
    let runner = VerifyRunner::with_root(&state.workspace_root);
    if through {
        Ok(runner.run_through(gate).await)
    } else {
        Ok(vec![runner.run_gate(gate).await])
    }
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
    state
        .mcp
        .call_tool(&server, &tool, arguments)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_a_workspace_root() {
        let state = AppState::discover();
        assert!(state.workspace_root.join("Cargo.toml").is_file());
        assert!(state.workspace_root.join("AGENTS.md").is_file());
    }
}
