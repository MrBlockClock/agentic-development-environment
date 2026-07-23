use ade_core::error::AdeError;
use ade_core::money::Money;
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct ProviderConfig {
    pub name: String,
    pub base_url: String,
    /// Kept out of Debug/Serialize so credentials cannot leak through diagnostics.
    pub api_key: Option<String>,
    pub models: Vec<ModelConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub id: String,
    pub name: String,
    pub context_limit: u64,
    pub output_limit: u64,
    /// Price per million input tokens in micro-dollars.
    pub cost_per_input_mtok: Money,
    /// Price per million output tokens in micro-dollars.
    pub cost_per_output_mtok: Money,
}

impl ModelConfig {
    /// Strict pricing validation: priced models must declare non-zero limits.
    pub fn validate_spend_limits(&self) -> Result<(), AdeError> {
        let priced =
            self.cost_per_input_mtok > Money::ZERO || self.cost_per_output_mtok > Money::ZERO;
        if !priced {
            return Ok(());
        }
        if self.context_limit == 0 || self.output_limit == 0 {
            return Err(AdeError::Config(format!(
                "priced model '{}' requires non-zero context_limit and output_limit for spend reservation",
                self.id
            )));
        }
        if self.output_limit > self.context_limit {
            return Err(AdeError::Config(format!(
                "model '{}' output_limit exceeds context_limit",
                self.id
            )));
        }
        Ok(())
    }

    pub fn is_priced(&self) -> bool {
        self.cost_per_input_mtok > Money::ZERO || self.cost_per_output_mtok > Money::ZERO
    }

    /// Worst-case round cost using declared context and output limits (hard ceiling).
    pub fn max_round_cost(&self) -> Result<Money, AdeError> {
        self.validate_spend_limits()?;
        if !self.is_priced() {
            return Ok(Money::ZERO);
        }
        Ok(
            Money::cost_for_tokens(self.context_limit, self.cost_per_input_mtok)
                + Money::cost_for_tokens(self.output_limit, self.cost_per_output_mtok),
        )
    }

    /// Honest per-round reserve from estimated input + bounded output (not full context).
    pub fn estimate_round_cost(
        &self,
        estimated_input_tokens: u64,
        output_budget_tokens: u64,
    ) -> Result<Money, AdeError> {
        self.validate_spend_limits()?;
        if !self.is_priced() {
            return Ok(Money::ZERO);
        }
        let input = estimated_input_tokens
            .max(256)
            .min(self.context_limit)
            .saturating_mul(12)
            .saturating_div(10); // ~20% cushion
        let output = output_budget_tokens
            .max(256)
            .min(self.output_limit.max(256));
        let estimate = Money::cost_for_tokens(input, self.cost_per_input_mtok)
            + Money::cost_for_tokens(output, self.cost_per_output_mtok);
        let ceiling = self.max_round_cost()?;
        Ok(if estimate > ceiling {
            ceiling
        } else {
            estimate
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone)]
pub struct ProviderRequest {
    pub model: ModelConfig,
    pub messages: Vec<Value>,
    pub tools: Vec<ProviderTool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl ProviderUsage {
    pub fn cost_money(&self, model: &ModelConfig) -> Money {
        Money::cost_for_tokens(self.input_tokens, model.cost_per_input_mtok)
            + Money::cost_for_tokens(self.output_tokens, model.cost_per_output_mtok)
    }

    pub fn exceeds_model_limits(&self, model: &ModelConfig) -> bool {
        // Unpriced / free BYOK turns use 0/0 = "no hard limit configured".
        // Treating that as a real ceiling falsely fails free OpenCode/FreeLLM turns.
        let over_context = model.context_limit > 0 && self.input_tokens > model.context_limit;
        let over_output = model.output_limit > 0 && self.output_tokens > model.output_limit;
        over_context || over_output
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCompletion {
    pub text: String,
    pub tool_calls: Vec<ProviderToolCall>,
    pub usage: ProviderUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderStreamEvent {
    TextDelta { text: String },
    Usage { usage: ProviderUsage },
}

#[async_trait]
pub trait ChatProvider: Send + Sync {
    fn name(&self) -> &str;

    async fn stream(
        &self,
        request: ProviderRequest,
        events: mpsc::Sender<ProviderStreamEvent>,
    ) -> Result<ProviderCompletion, AdeError>;
}

/// OpenAI-compatible chat-completions transport. This supports OpenAI itself
/// and local/provider gateways that implement the same SSE contract.
pub struct OpenAiCompatibleProvider {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl OpenAiCompatibleProvider {
    pub fn new(config: ProviderConfig) -> Result<Self, AdeError> {
        validate_config(&config)?;
        Ok(Self {
            config,
            client: reqwest::Client::new(),
        })
    }

    fn endpoint(&self) -> String {
        format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        )
    }
}

#[async_trait]
impl ChatProvider for OpenAiCompatibleProvider {
    fn name(&self) -> &str {
        &self.config.name
    }

    async fn stream(
        &self,
        request: ProviderRequest,
        events: mpsc::Sender<ProviderStreamEvent>,
    ) -> Result<ProviderCompletion, AdeError> {
        let body = request_body(&request);

        let mut http = self.client.post(self.endpoint()).json(&body);
        if let Some(key) = &self.config.api_key {
            http = http.bearer_auth(key);
        }
        let response = http
            .send()
            .await
            .map_err(|error| AdeError::Provider(format_request_error(&self.config, &error)))?;
        let status = response.status();
        if !status.is_success() {
            let detail = response.text().await.unwrap_or_default();
            let base = self.config.base_url.trim_end_matches('/');
            let hint = if status.as_u16() == 401 {
                let mut fix = format!(
                    " — vault key for provider '{}' rejected by {base}.",
                    self.config.name
                );
                if base.contains("opencode.ai") {
                    fix.push_str(
                        " Fix: Keys → OpenCode Zen → paste the Zen key from opencode.ai/auth (not a FreeLLMAPI key). Import OpenCode auth if auth.json has an `opencode` entry.",
                    );
                } else if base.contains("127.0.0.1") || base.contains("localhost") {
                    fix.push_str(
                        " Fix: Keys → FreeLLMAPI → paste the key for that local gateway (:31415 Desktop vs :3001 Docker are different).",
                    );
                } else {
                    fix.push_str(
                        " Fix: Keys → select that provider → paste a fresh key for this exact base URL.",
                    );
                }
                fix
            } else {
                String::new()
            };
            return Err(AdeError::Provider(format!(
                "{} → {base} returned HTTP {status}: {}{hint}",
                self.config.name,
                truncate(&detail, 400),
                hint = hint
            )));
        }

        let mut stream = response.bytes_stream();
        let mut buffer = Vec::new();
        let mut text = String::new();
        let mut tool_calls: BTreeMap<usize, ProviderToolCall> = BTreeMap::new();
        let mut usage = ProviderUsage::default();

        while let Some(chunk) = stream.next().await {
            buffer.extend_from_slice(
                &chunk.map_err(|error| {
                    AdeError::Provider(format!("response stream failed: {error}"))
                })?,
            );
            while let Some((end, delimiter_len)) = sse_event_boundary(&buffer) {
                let raw = buffer.drain(..end).collect::<Vec<_>>();
                buffer.drain(..delimiter_len);
                let event = std::str::from_utf8(&raw)
                    .map_err(|_| AdeError::Provider("provider returned invalid UTF-8".into()))?;
                if process_sse_event(event, &mut text, &mut tool_calls, &mut usage, &events).await?
                {
                    return Ok(ProviderCompletion {
                        text,
                        tool_calls: tool_calls.into_values().collect(),
                        usage,
                    });
                }
            }
        }

        Ok(ProviderCompletion {
            text,
            tool_calls: tool_calls.into_values().collect(),
            usage,
        })
    }
}

fn request_body(request: &ProviderRequest) -> Value {
    let tools: Vec<Value> = request
        .tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.input_schema,
                }
            })
        })
        .collect();
    let mut body = json!({
        "model": request.model.id,
        "messages": request.messages,
        "stream": true,
        "stream_options": { "include_usage": true },
    });
    if request.model.output_limit > 0 {
        body["max_tokens"] = Value::from(request.model.output_limit);
    }
    if !tools.is_empty() {
        body["tools"] = Value::Array(tools);
        body["tool_choice"] = Value::String("auto".into());
    }
    body
}

async fn process_sse_event(
    event: &str,
    text: &mut String,
    tool_calls: &mut BTreeMap<usize, ProviderToolCall>,
    usage: &mut ProviderUsage,
    events: &mpsc::Sender<ProviderStreamEvent>,
) -> Result<bool, AdeError> {
    for line in event.lines() {
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            return Ok(true);
        }
        if data.is_empty() {
            continue;
        }
        let chunk: Value = serde_json::from_str(data).map_err(|error| {
            AdeError::Provider(format!("invalid streaming response chunk: {error}"))
        })?;
        if let Some(chunk_usage) = chunk.get("usage").filter(|value| !value.is_null()) {
            usage.input_tokens = chunk_usage
                .get("prompt_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(usage.input_tokens);
            usage.output_tokens = chunk_usage
                .get("completion_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(usage.output_tokens);
            let _ = events
                .send(ProviderStreamEvent::Usage {
                    usage: usage.clone(),
                })
                .await;
        }
        let Some(delta) = chunk.pointer("/choices/0/delta") else {
            continue;
        };
        if let Some(content) = delta.get("content").and_then(Value::as_str) {
            text.push_str(content);
            let _ = events
                .send(ProviderStreamEvent::TextDelta {
                    text: content.to_string(),
                })
                .await;
        }
        if let Some(calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for call in calls {
                let index = call.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                let target = tool_calls.entry(index).or_insert_with(|| ProviderToolCall {
                    id: String::new(),
                    name: String::new(),
                    arguments: String::new(),
                });
                if let Some(id) = call.get("id").and_then(Value::as_str) {
                    target.id = id.to_string();
                }
                if let Some(name) = call.pointer("/function/name").and_then(Value::as_str) {
                    target.name.push_str(name);
                }
                if let Some(arguments) = call.pointer("/function/arguments").and_then(Value::as_str)
                {
                    target.arguments.push_str(arguments);
                }
            }
        }
    }
    Ok(false)
}

fn sse_event_boundary(buffer: &[u8]) -> Option<(usize, usize)> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| (index, 4))
        .or_else(|| {
            buffer
                .windows(2)
                .position(|window| window == b"\n\n")
                .map(|index| (index, 2))
        })
}

fn validate_config(config: &ProviderConfig) -> Result<(), AdeError> {
    if config.name.trim().is_empty() {
        return Err(AdeError::Provider("provider name cannot be empty".into()));
    }
    let url = reqwest::Url::parse(&config.base_url)
        .map_err(|error| AdeError::Provider(format!("invalid provider base URL: {error}")))?;
    let local_http =
        url.scheme() == "http" && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if url.scheme() != "https" && !local_http {
        return Err(AdeError::Provider(
            "provider URL must use HTTPS (HTTP is allowed only for loopback)".into(),
        ));
    }
    if config.models.is_empty() {
        return Err(AdeError::Provider(
            "provider must configure at least one model".into(),
        ));
    }
    Ok(())
}

fn format_request_error(config: &ProviderConfig, error: &reqwest::Error) -> String {
    let base = config.base_url.trim_end_matches('/');
    if error.is_connect() {
        if base.contains("127.0.0.1") || base.contains("localhost") {
            return format!(
                "cannot reach {base}/chat/completions — is the local gateway running? (FreeLLMAPI: docker compose up -d in ~/freellmapi, or start the FreeLLMAPI Desktop app)"
            );
        }
        return format!("cannot reach {base}/chat/completions: {error}");
    }
    if error.is_timeout() {
        return format!("provider timed out talking to {base}: {error}");
    }
    format!("request failed talking to {base}: {error}")
}

fn truncate(value: &str, max: usize) -> &str {
    value.get(..max).unwrap_or(value)
}

pub struct ProviderManager {
    providers: Vec<ProviderConfig>,
}

impl Default for ProviderManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderManager {
    pub fn new() -> Self {
        Self { providers: vec![] }
    }

    pub fn register(&mut self, provider: ProviderConfig) -> Result<(), AdeError> {
        validate_config(&provider)?;
        self.providers.retain(|item| item.name != provider.name);
        self.providers.push(provider);
        Ok(())
    }

    pub fn list_providers(&self) -> Vec<String> {
        self.providers
            .iter()
            .map(|provider| provider.name.clone())
            .collect()
    }

    pub fn select_model(&self, task_type: &str, budget: Option<Money>) -> Option<ModelConfig> {
        let prefer_strong = matches!(
            task_type.to_ascii_lowercase().as_str(),
            "architecture" | "migration" | "security" | "review"
        );
        self.providers
            .iter()
            .flat_map(|provider| provider.models.iter())
            .filter(|model| {
                budget.is_none_or(|limit| {
                    model.cost_per_input_mtok.max(model.cost_per_output_mtok) <= limit
                })
            })
            .max_by(|left, right| {
                let left_rank = if prefer_strong {
                    left.output_limit
                } else {
                    u64::MAX - left.output_limit
                };
                let right_rank = if prefer_strong {
                    right.output_limit
                } else {
                    u64::MAX - right.output_limit
                };
                left_rank.cmp(&right_rank)
            })
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn assembles_text_tools_and_usage_from_sse() {
        let (tx, mut rx) = mpsc::channel(8);
        let mut text = String::new();
        let mut calls = BTreeMap::new();
        let mut usage = ProviderUsage::default();
        process_sse_event(
            r#"data: {"choices":[{"delta":{"content":"Hi ","tool_calls":[{"index":0,"id":"c1","function":{"name":"fs__write","arguments":"{\"path\":"}}]}}]}"#,
            &mut text,
            &mut calls,
            &mut usage,
            &tx,
        )
        .await
        .unwrap();
        process_sse_event(
            r#"data: {"choices":[{"delta":{"content":"there","tool_calls":[{"index":0,"function":{"arguments":"\"a\"}"}}]}}],"usage":{"prompt_tokens":10,"completion_tokens":4}}"#,
            &mut text,
            &mut calls,
            &mut usage,
            &tx,
        )
        .await
        .unwrap();
        drop(tx);

        assert_eq!(text, "Hi there");
        assert_eq!(calls[&0].name, "fs__write");
        assert_eq!(calls[&0].arguments, r#"{"path":"a"}"#);
        assert_eq!(usage.input_tokens, 10);
        assert!(matches!(
            rx.recv().await,
            Some(ProviderStreamEvent::TextDelta { .. })
        ));
    }

    #[test]
    fn requires_tls_except_for_loopback() {
        let model = ModelConfig {
            id: "model".into(),
            name: "Model".into(),
            context_limit: 1,
            output_limit: 1,
            cost_per_input_mtok: Money::ZERO,
            cost_per_output_mtok: Money::ZERO,
        };
        let remote = ProviderConfig {
            name: "remote".into(),
            base_url: "http://example.com/v1".into(),
            api_key: None,
            models: vec![model.clone()],
        };
        assert!(validate_config(&remote).is_err());
        assert!(validate_config(&ProviderConfig {
            base_url: "http://127.0.0.1:11434/v1".into(),
            ..remote
        })
        .is_ok());
    }

    #[test]
    fn request_caps_provider_output() {
        let body = request_body(&ProviderRequest {
            model: ModelConfig {
                id: "smoke-model".into(),
                name: "Smoke model".into(),
                context_limit: 8_192,
                output_limit: 16,
                cost_per_input_mtok: Money::ZERO,
                cost_per_output_mtok: Money::ZERO,
            },
            messages: vec![json!({ "role": "user", "content": "ping" })],
            tools: vec![],
        });

        assert_eq!(body["max_tokens"], 16);
    }

    #[test]
    fn zero_limits_mean_unlimited_usage_check() {
        let model = ModelConfig {
            id: "free".into(),
            name: "Free".into(),
            context_limit: 0,
            output_limit: 0,
            cost_per_input_mtok: Money::ZERO,
            cost_per_output_mtok: Money::ZERO,
        };
        let usage = ProviderUsage {
            input_tokens: 3_737,
            output_tokens: 187,
        };
        assert!(!usage.exceeds_model_limits(&model));
    }
}
