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
pub struct ActionEnvelope {
    pub effect: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    pub autonomy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_tier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    Started {
        session_id: Uuid,
        provider: String,
        model: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        route_reason: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        slot: Option<String>,
    },
    TextDelta {
        text: String,
    },
    ToolCall {
        server: String,
        tool: String,
        arguments: Value,
        effect: ToolEffect,
        /// E1 thin: authorized envelope snapshot for the feed / Continuity.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        envelope: Option<ActionEnvelope>,
    },
    ToolResult {
        server: String,
        tool: String,
        is_error: bool,
        text: String,
    },
    /// C2: mid-turn boundary / occupancy capsule applied to the model window.
    ContextCompacted {
        trigger: String,
        tokens_before: u64,
        tokens_after: u64,
        occupancy_before: f64,
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
    /// Effort / turn gas tank empty (tool rounds or cumulative output tokens).
    BudgetExhausted {
        kind: String,
        limit: u64,
        used: u64,
        detail: String,
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
    /// Desktop should attach a workspace or open the in-shell Browser.
    HostIntent {
        action: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
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
    /// When autonomy is Act/Automate, Act-class tools require a ready eng-goal contract.
    /// Suggest/Observe ignore this (mutating tools already blocked by autonomy).
    contract_allows_act: bool,
    contract_block_detail: Option<String>,
    /// H3: effects denied by the active model profile.
    profile_effect_deny: Vec<String>,
    /// H3: visible route annotation for Started.
    route_profile_id: Option<String>,
    route_reason: Option<String>,
    route_slot: Option<String>,
    /// G2: human-confirmed risk categories / tiers for this turn.
    approved_risk_categories: Vec<String>,
    approved_risk_tiers: Vec<String>,
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
            contract_allows_act: true,
            contract_block_detail: None,
            profile_effect_deny: Vec::new(),
            route_profile_id: None,
            route_reason: None,
            route_slot: None,
            approved_risk_categories: Vec::new(),
            approved_risk_tiers: Vec::new(),
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

    /// Master-gameplan G1: pass `false` + detail when Act/Automate lacks eng-goal contract.
    pub fn with_contract_gate(mut self, allows_act: bool, block_detail: Option<String>) -> Self {
        self.contract_allows_act = allows_act;
        self.contract_block_detail = block_detail;
        self
    }

    /// H3: attach model-profile route annotation + optional effect deny mask.
    pub fn with_route(
        mut self,
        profile_id: impl Into<String>,
        reason: impl Into<String>,
        slot: impl Into<String>,
        effect_deny: Vec<String>,
    ) -> Self {
        self.route_profile_id = Some(profile_id.into());
        self.route_reason = Some(reason.into());
        self.route_slot = Some(slot.into());
        self.profile_effect_deny = effect_deny;
        self
    }

    /// G2: human-confirmed risk categories/tiers for this turn (e.g. publish, infra, high).
    pub fn with_risk_approvals(mut self, categories: Vec<String>, tiers: Vec<String>) -> Self {
        self.approved_risk_categories = categories;
        self.approved_risk_tiers = tiers;
        self
    }

    fn profile_allows_effect(&self, effect: ToolEffect) -> bool {
        let needle = match effect {
            ToolEffect::ReadOnly => "read_only",
            ToolEffect::WorkspaceWrite => "workspace_write",
            ToolEffect::ExternalWrite => "external_write",
            ToolEffect::ProcessExecution => "process_execution",
            ToolEffect::Unknown => "unknown",
        };
        !self
            .profile_effect_deny
            .iter()
            .any(|d| d.eq_ignore_ascii_case(needle))
    }

    fn contract_allows_effect(&self, effect: ToolEffect) -> bool {
        if !self.autonomy.allows_mutating_tools() {
            return true;
        }
        if !crate::goal::is_act_class_effect(effect) {
            return true;
        }
        self.contract_allows_act
    }

    fn contract_deny_message(&self) -> String {
        self.contract_block_detail
            .clone()
            .unwrap_or_else(|| {
                format!(
                    "{} Act tools blocked until an active eng-goal has acceptance criteria, out-of-scope, and verify pointer (or ≤3 clarify / logged waive). Define a goal or switch to Suggest.",
                    crate::goal::CONTRACT_GATE_PREFIX
                )
            })
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
            match session.run_turn(prompt, Vec::new(), events.clone()).await {
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
        image_paths: Vec<String>,
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
                profile_id: self.route_profile_id.clone(),
                route_reason: self.route_reason.clone(),
                slot: self.route_slot.clone(),
            })
            .await;

        let (provider_tools, tool_routes) = self.provider_tools().await?;
        let profile_vision = self.route_profile_id.as_deref().and_then(|id| {
            crate::model_profile::ModelProfileCatalog::load(&self.workspace_root)
                .get(id)
                .and_then(|profile| profile.vision_capability())
        });
        let user_content = crate::vision::user_message_content_ex(
            &prompt,
            &image_paths,
            &self.model.id,
            &self.workspace_root,
            profile_vision,
        )?;
        let mut messages = vec![
            json!({ "role": "system", "content": self.system_prompt }),
            json!({ "role": "user", "content": user_content }),
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
                    let detail = format!(
                        "agent exceeded the {max_tokens}-token output budget (used {used})"
                    );
                    let _ = events
                        .send(AgentEvent::BudgetExhausted {
                            kind: "output_tokens".into(),
                            limit: max_tokens,
                            used,
                            detail: detail.clone(),
                        })
                        .await;
                    return Err(AdeError::Budget(detail));
                }
            }
            // Text estimate excluding base64 blobs; add dedicated vision band (Sprint D).
            let text_est =
                crate::context_edit::estimate_messages_tokens_excluding_image_data(&messages);
            let vision_est = if round == 0 && !image_paths.is_empty() {
                crate::vision::estimate_vision_tokens(&image_paths, &self.workspace_root)
                    .unwrap_or(0) as u64
            } else {
                0
            };
            let input_est = text_est.saturating_add(vision_est);
            let out_budget = self
                .max_tokens
                .map(|cap| cap.saturating_sub(total_usage.output_tokens))
                .unwrap_or(self.model.output_limit)
                .min(self.model.output_limit.max(256))
                .max(256);
            // Prefer last-round input when available (closer to provider truth).
            let input_tokens = if total_usage.input_tokens > 0 {
                total_usage.input_tokens.max(input_est)
            } else {
                input_est
            };
            let estimate = self.model.estimate_round_cost(input_tokens, out_budget)?;
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

            let masked = crate::context_edit::mask_stale_tool_results(
                &messages,
                crate::context_edit::tool_result_keep_rounds_from_env(),
            );
            // C2: ~70% occupancy safety net → structured boundary capsule (mask first).
            let mut round_messages = masked;
            if crate::context_edit::should_compact_at_occupancy(
                &round_messages,
                self.model.context_limit,
                crate::context_edit::compact_occupancy_from_env(),
            ) {
                let (compacted, summary) = self.apply_and_persist_boundary(
                    &round_messages,
                    "occupancy_70",
                    None,
                    None,
                    None,
                );
                round_messages = compacted;
                messages = round_messages.clone();
                let _ = events
                    .send(AgentEvent::ContextCompacted {
                        trigger: summary.trigger,
                        tokens_before: summary.tokens_before,
                        tokens_after: summary.tokens_after,
                        occupancy_before: summary.occupancy_before,
                    })
                    .await;
            }
            let completion = match self
                .stream_provider_round(&round_messages, &provider_tools, &events)
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

            let missing_usage = self.model.is_priced()
                && completion.usage.input_tokens == 0
                && completion.usage.output_tokens == 0
                && (!completion.text.is_empty() || !completion.tool_calls.is_empty());
            // Invoice honesty: never commit $0 when provider omitted usage on a priced turn.
            let actual = if missing_usage {
                estimate
            } else {
                completion.usage.cost_money(&self.model)
            };
            let (reconcile_in, reconcile_out) = if missing_usage {
                (input_tokens, 0)
            } else {
                (
                    completion.usage.input_tokens,
                    completion.usage.output_tokens,
                )
            };
            if let Err(error) = spend
                .reconcile(&reservations, actual, reconcile_in, reconcile_out)
                .await
            {
                let _ = spend.release(&reservations).await;
                return Err(error);
            }

            all_text.push_str(&completion.text);
            total_usage.input_tokens += if completion.usage.input_tokens > 0 {
                completion.usage.input_tokens
            } else if missing_usage {
                reconcile_in
            } else {
                0
            };
            total_usage.output_tokens += completion.usage.output_tokens;

            if let Some(max_tokens) = self.max_tokens {
                let used = total_usage.output_tokens;
                if used > max_tokens {
                    let detail = format!(
                        "agent exceeded the {max_tokens}-token output budget after round {} (used {used})",
                        round + 1
                    );
                    let _ = events
                        .send(AgentEvent::BudgetExhausted {
                            kind: "output_tokens".into(),
                            limit: max_tokens,
                            used,
                            detail: detail.clone(),
                        })
                        .await;
                    return Err(AdeError::Budget(detail));
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
                if !self.contract_allows_effect(effect) {
                    return Err(AdeError::Authorization(self.contract_deny_message()));
                }
                if !self.profile_allows_effect(effect) {
                    return Err(AdeError::Authorization(format!(
                        "tool {}/{} ({effect:?}) blocked by model profile deny mask",
                        route.server, route.tool
                    )));
                }
                let risk = crate::risk::assess_tool(&route.server, &route.tool, &arguments, effect);
                if risk.requires_hitl()
                    && !crate::risk::risk_is_approved(
                        &risk,
                        &self.approved_risk_categories,
                        &self.approved_risk_tiers,
                    )
                {
                    return Err(AdeError::Authorization(crate::risk::risk_deny_message(
                        &risk,
                    )));
                }
                // Act/Automate is the human gate for full process tools. Suggest/Propose
                // may run inspect-only shell (validated before authorize).
                // G2: high-risk still requires explicit approved_risk_* even under Act.
                if matches!(
                    effect,
                    ToolEffect::ProcessExecution | ToolEffect::ExternalWrite | ToolEffect::Unknown
                ) {
                    if risk.requires_hitl() {
                        auth_request.human_approved = true;
                        let _ = crate::risk::log_risk_waive(
                            &self.workspace_root,
                            &crate::risk::RiskWaiveRecord {
                                at: chrono::Utc::now().to_rfc3339(),
                                category: risk.category.as_str().into(),
                                tier: risk.tier.as_str().into(),
                                reason: risk.reason.clone(),
                                autonomy: self.autonomy.as_str().into(),
                                server: route.server.clone(),
                                tool: route.tool.clone(),
                            },
                        );
                    } else if matches!(effect, ToolEffect::ProcessExecution) {
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
                }
                self.authority.authorize_tool_call(&auth_request)?;
                let paths = paths_from_tool_arguments(&arguments);
                let envelope = ActionEnvelope {
                    effect: format!("{effect:?}"),
                    paths,
                    autonomy: self.autonomy.as_str().into(),
                    risk_tier: Some(risk.tier.as_str().into()),
                    risk_category: Some(risk.category.as_str().into()),
                };
                let _ = crate::handoff::record_action_envelope(
                    &self.workspace_root,
                    &route.server,
                    &route.tool,
                    &envelope,
                );
                let _ = events
                    .send(AgentEvent::ToolCall {
                        server: route.server.clone(),
                        tool: route.tool.clone(),
                        arguments: crate::context_edit::scrub_secrets_in_json(&arguments),
                        effect,
                        envelope: Some(envelope),
                    })
                    .await;
                let (is_error, text, content, host_intent) = if route.host {
                    match self.call_host_tool(&route.tool, &arguments).await {
                        Ok((text, intent)) => (false, text.clone(), Value::String(text), intent),
                        Err(error) => {
                            let text = error.to_string();
                            (true, text.clone(), Value::String(text), None)
                        }
                    }
                } else {
                    let result = self
                        .mcp
                        .call_tool(&route.server, &route.tool, arguments.clone())
                        .await?;
                    (
                        result.is_error,
                        result.text.clone(),
                        if result.text.is_empty() {
                            result.content
                        } else {
                            Value::String(result.text)
                        },
                        None,
                    )
                };
                let scrubbed_text = crate::context_edit::scrub_secrets(&text);
                let raw_for_model = if text.is_empty() {
                    crate::context_edit::scrub_secrets(&serde_json::to_string(&content)?)
                } else {
                    scrubbed_text.clone()
                };
                let (model_content, _truncated) =
                    crate::context_edit::compact_tool_result_for_context(
                        &raw_for_model,
                        crate::context_edit::tool_result_max_chars_from_env(),
                    );
                let _ = events
                    .send(AgentEvent::ToolResult {
                        server: route.server.clone(),
                        tool: route.tool.clone(),
                        is_error,
                        text: scrubbed_text,
                    })
                    .await;
                if let Some((action, path, url)) = host_intent {
                    let _ = events
                        .send(AgentEvent::HostIntent { action, path, url })
                        .await;
                }
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": call.id,
                    "content": model_content,
                }));
                // C4 thin: model-invoked compact at resolve boundary.
                if !is_error && route.server == "ade" && route.tool == "compact_context" {
                    let reason = arguments
                        .get("reason")
                        .and_then(Value::as_str)
                        .unwrap_or("model_compact");
                    let intent = arguments.get("intent").and_then(Value::as_str);
                    let next = arguments.get("next").and_then(Value::as_str);
                    let verify = arguments.get("verify").and_then(Value::as_str);
                    let (compacted, summary) =
                        self.apply_and_persist_boundary(&messages, reason, intent, next, verify);
                    messages = compacted;
                    let _ = events
                        .send(AgentEvent::ContextCompacted {
                            trigger: summary.trigger,
                            tokens_before: summary.tokens_before,
                            tokens_after: summary.tokens_after,
                            occupancy_before: summary.occupancy_before,
                        })
                        .await;
                }
            }
        }

        let detail = format!(
            "agent exceeded the {}-round tool-call limit",
            self.max_tool_rounds
        );
        let _ = events
            .send(AgentEvent::BudgetExhausted {
                kind: "tool_rounds".into(),
                limit: self.max_tool_rounds as u64,
                used: self.max_tool_rounds as u64,
                detail: detail.clone(),
            })
            .await;
        Err(AdeError::Budget(detail))
    }

    fn apply_and_persist_boundary(
        &self,
        messages: &[Value],
        trigger: &str,
        intent: Option<&str>,
        next: Option<&str>,
        verify: Option<&str>,
    ) -> (Vec<Value>, crate::context_edit::BoundaryCapsuleSummary) {
        let keep = crate::context_edit::tool_result_keep_rounds_from_env();
        let (compacted, summary) = crate::context_edit::apply_boundary_compact(
            messages,
            keep,
            trigger,
            self.model.context_limit,
            crate::context_edit::BoundaryCompactExtras {
                intent,
                decisions: &[],
                paths: &[],
                failing: None,
                next,
                verify,
            },
        );
        let mut capsule = ade_core::handoff::HandoffCapsule::new(
            intent.unwrap_or("boundary compact"),
            "boundary_compact",
        );
        capsule.session_id = Some(self.session_id.to_string());
        capsule.provider = Some(self.provider.name().to_string());
        capsule.model = Some(self.model.id.clone());
        capsule.turn_status = Some(format!("compacted:{trigger}"));
        capsule.next_safe_command = next.map(str::to_string);
        capsule.compact_summary = Some(summary.summary.clone());
        fill_capsule_from_boundary_summary(&mut capsule, &summary.summary);
        capsule.context_compaction = Some(ade_core::handoff::HandoffContextCompaction {
            tokens_estimated: summary.tokens_after as u32,
            status: if summary.occupancy_before >= 0.70 {
                "warning".into()
            } else {
                "green".into()
            },
            sections: vec![ade_core::handoff::HandoffPromptSection {
                name: "boundary_capsule".into(),
                tokens: summary.tokens_after as u32,
                truncated: true,
            }],
        });
        let _ = crate::handoff::HandoffManager::new(&self.workspace_root).save_capsule(&capsule);
        let id = "boundary".to_string();
        let _ = crate::handoff::write_continuity_last_write(&self.workspace_root, &capsule, &id);
        let dir = self.workspace_root.join(".ade").join("continuity");
        let _ = std::fs::create_dir_all(&dir);
        let snap = json!({
            "schema": "ade.continuity-last-boundary/v1",
            "kind": "boundary_compact",
            "trigger": trigger,
            "tokensBefore": summary.tokens_before,
            "tokensAfter": summary.tokens_after,
            "occupancyBefore": summary.occupancy_before,
            "summary": summary.summary,
        });
        let _ = std::fs::write(
            dir.join("last-boundary.json"),
            serde_json::to_string_pretty(&snap).unwrap_or_default(),
        );
        (compacted, summary)
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
                        let text = crate::context_edit::scrub_secrets(&text);
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

    async fn call_host_tool(
        &self,
        tool: &str,
        arguments: &Value,
    ) -> Result<(String, Option<(String, Option<String>, Option<String>)>), AdeError> {
        match tool {
            "activate_skill" => {
                let name = arguments
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                let skill = SkillLoader::new(&self.workspace_root).activate(name)?;
                Ok((skill_body_block(&skill), None))
            }
            "compact_context" => {
                // C4: rubric gate — require non-empty reason; reject mid-stuck patterns.
                let reason = arguments
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                if reason.is_empty() {
                    return Err(AdeError::Config(
                        "ade.compact_context: reason is required (e.g. subtask_resolved, converging, handoff)"
                            .into(),
                    ));
                }
                let reason_l = reason.to_ascii_lowercase();
                if reason_l.contains("stuck")
                    || reason_l.contains("debugging")
                    || reason_l.contains("mid-derivation")
                    || reason_l.contains("mid_derivation")
                {
                    return Err(AdeError::Config(
                        "ade.compact_context: suppressed — rubric says do not compact while stuck/debugging or mid-derivation; finish the sub-task or raise Effort first"
                            .into(),
                    ));
                }
                let intent = arguments
                    .get("intent")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                Ok((
                    format!(
                    "ade.compact_context: accepted trigger={reason}; intent={intent}. Harness will emit a boundary capsule and clear older tool blobs. Fire when a sub-task is resolved/converging; suppress mid-derivation or while stuck debugging."
                    ),
                    None,
                ))
            }
            "web_fetch" => {
                let url = arguments
                    .get("url")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                Ok((crate::web::web_fetch(url).await?, None))
            }
            "web_search" => {
                let query = arguments
                    .get("query")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                Ok((crate::web::web_search(query).await?, None))
            }
            "create_named" => {
                let name = arguments
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                let parent = arguments
                    .get("parent")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                let root = crate::workspace_scaffold::create_named_workspace_on_disk(name, parent)?;
                let path = root.display().to_string();
                Ok((
                    format!(
                        "Created ADE workspace at {path}. Desktop will attach it. Continue in the next turn under Apply to write project files (do not paste a full blueprint into chat)."
                    ),
                    Some(("attach_workspace".into(), Some(path), None)),
                ))
            }
            "open" => {
                let url = arguments
                    .get("url")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                let normalized = crate::workspace_scaffold::normalize_browser_open_url(url)?;
                Ok((
                    format!("Opening {normalized} in ADE Browser."),
                    Some(("open_browser".into(), None, Some(normalized))),
                ))
            }
            "read_file" => Ok((self.host_read_file(arguments)?, None)),
            "write_file" => Ok((self.host_write_file(arguments)?, None)),
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
                Ok((
                    crate::shell::run_command(
                        &self.workspace_root,
                        command,
                        timeout_secs,
                        crate::shell::ShellOptions {
                            inspect_only: !self.autonomy.allows_mutating_tools(),
                            cwd,
                        },
                    )
                    .await?,
                    None,
                ))
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
            if !self.contract_allows_effect(effect) {
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
            if !self.contract_allows_effect(effect) {
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

fn fill_capsule_from_boundary_summary(
    capsule: &mut ade_core::handoff::HandoffCapsule,
    summary: &str,
) {
    let mut section = "";
    for line in summary.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("decisions:") {
            section = "decisions";
            continue;
        }
        if trimmed.starts_with("paths:") {
            section = "paths";
            continue;
        }
        if trimmed.starts_with("failing:")
            || trimmed.starts_with("next:")
            || trimmed.starts_with("verify:")
            || trimmed.starts_with("intent:")
            || trimmed.starts_with("trigger:")
            || trimmed.starts_with("note:")
            || trimmed.starts_with("ade.boundary-capsule")
        {
            section = "";
            continue;
        }
        let Some(item) = trimmed.strip_prefix("- ") else {
            continue;
        };
        if item == "(none)" {
            continue;
        }
        match section {
            "decisions" if capsule.decisions_touched.len() < 8 => {
                capsule.decisions_touched.push(item.to_string());
            }
            "paths" if capsule.changed_paths.len() < 12 => {
                capsule.changed_paths.push(item.to_string());
            }
            _ => {}
        }
    }
}

/// Collect workspace-relative paths (and path-like args) for E1 envelopes.
pub fn paths_from_tool_arguments(arguments: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let Some(obj) = arguments.as_object() else {
        return out;
    };
    for key in ["path", "file", "filename", "target", "cwd", "owned_path"] {
        if let Some(Value::String(s)) = obj.get(key) {
            let trimmed = s.trim();
            if !trimmed.is_empty() && !out.iter().any(|p| p == trimmed) {
                out.push(trimmed.to_string());
            }
        }
    }
    if let Some(Value::Array(paths)) = obj.get("paths") {
        for item in paths {
            if let Some(s) = item.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() && !out.iter().any(|p| p == trimmed) {
                    out.push(trimmed.to_string());
                }
            }
        }
    }
    out.truncate(12);
    out
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
            tool: "compact_context".into(),
            description: "SelfCompact-style context compaction (C4). Fire when a sub-task is resolved or converging; suppress mid-derivation or while stuck. Writes a structured boundary capsule and clears older tool blobs.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "reason": {
                        "type": "string",
                        "description": "Why compact now (e.g. subtask_resolved, occupancy, handoff)"
                    },
                    "intent": {
                        "type": "string",
                        "description": "One-line intent for the capsule"
                    },
                    "next": {
                        "type": "string",
                        "description": "Next safe step after compact"
                    },
                    "verify": {
                        "type": "string",
                        "description": "Verify pointer / gate if known"
                    }
                },
                "required": ["reason"],
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
            server: "workspace".into(),
            tool: "create_named".into(),
            description: "Create a new named ADE project folder (AGENTS.md + .ade/) on Desktop by default, then Desktop attaches it. Use for greenfield apps/demos — do not paste full project blueprints into chat. Continue writing files in the next turn under Apply.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Folder name only (e.g. oee-dashboard)"
                    },
                    "parent": {
                        "type": "string",
                        "description": "Optional parent directory (default: Desktop)"
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
            server: "browser".into(),
            tool: "open".into(),
            description: "Open a URL in the ADE in-shell Browser tab. For local servers use http://localhost:PORT (not https).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "http(s) URL or localhost:PORT"
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

    #[tokio::test]
    async fn tool_round_limit_emits_budget_exhausted_not_provider() {
        let root = std::env::temp_dir().join(format!("ade-session-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let provider = Arc::new(MockProvider {
            completions: Mutex::new(vec![ProviderCompletion {
                text: String::new(),
                tool_calls: vec![ProviderToolCall {
                    id: "call-1".into(),
                    name: "fs__read_file".into(),
                    arguments: r#"{"path":"missing.txt"}"#.into(),
                }],
                usage: ProviderUsage {
                    input_tokens: 10,
                    output_tokens: 5,
                },
            }]),
        });
        let session = AgentSession::new(provider, model(), McpHost::new(), "system")
            .with_workspace(&root)
            .with_max_tool_rounds(1)
            .with_spend_caps(SpendCaps::unlimited(), ledger().await);
        let mut receiver = session.start_turn("read something");
        let mut saw_budget = false;
        let mut failed_msg = None;
        while let Some(event) = receiver.recv().await {
            match event {
                AgentEvent::BudgetExhausted { kind, .. } => {
                    assert_eq!(kind, "tool_rounds");
                    saw_budget = true;
                }
                AgentEvent::Failed { error } => {
                    failed_msg = Some(error);
                    break;
                }
                _ => {}
            }
        }
        assert!(saw_budget, "expected BudgetExhausted event");
        let error = failed_msg.expect("expected Failed after BudgetExhausted");
        assert!(
            error.starts_with("Budget exhausted:"),
            "expected Budget exhausted prefix, got {error}"
        );
        assert!(!error.contains("Provider error:"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn output_token_budget_emits_budget_exhausted_not_provider() {
        let root = std::env::temp_dir().join(format!("ade-session-{}", Uuid::new_v4()));
        let provider = Arc::new(MockProvider {
            completions: Mutex::new(vec![ProviderCompletion {
                text: "partial".into(),
                tool_calls: vec![],
                usage: ProviderUsage {
                    input_tokens: 10,
                    output_tokens: 50,
                },
            }]),
        });
        let session = AgentSession::new(provider, model(), McpHost::new(), "system")
            .with_workspace(&root)
            .with_max_tokens(Some(10))
            .with_spend_caps(SpendCaps::unlimited(), ledger().await);
        let mut receiver = session.start_turn("hello");
        let mut saw_budget = false;
        let mut failed_msg = None;
        while let Some(event) = receiver.recv().await {
            match event {
                AgentEvent::BudgetExhausted {
                    kind, limit, used, ..
                } => {
                    assert_eq!(kind, "output_tokens");
                    assert_eq!(limit, 10);
                    assert!(used > 10);
                    saw_budget = true;
                }
                AgentEvent::Failed { error } => {
                    failed_msg = Some(error);
                    break;
                }
                AgentEvent::Completed { .. } => panic!("should not complete under token budget"),
                _ => {}
            }
        }
        assert!(saw_budget, "expected BudgetExhausted event");
        let error = failed_msg.expect("expected Failed after BudgetExhausted");
        assert!(error.starts_with("Budget exhausted:"));
        assert!(!error.contains("Provider error:"));
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
