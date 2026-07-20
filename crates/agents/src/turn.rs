use crate::authority::AuthorityEnforcer;
use crate::autonomy::AutonomyLevel;
use crate::context::PromptAssembler;
use crate::handoff::HandoffManager;
use crate::mcp::McpHost;
use crate::provider::{ChatProvider, ModelConfig, OpenAiCompatibleProvider, ProviderConfig};
use crate::session::{AgentEvent, AgentSession, AgentTurnResult};
use crate::skills::SkillLoader;
use crate::spend::{SpendCaps, SpendGuard};
use crate::start_prompt::StartPromptBuilder;
use ade_core::audit::{AuditMode, AuditRunner};
use ade_core::error::AdeError;
use ade_core::handoff::{HandoffCapsule, HandoffContextCompaction};
use ade_core::ignore::SensitivePathPolicy;
use ade_core::money::Money;
use ade_core::verify::{VerifyGate, VerifyStatus};
use ade_db::repo::{AdeDatabase, DbConfig};
use ade_db::secrets::{NativeProviderKeyVault, ProviderKeyVault};
use ade_db::usage_ledger::UsageLedgerStore;
use ade_workflow::parallel::LeaseManager;
use ade_workflow::verify::VerifyRunner;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

const DEFAULT_HANDOFF_CHARS: usize = 1_500;
const DEFAULT_MAX_TOOL_ROUNDS: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTurnSpec {
    pub prompt: String,
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub input_cost_per_mtok: Money,
    pub output_cost_per_mtok: Money,
    pub context_limit: u64,
    pub output_limit: u64,
    pub profile: String,
    pub workspace_root: PathBuf,
    #[serde(default)]
    pub owned_paths: Vec<String>,
    #[serde(default = "default_handoff_chars")]
    pub handoff_chars: usize,
}

fn default_handoff_chars() -> usize {
    DEFAULT_HANDOFF_CHARS
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnUsageRecord {
    pub session_id: Uuid,
    pub provider: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_micros: i64,
    pub tool_calls: usize,
}

#[async_trait]
pub trait UsageSink: Send + Sync {
    async fn record_turn(
        &self,
        workspace_root: &Path,
        result: &AgentTurnResult,
    ) -> Result<(), AdeError>;
}

/// Persists structured turn completion into the usage ledger as a committed summary row.
pub struct LedgerUsageSink {
    ledger: UsageLedgerStore,
}

impl LedgerUsageSink {
    pub fn new(ledger: UsageLedgerStore) -> Self {
        Self { ledger }
    }
}

#[async_trait]
impl UsageSink for LedgerUsageSink {
    async fn record_turn(
        &self,
        workspace_root: &Path,
        result: &AgentTurnResult,
    ) -> Result<(), AdeError> {
        // Completion path already reconciled per-round reservations. This sink
        // records a final analytics-friendly committed summary for the turn.
        let reservation = self
            .ledger
            .reserve(ade_db::usage_ledger::ReserveRequest {
                session_id: result.session_id,
                workspace_root: workspace_root.display().to_string(),
                actor: None,
                scope: "turn_summary".into(),
                period_key: format!("session:{}", result.session_id),
                provider: Some(result.provider.clone()),
                model: Some(result.model.clone()),
                estimate: Money::from_micros(result.cost_micros),
                hard_cap: Money::from_micros(i64::MAX / 4),
                ttl_secs: 30,
            })
            .await?;
        self.ledger
            .commit(ade_db::usage_ledger::UsageCommit {
                reservation_id: reservation.id,
                actual: Money::from_micros(result.cost_micros),
                input_tokens: result.usage.input_tokens,
                output_tokens: result.usage.output_tokens,
            })
            .await?;
        Ok(())
    }
}

pub struct AgentTurnBuilder {
    spec: AgentTurnSpec,
    mcp: Option<McpHost>,
    spend_caps: Option<SpendCaps>,
    cancel: Option<Arc<AtomicBool>>,
    request_timeout: Duration,
    usage: Option<Arc<dyn UsageSink>>,
    ledger: Option<UsageLedgerStore>,
    actor: Option<String>,
    lease_agent_id: Option<Uuid>,
    /// Primary workspace used for leases/handoff/spend when execution occurs
    /// inside an isolated worktree. Defaults to `spec.workspace_root`.
    coordination_root: Option<PathBuf>,
    key_vault: Arc<dyn ProviderKeyVault>,
    provider_transport: Option<Arc<dyn ChatProvider>>,
    autonomy: AutonomyLevel,
    max_tool_rounds: usize,
    max_tokens: Option<u64>,
    verify_on_complete: Option<VerifyGate>,
}

impl AgentTurnBuilder {
    pub fn new(spec: AgentTurnSpec) -> Self {
        Self {
            spec,
            mcp: None,
            spend_caps: None,
            cancel: None,
            request_timeout: Duration::from_secs(120),
            usage: None,
            ledger: None,
            actor: None,
            lease_agent_id: None,
            coordination_root: None,
            key_vault: Arc::new(NativeProviderKeyVault),
            provider_transport: None,
            autonomy: AutonomyLevel::Propose,
            max_tool_rounds: DEFAULT_MAX_TOOL_ROUNDS,
            max_tokens: None,
            verify_on_complete: None,
        }
    }

    pub fn mcp(mut self, mcp: McpHost) -> Self {
        self.mcp = Some(mcp);
        self
    }

    pub fn spend_caps(mut self, caps: SpendCaps) -> Self {
        self.spend_caps = Some(caps);
        self
    }

    pub fn cancel_flag(mut self, flag: Arc<AtomicBool>) -> Self {
        self.cancel = Some(flag);
        self
    }

    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    pub fn usage_sink(mut self, sink: Arc<dyn UsageSink>) -> Self {
        self.usage = Some(sink);
        self
    }

    pub fn ledger(mut self, ledger: UsageLedgerStore) -> Self {
        self.ledger = Some(ledger);
        self
    }

    pub fn actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    /// Bind write authority to this agent's active durable path leases.
    pub fn lease_agent(mut self, agent_id: Uuid) -> Self {
        self.lease_agent_id = Some(agent_id);
        self
    }

    /// Use a different root for leases, handoff capsules, and spend tracking
    /// while executing tools against `spec.workspace_root` (e.g. a worktree).
    pub fn coordination_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.coordination_root = Some(root.into());
        self
    }

    pub fn autonomy(mut self, autonomy: AutonomyLevel) -> Self {
        self.autonomy = autonomy;
        self
    }

    pub fn max_tool_rounds(mut self, rounds: usize) -> Self {
        self.max_tool_rounds = rounds.max(1);
        self
    }

    pub fn max_tokens(mut self, max_tokens: Option<u64>) -> Self {
        self.max_tokens = max_tokens.filter(|value| *value > 0);
        self
    }

    pub fn verify_on_complete(mut self, gate: Option<VerifyGate>) -> Self {
        self.verify_on_complete = gate;
        self
    }

    pub fn key_vault(mut self, key_vault: Arc<dyn ProviderKeyVault>) -> Self {
        self.key_vault = key_vault;
        self
    }

    pub fn provider_transport(mut self, provider: Arc<dyn ChatProvider>) -> Self {
        self.provider_transport = Some(provider);
        self
    }

    pub async fn prepare(self) -> Result<AgentTurnService, AdeError> {
        let model = ModelConfig {
            id: self.spec.model.clone(),
            name: self.spec.model.clone(),
            context_limit: self.spec.context_limit,
            output_limit: self.spec.output_limit,
            cost_per_input_mtok: self.spec.input_cost_per_mtok,
            cost_per_output_mtok: self.spec.output_cost_per_mtok,
        };
        model.validate_spend_limits()?;

        let transport: Arc<dyn ChatProvider> = match self.provider_transport {
            Some(provider) => provider,
            None => {
                let api_key = self
                    .key_vault
                    .get(&self.spec.profile, &self.spec.provider)?;
                Arc::new(OpenAiCompatibleProvider::new(ProviderConfig {
                    name: self.spec.provider.clone(),
                    base_url: self.spec.base_url.clone(),
                    api_key,
                    models: vec![model.clone()],
                })?)
            }
        };

        let coordination_root = self
            .coordination_root
            .unwrap_or_else(|| self.spec.workspace_root.clone());
        let owned_paths = match self.lease_agent_id {
            Some(agent_id) => LeaseManager::new(&coordination_root)
                .resolve_owned_paths(agent_id, &self.spec.owned_paths)?,
            None => self.spec.owned_paths.clone(),
        };
        let authority = AuthorityEnforcer::load(&self.spec.workspace_root, owned_paths)?;
        let effective_owned_paths = authority.owned_paths();
        ade_workflow::plan_enforcement::PlanEnforcer::new().ensure_approved_plan(
            &self.spec.workspace_root,
            &effective_owned_paths,
            Some(&self.spec.prompt),
        )?;
        let handoff_summary = match HandoffManager::new(&coordination_root).load_latest() {
            Ok(handoff) => {
                let summary = handoff.prompt_summary(self.spec.handoff_chars);
                if summary
                    .split_whitespace()
                    .any(SensitivePathPolicy::is_secret_path)
                    || SensitivePathPolicy::is_secret_path(&summary)
                {
                    None
                } else {
                    Some(summary)
                }
            }
            Err(_) => None,
        };
        let skills_context = SkillLoader::new(&self.spec.workspace_root)
            .prompt_context(
                &self.spec.prompt,
                PromptAssembler::daily(self.spec.context_limit)
                    .budget()
                    .skills_tokens,
            )
            .unwrap_or_default();
        let assembled = PromptAssembler::daily(self.spec.context_limit).assemble(
            &format!(
                "{}\n\n{}",
                StartPromptBuilder::new().build(),
                self.autonomy.prompt_clause()
            ),
            &authority.prompt_context(),
            Some(skills_context.as_str()).filter(|text| !text.is_empty()),
            handoff_summary.as_deref(),
        );
        if matches!(assembled.status, crate::context::ContextStatus::Critical) {
            tracing::warn!(
                tokens = assembled.tokens_estimated,
                "assembled system prompt is over context budget"
            );
        }
        let context_compaction = assembled.compaction_metrics();
        let system_prompt = assembled.text;

        let ledger = match self.ledger {
            Some(ledger) => ledger,
            None => {
                let config = ade_core::config::AdeConfig::load()?;
                let db = AdeDatabase::open(&DbConfig::from_ade_config(&config)).await?;
                UsageLedgerStore::new(db.connect()?)
            }
        };
        let session_id = Uuid::new_v4();
        let caps = self.spend_caps.unwrap_or_else(SpendCaps::from_env);
        let mut guard = SpendGuard::new(&coordination_root, session_id, caps, ledger.clone());
        if let Some(actor) = self.actor {
            guard = guard.with_actor(actor);
        }

        let verify_on_complete = if self.autonomy.requires_verify_on_complete() {
            Some(self.verify_on_complete.unwrap_or(VerifyGate::G3))
        } else {
            self.verify_on_complete
        };

        let mut session = AgentSession::new(
            transport,
            model,
            self.mcp.unwrap_or_default(),
            system_prompt,
        )
        .with_authority(authority)
        .with_workspace(&self.spec.workspace_root)
        .with_spend_guard(guard)
        .with_request_timeout(self.request_timeout)
        .with_autonomy(self.autonomy)
        .with_max_tool_rounds(self.max_tool_rounds)
        .with_max_tokens(self.max_tokens);
        if let Some(cancel) = &self.cancel {
            session = session.with_cancel_flag(Arc::clone(cancel));
        }

        let usage = self
            .usage
            .unwrap_or_else(|| Arc::new(LedgerUsageSink::new(ledger)));
        let score_before = workspace_score(&self.spec.workspace_root);

        Ok(AgentTurnService {
            session,
            prompt: self.spec.prompt,
            workspace_root: self.spec.workspace_root.clone(),
            coordination_root,
            provider: self.spec.provider,
            model: self.spec.model,
            cancel: self.cancel,
            usage,
            score_before,
            context_compaction,
            effective_owned_paths,
            verify_on_complete,
        })
    }

    /// Build the same system prompt/authority/spend policy fingerprint used by adapters.
    pub fn policy_fingerprint(spec: &AgentTurnSpec, caps: &SpendCaps) -> String {
        format!(
            "profile={};provider={};model={};context={};output={};owned={};handoff={};session_cap={};daily_cap={}",
            spec.profile,
            spec.provider,
            spec.model,
            spec.context_limit,
            spec.output_limit,
            spec.owned_paths.join(","),
            spec.handoff_chars,
            caps.session.micros(),
            caps.daily.micros()
        )
    }
}

pub struct AgentTurnService {
    session: AgentSession,
    prompt: String,
    workspace_root: PathBuf,
    coordination_root: PathBuf,
    provider: String,
    model: String,
    cancel: Option<Arc<AtomicBool>>,
    usage: Arc<dyn UsageSink>,
    score_before: WorkspaceScore,
    context_compaction: HandoffContextCompaction,
    effective_owned_paths: Vec<String>,
    verify_on_complete: Option<VerifyGate>,
}

impl AgentTurnService {
    pub fn session_id(&self) -> Uuid {
        self.session.session_id()
    }

    pub fn effective_owned_paths(&self) -> &[String] {
        &self.effective_owned_paths
    }

    pub fn cancel(&self) {
        if let Some(flag) = &self.cancel {
            flag.store(true, Ordering::Relaxed);
        }
    }

    pub fn start(&self) -> mpsc::Receiver<AgentEvent> {
        let (events, receiver) = mpsc::channel(128);
        let session = self.session.clone();
        let prompt = self.prompt.clone();
        let workspace_root = self.workspace_root.clone();
        let coordination_root = self.coordination_root.clone();
        let provider = self.provider.clone();
        let model = self.model.clone();
        let usage = Arc::clone(&self.usage);
        let score_before = self.score_before;
        let context_compaction = self.context_compaction.clone();
        let verify_on_complete = self.verify_on_complete;
        tokio::spawn(async move {
            match session.run_turn(prompt.clone(), events.clone()).await {
                Ok(result) => {
                    if let Err(error) = usage
                        .record_turn(&coordination_root, &result)
                        .await
                        .and_then(|_| {
                            save_turn_capsule(
                                &coordination_root,
                                &prompt,
                                result.session_id,
                                &result.provider,
                                &result.model,
                                TurnCapsuleOutcome {
                                    status: "completed",
                                    blockers: vec![],
                                    score_before,
                                    context_compaction: context_compaction.clone(),
                                },
                            )
                        })
                    {
                        let _ = save_turn_capsule(
                            &coordination_root,
                            &prompt,
                            result.session_id,
                            &result.provider,
                            &result.model,
                            TurnCapsuleOutcome {
                                status: "failed",
                                blockers: vec![error.to_string()],
                                score_before,
                                context_compaction: context_compaction.clone(),
                            },
                        );
                        let _ = events
                            .send(AgentEvent::Failed {
                                error: format!("turn completed but persistence failed: {error}"),
                            })
                            .await;
                        return;
                    }

                    if let Some(gate) = verify_on_complete {
                        let results = VerifyRunner::with_root(&workspace_root)
                            .run_through(gate)
                            .await;
                        let passed = results.iter().all(|result| {
                            result.passed || result.status == VerifyStatus::Unavailable
                        });
                        let summary = results
                            .iter()
                            .map(|result| {
                                format!(
                                    "{}:{}",
                                    result.gate,
                                    if result.passed {
                                        "pass"
                                    } else if result.status == VerifyStatus::Unavailable {
                                        "skip"
                                    } else {
                                        "fail"
                                    }
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(" ");
                        let _ = events
                            .send(AgentEvent::VerifyComplete {
                                gate: gate.id().into(),
                                passed,
                                summary: summary.clone(),
                            })
                            .await;
                        if !passed {
                            let _ = events
                                .send(AgentEvent::Failed {
                                    error: format!(
                                        "verify-on-complete failed through {}: {summary}",
                                        gate.id()
                                    ),
                                })
                                .await;
                            return;
                        }
                    }

                    let _ = events.send(AgentEvent::Completed { result }).await;
                }
                Err(error) => {
                    let turn_status = if matches!(error, AdeError::Cancelled(_)) {
                        "cancelled"
                    } else {
                        "failed"
                    };
                    let _ = save_turn_capsule(
                        &coordination_root,
                        &prompt,
                        session.session_id(),
                        &provider,
                        &model,
                        TurnCapsuleOutcome {
                            status: turn_status,
                            blockers: vec![error.to_string()],
                            score_before,
                            context_compaction,
                        },
                    );
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
}

#[derive(Debug, Clone, Copy)]
struct WorkspaceScore {
    score: u32,
    score_max: u32,
}

struct TurnCapsuleOutcome<'a> {
    status: &'a str,
    blockers: Vec<String>,
    score_before: WorkspaceScore,
    context_compaction: HandoffContextCompaction,
}

fn save_turn_capsule(
    workspace_root: &Path,
    prompt: &str,
    session_id: Uuid,
    provider: &str,
    model: &str,
    outcome: TurnCapsuleOutcome<'_>,
) -> Result<(), AdeError> {
    let goal = prompt.chars().take(240).collect::<String>();
    let mut capsule = HandoffCapsule::from_agent_turn(
        goal,
        session_id,
        provider,
        model,
        outcome.status,
        outcome.blockers,
    );
    capsule.score_before = Some(outcome.score_before.score);
    capsule.score_max = Some(outcome.score_before.score_max);
    capsule.context_compaction = Some(outcome.context_compaction);
    capsule.compact_summary = Some(capsule.prompt_summary(480));
    HandoffManager::new(workspace_root)
        .save_capsule(&capsule)
        .map(|_| ())
}

fn workspace_score(workspace_root: &Path) -> WorkspaceScore {
    let report = AuditRunner::new(workspace_root).run(AuditMode::EvaluateExisting);
    WorkspaceScore {
        score: report.score,
        score_max: report.score_max,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{
        ProviderCompletion, ProviderRequest, ProviderStreamEvent, ProviderUsage,
    };
    use ade_db::secrets::{InMemoryProviderKeyVault, ProviderKeyVault};

    struct SmokeProvider;

    #[async_trait]
    impl ChatProvider for SmokeProvider {
        fn name(&self) -> &str {
            "test-provider"
        }

        async fn stream(
            &self,
            _request: ProviderRequest,
            events: mpsc::Sender<ProviderStreamEvent>,
        ) -> Result<ProviderCompletion, AdeError> {
            let _ = events
                .send(ProviderStreamEvent::TextDelta {
                    text: "ADE_SMOKE_OK".into(),
                })
                .await;
            Ok(ProviderCompletion {
                text: "ADE_SMOKE_OK".into(),
                tool_calls: vec![],
                usage: ProviderUsage {
                    input_tokens: 10,
                    output_tokens: 2,
                },
            })
        }
    }

    #[test]
    fn cli_and_desktop_specs_share_policy_fingerprint() {
        let caps = SpendCaps {
            session: Money::from_usd_str("1.0").unwrap(),
            daily: Money::from_usd_str("10.0").unwrap(),
        };
        let mut cli = AgentTurnSpec {
            prompt: "hi".into(),
            provider: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-4.1-mini".into(),
            input_cost_per_mtok: Money::ZERO,
            output_cost_per_mtok: Money::ZERO,
            context_limit: 128_000,
            output_limit: 16_384,
            profile: "local".into(),
            workspace_root: PathBuf::from("."),
            owned_paths: vec![],
            handoff_chars: DEFAULT_HANDOFF_CHARS,
        };
        let desktop = cli.clone();
        assert_eq!(
            AgentTurnBuilder::policy_fingerprint(&cli, &caps),
            AgentTurnBuilder::policy_fingerprint(&desktop, &caps)
        );
        cli.profile = "staging".into();
        assert_ne!(
            AgentTurnBuilder::policy_fingerprint(&cli, &caps),
            AgentTurnBuilder::policy_fingerprint(&desktop, &caps)
        );
    }

    #[tokio::test]
    async fn completed_event_waits_for_ledger_and_redacted_handoff() {
        let root = std::env::temp_dir().join(format!("ade-turn-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("AGENTS.md"), "# Test agent contract\n").unwrap();
        let lease_agent = Uuid::new_v4();
        LeaseManager::new(&root)
            .acquire(
                lease_agent,
                "src/feature",
                ade_workflow::parallel::LeaseMode::Strong,
                chrono::Duration::minutes(5),
            )
            .unwrap();
        let database = AdeDatabase::open_path(":memory:").await.unwrap();
        let ledger = UsageLedgerStore::new(database.connect().unwrap());
        let vault = InMemoryProviderKeyVault::default();
        vault
            .set("local", "test-provider", "not-a-real-secret")
            .unwrap();

        let service = AgentTurnBuilder::new(AgentTurnSpec {
            prompt: "Reply with exactly ADE_SMOKE_OK".into(),
            provider: "test-provider".into(),
            base_url: "https://example.invalid/v1".into(),
            model: "test-model".into(),
            input_cost_per_mtok: Money::ZERO,
            output_cost_per_mtok: Money::ZERO,
            context_limit: 8_192,
            output_limit: 16,
            profile: "local".into(),
            workspace_root: root.clone(),
            owned_paths: vec![],
            handoff_chars: DEFAULT_HANDOFF_CHARS,
        })
        .ledger(ledger.clone())
        .lease_agent(lease_agent)
        .key_vault(Arc::new(vault))
        .provider_transport(Arc::new(SmokeProvider))
        .prepare()
        .await
        .unwrap();
        assert_eq!(
            service.effective_owned_paths(),
            &["src/feature".to_string()]
        );

        let mut events = service.start();
        let result = loop {
            match events.recv().await.unwrap() {
                AgentEvent::Completed { result } => break result,
                AgentEvent::Failed { error } => panic!("{error}"),
                _ => {}
            }
        };

        assert!(ledger
            .has_committed_entry(
                result.session_id,
                "turn_summary",
                &root.display().to_string()
            )
            .await
            .unwrap());
        let capsule = HandoffManager::new(&root).load_latest().unwrap();
        assert_eq!(
            capsule.session_id.as_deref(),
            Some(result.session_id.to_string().as_str())
        );
        assert_eq!(
            capsule.next_safe_command.as_deref(),
            Some("ade verify --gate G0 --through")
        );
        assert!(capsule.score_before.is_some());
        assert!(capsule.score_after.is_none());
        assert!(capsule.score_max.is_some_and(|value| value > 0));
        assert!(capsule.context_compaction.is_some());
        assert!(!serde_json::to_string(&capsule)
            .unwrap()
            .contains("not-a-real-secret"));
        let _ = std::fs::remove_dir_all(root);
    }
}
