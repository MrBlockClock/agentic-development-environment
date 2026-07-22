use crate::authority::{
    classify_tool_effect, AuthorityEnforcer, ToolAnnotations, ToolAuthRequest, ToolEffect,
    WriteScope,
};
use crate::autonomy::AutonomyLevel;
use crate::mcp::McpHost;
use crate::provider::{
    ChatProvider, ModelConfig, ProviderRequest, ProviderStreamEvent, ProviderTool, ProviderUsage,
};
use crate::skills::{skill_body_block, SkillLoader};
use crate::spend::{SpendCaps, SpendGuard, SpendOutcome};
use ade_core::error::AdeError;
use ade_db::usage_ledger::UsageLedgerStore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

const DEFAULT_MAX_TOOL_ROUNDS: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTurnResult {
    pub session_id: Uuid,
    pub provider: String,
    pub model: String,
    pub text: String,
    pub tool_calls: usize,
    pub usage: ProviderUsage,
    pub cost_micros: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    Started {
        session_id: Uuid,
        provider: String,
        model: String,
    },
    TextDelta {
        text: String,
    },
    ToolCall {
        server: String,
        tool: String,
        arguments: Value,
        effect: ToolEffect,
    },
    ToolResult {
        server: String,
        tool: String,
        is_error: bool,
        text: String,
    },
    Usage {
        input_tokens: u64,
        output_tokens: u64,
        cost_micros: i64,
    },
    SpendWarning {
        scope: String,
        period_key: String,
        projected_micros: i64,
        soft_cap_micros: i64,
    },
    VerifyComplete {
        gate: String,
        passed: bool,
        summary: String,
    },
    Completed {
        result: AgentTurnResult,
    },
    Failed {
        error: String,
    },
    Cancelled {
        reason: String,
    },
}

#[derive(Clone)]
pub struct AgentSession {
    provider: Arc<dyn ChatProvider>,
    model: ModelConfig,
    mcp: McpHost,
    system_prompt: String,
    authority: AuthorityEnforcer,
    workspace_root: PathBuf,
    session_id: Uuid,
    spend: Option<SpendGuard>,
    cancel: Option<Arc<AtomicBool>>,
    request_timeout: Duration,
    autonomy: AutonomyLevel,
    max_tool_rounds: usize,
    max_tokens: Option<u64>,
    preferred_shell_cwd: Option<String>,
}

impl AgentSession {
    pub fn new(
        provider: Arc<dyn ChatProvider>,
        model: ModelConfig,
        mcp: McpHost,
        system_prompt: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            model,
            mcp,
            system_prompt: system_prompt.into(),
            authority: AuthorityEnforcer::read_only(),
            workspace_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            session_id: Uuid::new_v4(),
            spend: None,
            cancel: None,
            request_timeout: Duration::from_secs(120),
            autonomy: AutonomyLevel::Propose,
            max_tool_rounds: DEFAULT_MAX_TOOL_ROUNDS,
            max_tokens: None,
            preferred_shell_cwd: None,
        }
    }

    pub fn with_authority(mut self, authority: AuthorityEnforcer) -> Self {
        self.authority = authority;
        self
    }

    pub fn with_workspace(mut self, root: impl Into<PathBuf>) -> Self {
        self.workspace_root = root.into();
        self
    }

    pub fn with_preferred_shell_cwd(mut self, cwd: Option<impl Into<String>>) -> Self {
        self.preferred_shell_cwd = cwd.map(Into::into).filter(|s| !s.trim().is_empty());
        self
    }

    pub fn with_autonomy(mut self, autonomy: AutonomyLevel) -> Self {
        self.autonomy = autonomy;
        self
    }

    /// Apply/Automate without PLAN owned_paths: the dial is the human write approval.
    fn tool_write_scope(&self) -> WriteScope {
        if self.autonomy.allows_mutating_tools() && self.authority.owned_paths().is_empty() {
            WriteScope::HumanReviewed
        } else {
            WriteScope::PlanOwnedPaths
        }
    }

    pub fn with_max_tool_rounds(mut self, rounds: usize) -> Self {
        self.max_tool_rounds = rounds.max(1);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: Option<u64>) -> Self {
        self.max_tokens = max_tokens.filter(|value| *value > 0);
        self
    }

    pub fn with_session_id(mut self, session_id: Uuid) -> Self {
        self.session_id = session_id;
        self
    }

    pub fn with_spend_guard(mut self, guard: SpendGuard) -> Self {
        self.session_id = guard.session_id();
        self.spend = Some(guard);
        self
    }

    pub fn with_spend_caps(self, caps: SpendCaps, ledger: UsageLedgerStore) -> Self {
        let session_id = self.session_id;
        let root = self.workspace_root.clone();
        self.with_spend_guard(SpendGuard::new(root, session_id, caps, ledger))
    }

    pub fn with_cancel_flag(mut self, cancel: Arc<AtomicBool>) -> Self {
        self.cancel = Some(cancel);
        self
    }

    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    /// Starts one user turn and returns a live event stream. The task owns no
    /// credentials beyond this session and never includes the provider key in
    /// an event or result.
    pub fn start_turn(&self, prompt: impl Into<String>) -> mpsc::Receiver<AgentEvent> {
        let (events, receiver) = mpsc::channel(128);
        let session = self.clone();
        let prompt = prompt.into();
        tokio::spawn(async move {
            match session.run_turn(prompt, events.clone()).await {
                Ok(result) => {
                    let _ = events.send(AgentEvent::Completed { result }).await;
                }
                Err(error) => {
                    let event = if matches!(error, AdeError::Cancelled(_)) {
                        AgentEvent::Cancelled {
                            reason: error.to_string(),
                        }
                    } else {
                        AgentEvent::Failed {
                            error: error.to_string(),
                        }
                    };
                    let _ = events.send(event).await;
                }
            }
        });
        receiver
    }

    pub async fn run_turn(
        &self,
        prompt: String,
        events: mpsc::Sender<AgentEvent>,
    ) -> Result<AgentTurnResult, AdeError> {
        self.model.validate_spend_limits()?;
        let spend = self.spend.as_ref().ok_or_else(|| {
            AdeError::Config("agent session requires a SpendGuard before running turns".into())
        })?;
        let provider_name = self.provider.name().to_string();
        let _ = events
            .send(AgentEvent::Started {
                session_id: self.session_id,
                provider: provider_name.clone(),
                model: self.model.id.clone(),
            })
            .await;

        let (provider_tools, tool_routes) = self.provider_tools().await?;
        let mut messages = vec![
            json!({ "role": "system", "content": self.system_prompt }),
            json!({ "role": "user", "content": prompt }),
        ];
        let mut all_text = String::new();
        let mut total_usage = ProviderUsage::default();
        let mut tool_call_count = 0;

        for round in 0..self.max_tool_rounds {
            self.check_cancelled()?;
            if let Some(max_tokens) = self.max_tokens {
                // Effort budgets generated tokens only. Counting input each round
                // re-bills the full system prompt and kills Apply mid-task.
                let used = total_usage.output_tokens;
                if used >= max_tokens {
                    return Err(AdeError::Provider(format!(
                        "agent exceeded the {max_tokens}-token output budget (used {used})"
                    )));
                }
            }
            let estimate = self.model.max_round_cost()?;
            let (reservations, outcomes) = spend
                .reserve(estimate, &provider_name, &self.model.id)
                .await?;
            for outcome in outcomes {
                if let SpendOutcome::SoftWarning {
                    scope,
                    period_key,
                    projected,
                    soft_cap,
                } = outcome
                {
                    let _ = events
                        .send(AgentEvent::SpendWarning {
                            scope: scope.as_str().into(),
                            period_key,
                            projected_micros: projected.micros(),
                            soft_cap_micros: soft_cap.micros(),
                        })
                        .await;
                }
            }

            let completion = match self
                .stream_provider_round(&messages, &provider_tools, &events)
                .await
            {
                Ok(completion) => completion,
                Err(error) => {
                    let _ = spend.release(&reservations).await;
                    return Err(error);
                }
            };

            if completion.usage.exceeds_model_limits(&self.model) {
                let _ = spend.release(&reservations).await;
                return Err(AdeError::Provider(format!(
                    "provider reported usage exceeding model limits (in={} out={} limits {}/{})",
                    completion.usage.input_tokens,
                    completion.usage.output_tokens,
                    self.model.context_limit,
                    self.model.output_limit
                )));
            }

            let actual = completion.usage.cost_money(&self.model);
            if let Err(error) = spend
                .reconcile(
                    &reservations,
                    actual,
                    completion.usage.input_tokens,
                    completion.usage.output_tokens,
                )
                .await
            {
                let _ = spend.release(&reservations).await;
                return Err(error);
            }

            all_text.push_str(&completion.text);
            total_usage.input_tokens += completion.usage.input_tokens;
            total_usage.output_tokens += completion.usage.output_tokens;

            if let Some(max_tokens) = self.max_tokens {
                let used = total_usage.output_tokens;
                if used > max_tokens {
                    return Err(AdeError::Provider(format!(
                        "agent exceeded the {max_tokens}-token output budget after round {} (used {used})",
                        round + 1
                    )));
                }
            }

            if completion.tool_calls.is_empty() {
                let result = AgentTurnResult {
                    session_id: self.session_id,
                    provider: provider_name,
                    model: self.model.id.clone(),
                    text: all_text,
                    tool_calls: tool_call_count,
                    cost_micros: total_usage.cost_money(&self.model).micros(),
                    usage: total_usage,
                };
                return Ok(result);
            }

            let assistant_calls: Vec<Value> = completion
                .tool_calls
                .iter()
                .map(|call| {
                    json!({
                        "id": call.id,
                        "type": "function",
                        "function": { "name": call.name, "arguments": call.arguments },
                    })
                })
                .collect();
            messages.push(json!({
                "role": "assistant",
                "content": completion.text,
                "tool_calls": assistant_calls,
            }));

            for call in completion.tool_calls {
                self.check_cancelled()?;
                tool_call_count += 1;
                let route = tool_routes.get(&call.name).ok_or_else(|| {
                    AdeError::Provider(format!(
                        "model requested unknown or ambiguous tool '{}'",
                        call.name
                    ))
                })?;
                let arguments: Value = serde_json::from_str(&call.arguments).map_err(|error| {
                    AdeError::Provider(format!(
                        "model returned invalid arguments for '{}': {error}",
                        call.name
                    ))
                })?;
                if !arguments.is_object() {
                    return Err(AdeError::Provider(format!(
                        "model arguments for '{}' must be a JSON object",
                        call.name
                    )));
                }
                let mut auth_request = ToolAuthRequest {
                    server: route.server.clone(),
                    tool: route.tool.clone(),
                    arguments: arguments.clone(),
                    input_schema: route.input_schema.clone(),
                    annotations: Some(route.annotations.clone()),
                    write_scope: self.tool_write_scope(),
                    human_approved: false,
                };
                let effect = classify_tool_effect(&auth_request);
                if !self.autonomy.allows_tool_effect(effect) {
                    return Err(AdeError::Authorization(format!(
                        "tool {}/{} ({effect:?}) blocked by autonomy={}",
                        route.server,
                        route.tool,
                        self.autonomy.as_str()
                    )));
                }
                // Act/Automate is the human gate for full process tools. Suggest/Propose
                // may run inspect-only shell (validated before authorize).
                if matches!(effect, ToolEffect::ProcessExecution) {
                    if self.autonomy.allows_mutating_tools() {
                        auth_request.human_approved = true;
                    } else if self.autonomy == AutonomyLevel::Propose {
                        let cmd = arguments
                            .get("command")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        if crate::shell::is_inspect_command(cmd) {
                            auth_request.human_approved = true;
                        } else {
                            return Err(AdeError::Authorization(
                                "Suggest shell is inspect-only (list/read). Switch to Apply for mkdir/move/write commands."
                                    .into(),
                            ));
                        }
                    }
                }
                self.authority.authorize_tool_call(&auth_request)?;
                let _ = events
                    .send(AgentEvent::ToolCall {
                        server: route.server.clone(),
                        tool: route.tool.clone(),
                        arguments: arguments.clone(),
                        effect,
                    })
                    .await;
                let (is_error, text, content) = if route.host {
                    match self.call_host_tool(&route.tool, &arguments).await {
                        Ok(text) => (false, text.clone(), Value::String(text)),
                        Err(error) => {
                            let text = error.to_string();
                            (true, text.clone(), Value::String(text))
                        }
                    }
                } else {
                    let result = self
                        .mcp
                        .call_tool(&route.server, &route.tool, arguments)
                        .await?;
                    (
                        result.is_error,
                        result.text.clone(),
                        if result.text.is_empty() {
                            result.content
                        } else {
                            Value::String(result.text)
                        },
                    )
                };
                let _ = events
                    .send(AgentEvent::ToolResult {
                        server: route.server.clone(),
                        tool: route.tool.clone(),
                        is_error,
                        text: text.clone(),
                    })
                    .await;
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": call.id,
                    "content": if text.is_empty() {
                        serde_json::to_string(&content)?
                    } else {
                        text
                    },
                }));
            }
        }

        Err(AdeError::Provider(format!(
            "agent exceeded the {}-round tool-call limit",
            self.max_tool_rounds
        )))
    }

    async fn stream_provider_round(
        &self,
        messages: &[Value],
        provider_tools: &[ProviderTool],
        events: &mpsc::Sender<AgentEvent>,
    ) -> Result<crate::provider::ProviderCompletion, AdeError> {
        let (provider_events, mut provider_receiver) = mpsc::channel(128);
        let provider = Arc::clone(&self.provider);
        let request = ProviderRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            tools: provider_tools.to_vec(),
        };
        let provider_task =
            tokio::spawn(async move { provider.stream(request, provider_events).await });

        let recv_loop = async {
            while let Some(event) = provider_receiver.recv().await {
                self.check_cancelled()?;
                match event {
                    ProviderStreamEvent::TextDelta { text } => {
                        let _ = events.send(AgentEvent::TextDelta { text }).await;
                    }
                    ProviderStreamEvent::Usage { usage } => {
                        let _ = events
                            .send(AgentEvent::Usage {
                                input_tokens: usage.input_tokens,
                                output_tokens: usage.output_tokens,
                                cost_micros: usage.cost_money(&self.model).micros(),
                            })
                            .await;
                    }
                }
            }
            Ok::<(), AdeError>(())
        };

        match tokio::time::timeout(self.request_timeout, recv_loop).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                provider_task.abort();
                return Err(error);
            }
            Err(_) => {
                provider_task.abort();
                return Err(AdeError::Provider(format!(
                    "provider request timed out after {}s",
                    self.request_timeout.as_secs()
                )));
            }
        }

        provider_task
            .await
            .map_err(|error| AdeError::Provider(format!("provider task failed: {error}")))?
    }

    fn check_cancelled(&self) -> Result<(), AdeError> {
        if self
            .cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            return Err(AdeError::Cancelled("agent turn cancelled".into()));
        }
        Ok(())
    }

    async fn call_host_tool(&self, tool: &str, arguments: &Value) -> Result<String, AdeError> {
        match tool {
            "activate_skill" => {
                let name = arguments
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                let skill = SkillLoader::new(&self.workspace_root).activate(name)?;
                Ok(skill_body_block(&skill))
            }
            "web_fetch" => {
                let url = arguments
                    .get("url")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                crate::web::web_fetch(url).await
            }
            "web_search" => {
                let query = arguments
                    .get("query")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                crate::web::web_search(query).await
            }
            "read_file" => self.host_read_file(arguments),
            "write_file" => self.host_write_file(arguments),
            "run_command" => {
                let command = arguments
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                let timeout_secs = arguments.get("timeout_secs").and_then(Value::as_u64);
                let cwd = arguments
                    .get("cwd")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .or(self.preferred_shell_cwd.as_deref());
                crate::shell::run_command(
                    &self.workspace_root,
                    command,
                    timeout_secs,
                    crate::shell::ShellOptions {
                        inspect_only: !self.autonomy.allows_mutating_tools(),
                        cwd,
                    },
                )
                .await
            }
            other => Err(AdeError::NotFound(format!("unknown host tool '{other}'"))),
        }
    }

    fn host_resolve_path(&self, relative: &str) -> Result<PathBuf, AdeError> {
        let relative = relative.trim().replace('\\', "/");
        if relative.is_empty() {
            return Err(AdeError::Config("path is required".into()));
        }
        if PathBuf::from(&relative).is_absolute() {
            return Err(AdeError::Authorization(
                "absolute paths are not allowed; use a workspace-relative path".into(),
            ));
        }
        if relative.split('/').any(|part| part == "..") {
            return Err(AdeError::Authorization(
                "path traversal (..) is not allowed".into(),
            ));
        }
        if crate::ignore_enforcer::IgnoreEnforcer::new(&self.workspace_root)
            .path_is_blocked(&relative)
        {
            return Err(AdeError::Authorization(format!(
                "path '{relative}' is blocked by ADE ignore policy"
            )));
        }
        let full = self.workspace_root.join(&relative);
        let canonical_root = self
            .workspace_root
            .canonicalize()
            .unwrap_or_else(|_| self.workspace_root.clone());
        if let Ok(canonical) = full.canonicalize() {
            if !canonical.starts_with(&canonical_root) {
                return Err(AdeError::Authorization(
                    "resolved path escapes the workspace root".into(),
                ));
            }
        }
        Ok(full)
    }

    fn host_read_file(&self, arguments: &Value) -> Result<String, AdeError> {
        let path = arguments
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let full = self.host_resolve_path(path)?;
        let text = std::fs::read_to_string(&full)
            .map_err(|error| AdeError::Other(format!("read_file {}: {error}", full.display())))?;
        const MAX: usize = 32_000;
        if text.chars().count() > MAX {
            let clipped: String = text.chars().take(MAX).collect();
            return Ok(format!("{clipped}\n\n…[truncated at {MAX} chars]"));
        }
        Ok(text)
    }

    fn host_write_file(&self, arguments: &Value) -> Result<String, AdeError> {
        let path = arguments
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let content = arguments
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| AdeError::Config("content string is required".into()))?;
        let full = self.host_resolve_path(path)?;
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full, content)?;
        Ok(format!("wrote {} bytes to {path}", content.len()))
    }

    async fn provider_tools(
        &self,
    ) -> Result<(Vec<ProviderTool>, BTreeMap<String, ToolRoute>), AdeError> {
        let tools = self.mcp.list_tools().await?;
        let mut provider_tools = Vec::with_capacity(tools.len() + 1);
        let mut routes = BTreeMap::new();

        for host in host_tools() {
            let effect = classify_tool_effect(&ToolAuthRequest {
                server: host.server.clone(),
                tool: host.tool.clone(),
                arguments: json!({}),
                input_schema: Some(host.input_schema.clone()),
                annotations: Some(host.annotations.clone()),
                write_scope: self.tool_write_scope(),
                human_approved: false,
            });
            if !self.autonomy.allows_tool_effect(effect) {
                continue;
            }
            let name = provider_tool_name(&host.server, &host.tool);
            if routes
                .insert(
                    name.clone(),
                    ToolRoute {
                        server: host.server.clone(),
                        tool: host.tool.clone(),
                        input_schema: Some(host.input_schema.clone()),
                        annotations: host.annotations.clone(),
                        host: true,
                    },
                )
                .is_some()
            {
                return Err(AdeError::Mcp(format!(
                    "tool name collision after provider encoding: '{name}'"
                )));
            }
            provider_tools.push(ProviderTool {
                name,
                description: format!("[{}] {}", host.server, host.description),
                input_schema: host.input_schema,
            });
        }

        for tool in tools {
            let effect = classify_tool_effect(&ToolAuthRequest {
                server: tool.server.clone(),
                tool: tool.name.clone(),
                arguments: json!({}),
                input_schema: Some(tool.input_schema.clone()),
                annotations: Some(tool.annotations.clone()),
                write_scope: self.tool_write_scope(),
                human_approved: false,
            });
            if !self.autonomy.allows_tool_effect(effect) {
                continue;
            }
            let name = provider_tool_name(&tool.server, &tool.name);
            if routes
                .insert(
                    name.clone(),
                    ToolRoute {
                        server: tool.server.clone(),
                        tool: tool.name.clone(),
                        input_schema: Some(tool.input_schema.clone()),
                        annotations: tool.annotations.clone(),
                        host: false,
                    },
                )
                .is_some()
            {
                return Err(AdeError::Mcp(format!(
                    "tool name collision after provider encoding: '{name}'"
                )));
            }
            provider_tools.push(ProviderTool {
                name,
                description: format!("[{}] {}", tool.server, tool.description),
                input_schema: tool.input_schema,
            });
        }
        Ok((provider_tools, routes))
    }
}

#[derive(Clone)]
struct ToolRoute {
    server: String,
    tool: String,
    input_schema: Option<Value>,
    annotations: ToolAnnotations,
    host: bool,
}

struct HostToolDef {
    server: String,
    tool: String,
    description: String,
    input_schema: Value,
    annotations: ToolAnnotations,
}

fn host_tools() -> Vec<HostToolDef> {
    vec![
        HostToolDef {
            server: "ade".into(),
            tool: "activate_skill".into(),
            description: "Load the full body of a listed .ade/skills skill by exact name (progressive disclosure T2).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Exact skill name from the T1 catalog"
                    }
                },
                "required": ["name"],
                "x-ade-effect": "read_only"
            }),
            annotations: ToolAnnotations {
                read_only_hint: Some(true),
                ade_effect: Some(ToolEffect::ReadOnly),
                ..Default::default()
            },
        },
        HostToolDef {
            server: "ade".into(),
            tool: "web_search".into(),
            description: "Search the public web (DuckDuckGo Instant Answer) for docs, APIs, or facts. Prefer this before guessing URLs.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    }
                },
                "required": ["query"],
                "x-ade-effect": "read_only"
            }),
            annotations: ToolAnnotations {
                read_only_hint: Some(true),
                ade_effect: Some(ToolEffect::ReadOnly),
                ..Default::default()
            },
        },
        HostToolDef {
            server: "ade".into(),
            tool: "web_fetch".into(),
            description: "Fetch an http(s) URL and return truncated text (HTML stripped). Use for docs, changelogs, and API references.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "Absolute http or https URL"
                    }
                },
                "required": ["url"],
                "x-ade-effect": "read_only"
            }),
            annotations: ToolAnnotations {
                read_only_hint: Some(true),
                ade_effect: Some(ToolEffect::ReadOnly),
                ..Default::default()
            },
        },
        HostToolDef {
            server: "fs".into(),
            tool: "read_file".into(),
            description: "Read a UTF-8 text file relative to the workspace root (read-only).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Workspace-relative path"
                    }
                },
                "required": ["path"],
                "x-ade-effect": "read_only"
            }),
            annotations: ToolAnnotations {
                read_only_hint: Some(true),
                ade_effect: Some(ToolEffect::ReadOnly),
                ..Default::default()
            },
        },
        HostToolDef {
            server: "fs".into(),
            tool: "write_file".into(),
            description: "Write a UTF-8 text file under an approved owned path (workspace write). Create parent dirs as needed.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Workspace-relative path inside approved owned_paths"
                    },
                    "content": {
                        "type": "string",
                        "description": "Full file contents to write"
                    }
                },
                "required": ["path", "content"],
                "x-ade-effect": "workspace_write"
            }),
            annotations: ToolAnnotations {
                read_only_hint: Some(false),
                ade_effect: Some(ToolEffect::WorkspaceWrite),
                ..Default::default()
            },
        },
        HostToolDef {
            server: "shell".into(),
            tool: "run_command".into(),
            description: "Run a one-shot shell command (shown as Shell in Live activity). Suggest: inspect-only (Get-ChildItem/ls/pwd/Get-Content). Apply: full shell minus dangerous wipes. Optional cwd may be workspace-relative, absolute under the user profile (Desktop), or $env:USERPROFILE\\\\Desktop. When omitted, uses the host SHELL SCOPE default (workspace or Home/Desktop). Not an interactive PTY.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command (PowerShell on Windows)"
                    },
                    "cwd": {
                        "type": "string",
                        "description": "Working directory override: workspace path, ~, $env:USERPROFILE\\\\Desktop, or absolute under user profile. Omit to use host SHELL SCOPE default."
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Optional timeout (1–300, default 60)"
                    }
                },
                "required": ["command"],
                "x-ade-effect": "process_execution"
            }),
            annotations: ToolAnnotations {
                read_only_hint: Some(false),
                ade_effect: Some(ToolEffect::ProcessExecution),
                ..Default::default()
            },
        },
    ]
}

fn provider_tool_name(server: &str, tool: &str) -> String {
    let raw = format!("{server}__{tool}");
    raw.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .take(64)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{ProviderCompletion, ProviderStreamEvent, ProviderToolCall};
    use ade_core::money::Money;
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct MockProvider {
        completions: Mutex<Vec<ProviderCompletion>>,
    }

    #[async_trait]
    impl ChatProvider for MockProvider {
        fn name(&self) -> &str {
            "mock"
        }

        async fn stream(
            &self,
            _request: ProviderRequest,
            events: mpsc::Sender<ProviderStreamEvent>,
        ) -> Result<ProviderCompletion, AdeError> {
            let completion = self.completions.lock().unwrap().remove(0);
            let _ = events
                .send(ProviderStreamEvent::TextDelta {
                    text: completion.text.clone(),
                })
                .await;
            Ok(completion)
        }
    }

    fn model() -> ModelConfig {
        ModelConfig {
            id: "mock-1".into(),
            name: "Mock".into(),
            context_limit: 10_000,
            output_limit: 1_000,
            cost_per_input_mtok: Money::from_usd_str("2.0").unwrap(),
            cost_per_output_mtok: Money::from_usd_str("4.0").unwrap(),
        }
    }

    async fn ledger() -> UsageLedgerStore {
        let database = ade_db::repo::AdeDatabase::open_path(":memory:")
            .await
            .unwrap();
        UsageLedgerStore::new(database.connect().unwrap())
    }

    #[tokio::test]
    async fn streams_and_prices_a_provider_turn() {
        let root = std::env::temp_dir().join(format!("ade-session-{}", Uuid::new_v4()));
        let provider = Arc::new(MockProvider {
            completions: Mutex::new(vec![ProviderCompletion {
                text: "done".into(),
                tool_calls: vec![],
                usage: ProviderUsage {
                    input_tokens: 1_000,
                    output_tokens: 500,
                },
            }]),
        });
        let session = AgentSession::new(provider, model(), McpHost::new(), "system")
            .with_workspace(&root)
            .with_spend_caps(
                SpendCaps {
                    session: Money::from_usd_str("1.0").unwrap(),
                    daily: Money::from_usd_str("10.0").unwrap(),
                },
                ledger().await,
            );
        let mut receiver = session.start_turn("hello");
        let mut result = None;
        while let Some(event) = receiver.recv().await {
            if let AgentEvent::Completed { result: complete } = event {
                result = Some(complete);
                break;
            }
        }
        let result = result.unwrap();
        assert_eq!(result.text, "done");
        // 1000*2 + 500*4 = 4000 micros after /1e6 ceiling math => 2+2 = 4_000 micros?
        // cost_for_tokens(1000, $2) = ceil(1000*2e6 / 1e6) = 2000 micros
        // cost_for_tokens(500, $4) = ceil(500*4e6 / 1e6) = 2000 micros
        assert_eq!(result.cost_micros, 4_000);
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn enforces_session_spend_cap_on_reserve() {
        let root = std::env::temp_dir().join(format!("ade-session-{}", Uuid::new_v4()));
        let provider = Arc::new(MockProvider {
            completions: Mutex::new(vec![ProviderCompletion {
                text: "done".into(),
                tool_calls: vec![],
                usage: ProviderUsage {
                    input_tokens: 1,
                    output_tokens: 1,
                },
            }]),
        });
        let expensive = ModelConfig {
            id: "mock-1".into(),
            name: "Mock".into(),
            context_limit: 1_000_000,
            output_limit: 1_000_000,
            cost_per_input_mtok: Money::from_usd_str("2.0").unwrap(),
            cost_per_output_mtok: Money::from_usd_str("4.0").unwrap(),
        };
        let session = AgentSession::new(provider, expensive, McpHost::new(), "system")
            .with_workspace(&root)
            .with_spend_caps(
                SpendCaps {
                    session: Money::from_usd_str("0.001").unwrap(),
                    daily: Money::from_usd_str("10.0").unwrap(),
                },
                ledger().await,
            );
        let mut receiver = session.start_turn("hello");
        let mut failed = false;
        while let Some(event) = receiver.recv().await {
            if let AgentEvent::Failed { .. } = event {
                failed = true;
                break;
            }
        }
        assert!(failed);
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn rejects_priced_model_with_zero_limits() {
        let root = std::env::temp_dir().join(format!("ade-session-{}", Uuid::new_v4()));
        let provider = Arc::new(MockProvider {
            completions: Mutex::new(vec![]),
        });
        let model = ModelConfig {
            id: "bad".into(),
            name: "Bad".into(),
            context_limit: 0,
            output_limit: 0,
            cost_per_input_mtok: Money::from_usd_str("1.0").unwrap(),
            cost_per_output_mtok: Money::ZERO,
        };
        let session = AgentSession::new(provider, model, McpHost::new(), "system")
            .with_workspace(&root)
            .with_spend_caps(SpendCaps::unlimited(), ledger().await);
        let mut receiver = session.start_turn("hello");
        let mut failed = false;
        while let Some(event) = receiver.recv().await {
            if let AgentEvent::Failed { error } = event {
                assert!(error.contains("non-zero"));
                failed = true;
                break;
            }
        }
        assert!(failed);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn provider_names_are_safe_and_bounded() {
        let name = provider_tool_name("files.server", &"write/file".repeat(20));
        assert!(name.len() <= 64);
        assert!(name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-')));
    }

    #[test]
    fn provider_tool_call_type_remains_serializable() {
        let call = ProviderToolCall {
            id: "1".into(),
            name: "tool".into(),
            arguments: "{}".into(),
        };
        assert!(serde_json::to_string(&call).is_ok());
    }
}
