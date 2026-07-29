use ade_core::error::AdeError;
use rmcp::model::CallToolRequestParam;
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::{Peer, ServiceExt};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Extra process env (e.g. vault-injected tokens). Never logged.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Set only after a human reviews this exact command and argument list.
    #[serde(default)]
    pub approved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    pub server: String,
    pub name: String,
    pub description: String,
    /// JSON Schema describing the tool's expected arguments.
    pub input_schema: serde_json::Value,
    /// Optional ADE/MCP annotations extracted from schema extensions.
    #[serde(default)]
    pub annotations: crate::authority::ToolAnnotations,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCallResult {
    pub server: String,
    pub tool: String,
    pub is_error: bool,
    pub text: String,
    pub content: serde_json::Value,
}

type ClientService = RunningService<RoleClient, ()>;

struct ConnectedServer {
    service: ClientService,
}

#[derive(Clone)]
pub struct McpHost {
    servers: Arc<Mutex<BTreeMap<String, ConnectedServer>>>,
    connect_timeout: Duration,
}

impl Default for McpHost {
    fn default() -> Self {
        Self::new()
    }
}

impl McpHost {
    pub fn new() -> Self {
        Self {
            servers: Arc::new(Mutex::new(BTreeMap::new())),
            connect_timeout: Duration::from_secs(15),
        }
    }

    pub fn with_timeout(connect_timeout: Duration) -> Self {
        Self {
            servers: Arc::new(Mutex::new(BTreeMap::new())),
            connect_timeout,
        }
    }

    pub async fn connect_server(&self, config: McpServerConfig) -> Result<(), AdeError> {
        validate_config(&config)?;
        {
            let servers = self.lock_servers()?;
            if servers.contains_key(&config.name) {
                return Err(AdeError::Mcp(format!(
                    "MCP server '{}' is already connected",
                    config.name
                )));
            }
        }

        let mut command = Command::new(&config.command);
        command.args(&config.args);
        for (key, value) in &config.env {
            command.env(key, value);
        }
        let transport = TokioChildProcess::new(&mut command).map_err(|error| {
            AdeError::Mcp(format!("failed to start '{}': {error}", config.name))
        })?;
        let service = tokio::time::timeout(self.connect_timeout, ().serve(transport))
            .await
            .map_err(|_| {
                AdeError::Mcp(format!(
                    "MCP server '{}' did not initialize within {}s",
                    config.name,
                    self.connect_timeout.as_secs()
                ))
            })?
            .map_err(|error| {
                AdeError::Mcp(format!(
                    "MCP server '{}' initialization failed: {error}",
                    config.name
                ))
            })?;

        self.lock_servers()?
            .insert(config.name, ConnectedServer { service });
        Ok(())
    }

    pub fn connected_servers(&self) -> Result<Vec<String>, AdeError> {
        Ok(self.lock_servers()?.keys().cloned().collect())
    }

    pub async fn list_tools(&self) -> Result<Vec<McpToolInfo>, AdeError> {
        let peers: Vec<(String, Peer<RoleClient>)> = self
            .lock_servers()?
            .iter()
            .map(|(name, server)| (name.clone(), server.service.peer().clone()))
            .collect();
        let mut result = Vec::new();
        for (server, peer) in peers {
            let tools = peer.list_all_tools().await.map_err(|error| {
                AdeError::Mcp(format!("failed to list tools from '{server}': {error}"))
            })?;
            result.extend(tools.into_iter().map(|tool| {
                let input_schema = tool.schema_as_json_value();
                let annotations = annotations_from_schema(&input_schema);
                McpToolInfo {
                    server: server.clone(),
                    input_schema,
                    name: tool.name.into_owned(),
                    description: tool.description.into_owned(),
                    annotations,
                }
            }));
        }
        Ok(result)
    }

    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: serde_json::Value,
    ) -> Result<McpToolCallResult, AdeError> {
        validate_id(server, "server name")?;
        validate_id(tool, "tool name")?;
        let arguments = match arguments {
            serde_json::Value::Null => None,
            serde_json::Value::Object(map) => Some(map),
            _ => {
                return Err(AdeError::Mcp(
                    "MCP tool arguments must be a JSON object".into(),
                ));
            }
        };

        let peer = {
            let servers = self.lock_servers()?;
            let connected = servers.get(server).ok_or_else(|| {
                AdeError::NotFound(format!("MCP server '{server}' is not connected"))
            })?;
            connected.service.peer().clone()
        };

        let result = peer
            .call_tool(CallToolRequestParam {
                name: tool.to_string().into(),
                arguments,
            })
            .await
            .map_err(|error| {
                AdeError::Mcp(format!(
                    "tool '{tool}' on server '{server}' failed: {error}"
                ))
            })?;

        let is_error = result.is_error.unwrap_or(false);
        let text = result
            .content
            .iter()
            .filter_map(|block| block.as_text().map(|text| text.text.clone()))
            .collect::<Vec<_>>()
            .join("\n");
        let content = serde_json::to_value(&result.content)?;

        Ok(McpToolCallResult {
            server: server.to_string(),
            tool: tool.to_string(),
            is_error,
            text,
            content,
        })
    }

    pub async fn disconnect_server(&self, name: &str) -> Result<bool, AdeError> {
        let server = self.lock_servers()?.remove(name);
        if let Some(server) = server {
            server
                .service
                .cancel()
                .await
                .map_err(|error| AdeError::Mcp(format!("failed to stop '{name}': {error}")))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn lock_servers(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, BTreeMap<String, ConnectedServer>>, AdeError> {
        self.servers
            .lock()
            .map_err(|_| AdeError::Mcp("MCP server registry lock poisoned".into()))
    }
}

fn validate_config(config: &McpServerConfig) -> Result<(), AdeError> {
    if !config.approved {
        return Err(AdeError::Authorization(format!(
            "MCP server '{}' requires explicit approval of its command and arguments",
            config.name
        )));
    }
    validate_id(&config.name, "server name")?;
    if config.command.trim().is_empty() || config.command.contains('\0') {
        return Err(AdeError::Mcp("MCP command is empty or invalid".into()));
    }
    if config.args.iter().any(|arg| arg.contains('\0')) {
        return Err(AdeError::Mcp(
            "MCP arguments may not contain null bytes".into(),
        ));
    }
    Ok(())
}

fn validate_id(value: &str, label: &str) -> Result<(), AdeError> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err(AdeError::Mcp(format!(
            "invalid {label} '{value}' (use 1-64 letters, digits, '.', '-', or '_')"
        )));
    }
    Ok(())
}

fn annotations_from_schema(schema: &serde_json::Value) -> crate::authority::ToolAnnotations {
    use crate::authority::{ToolAnnotations, ToolEffect};
    let mut annotations = ToolAnnotations::default();
    let Some(map) = schema.as_object() else {
        return annotations;
    };
    if let Some(value) = map
        .get("x-ade-effect")
        .or_else(|| map.get("x_ade_effect"))
        .or_else(|| map.get("ade_effect"))
        .and_then(|value| value.as_str())
    {
        annotations.ade_effect = match value.trim().to_ascii_lowercase().as_str() {
            "read" | "readonly" | "read_only" => Some(ToolEffect::ReadOnly),
            "write" | "workspace_write" | "mutate" => Some(ToolEffect::WorkspaceWrite),
            "external_write" | "network" | "upload" => Some(ToolEffect::ExternalWrite),
            "process" | "process_execution" | "execute" | "shell" => {
                Some(ToolEffect::ProcessExecution)
            }
            "unknown" => Some(ToolEffect::Unknown),
            _ => None,
        };
    }
    if let Some(value) = map
        .get("readOnlyHint")
        .or_else(|| map.get("read_only_hint"))
        .and_then(|value| value.as_bool())
    {
        annotations.read_only_hint = Some(value);
    }
    if let Some(value) = map
        .get("destructiveHint")
        .or_else(|| map.get("destructive_hint"))
        .and_then(|value| value.as_bool())
    {
        annotations.destructive_hint = Some(value);
    }
    if let Some(value) = map
        .get("openWorldHint")
        .or_else(|| map.get("open_world_hint"))
        .and_then(|value| value.as_bool())
    {
        annotations.open_world_hint = Some(value);
    }
    annotations
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> McpServerConfig {
        McpServerConfig {
            name: "filesystem".into(),
            command: "npx".into(),
            args: vec![
                "-y".into(),
                "@modelcontextprotocol/server-filesystem".into(),
            ],
            env: BTreeMap::new(),
            approved: false,
        }
    }

    #[tokio::test]
    async fn refuses_unapproved_server_before_spawn() {
        let error = McpHost::new().connect_server(config()).await.unwrap_err();
        assert!(error.to_string().contains("approval"));
    }

    #[tokio::test]
    async fn call_tool_requires_connected_server() {
        let error = McpHost::new()
            .call_tool("filesystem", "list_directory", serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("not connected"));
    }

    #[tokio::test]
    async fn call_tool_rejects_non_object_arguments() {
        let error = McpHost::new()
            .call_tool("filesystem", "list_directory", serde_json::json!(["bad"]))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("JSON object"));
    }

    #[test]
    fn accepts_reviewed_shell_free_config() {
        let mut config = config();
        config.approved = true;
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn rejects_invalid_names_and_null_arguments() {
        let mut config = config();
        config.approved = true;
        config.name = "../server".into();
        assert!(validate_config(&config).is_err());

        config.name = "server".into();
        config.args = vec!["bad\0arg".into()];
        assert!(validate_config(&config).is_err());
    }
}
