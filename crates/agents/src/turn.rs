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
const DEFAULT_MAX_TOOL_ROUNDS: usize = 32;

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
    /// Default shell cwd when the model omits `cwd` (e.g. `~/Desktop` for Home scope).
    preferred_shell_cwd: Option<String>,
    /// G2: human-confirmed risk categories / tiers for this turn.
    approved_risk_categories: Vec<String>,
    approved_risk_tiers: Vec<String>,
    /// H2: claimed task id for heartbeat / claim_gate satisfaction.
    claimed_task_id: Option<String>,
    /// H2: allow free-form Act while ready queue is non-empty (audited).
    waive_queue: bool,
    /// H2: force Verifier (or other) slot instead of autonomy mapping.
    slot_override: Option<crate::slots::SlotRole>,
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
            preferred_shell_cwd: None,
            approved_risk_categories: Vec::new(),
            approved_risk_tiers: Vec::new(),
            claimed_task_id: None,
            waive_queue: false,
            slot_override: None,
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

    /// Host-selected default for `shell__run_command` when the model omits `cwd`.
    pub fn preferred_shell_cwd(mut self, cwd: Option<impl Into<String>>) -> Self {
        self.preferred_shell_cwd = cwd.map(Into::into).filter(|s| !s.trim().is_empty());
        self
    }

    /// G2: approve high-risk categories (publish/infra/migrate/secrets) for this turn.
    pub fn approved_risk_categories(mut self, categories: Vec<String>) -> Self {
        self.approved_risk_categories = categories;
        self
    }

    pub fn approved_risk_tiers(mut self, tiers: Vec<String>) -> Self {
        self.approved_risk_tiers = tiers;
        self
    }

    pub fn claimed_task_id(mut self, task_id: Option<impl Into<String>>) -> Self {
        self.claimed_task_id = task_id
            .map(Into::into)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        self
    }

    pub fn waive_queue(mut self, waive: bool) -> Self {
        self.waive_queue = waive;
        self
    }

    pub fn slot_override(mut self, slot: Option<crate::slots::SlotRole>) -> Self {
        self.slot_override = slot;
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
        let mut autonomy = self.autonomy;
        if matches!(self.slot_override, Some(crate::slots::SlotRole::Verifier)) {
            // Verifier is sensors-first: never mutate under this override.
            if autonomy.allows_mutating_tools() {
                autonomy = AutonomyLevel::Propose;
            }
        }

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

        // H2 claim_gate: Act/Automate must claim (or waive) when ready work is queued.
        enforce_claim_gate(
            &coordination_root,
            autonomy,
            self.lease_agent_id,
            self.claimed_task_id.as_deref(),
            self.waive_queue,
        )?;

        let owned_paths = if matches!(self.slot_override, Some(crate::slots::SlotRole::Verifier)) {
            Vec::new()
        } else {
            match self.lease_agent_id {
                Some(agent_id) => LeaseManager::new(&coordination_root)
                    .resolve_owned_paths(agent_id, &self.spec.owned_paths)?,
                None => self.spec.owned_paths.clone(),
            }
        };
        let authority = AuthorityEnforcer::load(&self.spec.workspace_root, owned_paths)?;
        let effective_owned_paths = authority.owned_paths();
        // PLAN / eng-goal live on the coordination root (primary checkout), even
        // when tools execute inside an isolated worktree (G4).
        ade_workflow::plan_enforcement::PlanEnforcer::new().ensure_approved_plan(
            &coordination_root,
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
        let scope_clause = match self.preferred_shell_cwd.as_deref() {
            Some(cwd) => format!(
                "SHELL SCOPE=Home/Desktop: shell__run_command defaults to cwd `{cwd}` when omitted. Prefer this for goals about that folder. fs__* tools remain workspace-scoped."
            ),
            None => "SHELL SCOPE=Workspace: shell__run_command defaults to the attached workspace root. Set cwd to ~/Desktop (or $env:USERPROFILE\\\\Desktop) for Desktop/home goals.".into(),
        };
        let eng_goal = crate::goal::GoalStore::new(&coordination_root)
            .load_active()
            .ok()
            .flatten()
            .filter(|g| g.status == "active");
        let (contract_allows_act, contract_block_detail) = if autonomy.allows_mutating_tools() {
            match &eng_goal {
                    Some(goal) if goal.allows_act_tools() => (true, None),
                    Some(goal) => (false, Some(goal.contract_block_detail())),
                    None => (
                        false,
                        Some(format!(
                            "{} Act tools blocked until an active eng-goal has acceptance criteria, out-of-scope, and verify pointer (or ≤3 clarify / logged waive). Define a goal or switch to Suggest.",
                            crate::goal::CONTRACT_GATE_PREFIX
                        )),
                    ),
                }
        } else {
            (true, None)
        };
        let eng_goal_clause = eng_goal.as_ref().map(|g| g.prompt_block());
        let contract_clause = if autonomy.allows_mutating_tools() && !contract_allows_act {
            Some(
                "CONTRACT GATE: Act/Automate dial is on but eng-goal contract is incomplete. Read/inspect only this turn — ask the user to fill acceptance criteria, out-of-scope, and verify (or waive). Do not attempt writes."
                    .to_string(),
            )
        } else {
            None
        };
        let isolation_clause = if coordination_root != self.spec.workspace_root {
            format!(
                "\n\nISOLATION=worktree: tools execute in `{}`; leases/PLAN/goals stay on the primary checkout.",
                self.spec.workspace_root.display()
            )
        } else {
            String::new()
        };
        let assembled = PromptAssembler::daily(self.spec.context_limit).assemble(
            &format!(
                "{}\n\n{}\n\n{}{}{}{}",
                StartPromptBuilder::new().build(),
                autonomy.prompt_clause(),
                scope_clause,
                eng_goal_clause
                    .as_deref()
                    .map(|block| format!("\n\n{block}"))
                    .unwrap_or_default(),
                contract_clause
                    .as_deref()
                    .map(|block| format!("\n\n{block}"))
                    .unwrap_or_default(),
                isolation_clause
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
        let session_cap = caps.session;
        let mut guard = SpendGuard::new(&coordination_root, session_id, caps, ledger.clone());
        if let Some(actor) = self.actor {
            guard = guard.with_actor(actor);
        }

        let verify_on_complete = if autonomy.requires_verify_on_complete() {
            Some(self.verify_on_complete.unwrap_or(VerifyGate::G3))
        } else {
            self.verify_on_complete
        };

        let catalog = crate::model_profile::ModelProfileCatalog::load(&coordination_root);
        let _ = crate::model_profile::ensure_default_profiles(&coordination_root);
        let route = crate::model_profile::route(
            &catalog,
            &crate::model_profile::RouteInput {
                provider: self.spec.provider.clone(),
                model: self.spec.model.clone(),
                autonomy,
                max_tool_rounds: self.max_tool_rounds,
                session_cap: Some(session_cap),
                slot_override: self.slot_override,
            },
        );
        let max_tool_rounds = route.effective_max_tool_rounds(self.max_tool_rounds);
        let verify_on_complete = if route.require_verify && verify_on_complete.is_none() {
            Some(VerifyGate::G3)
        } else {
            verify_on_complete
        };

        let mut session = AgentSession::new(
            transport,
            model,
            self.mcp.unwrap_or_default(),
            system_prompt,
        )
        .with_authority(authority)
        .with_workspace(&self.spec.workspace_root)
        .with_preferred_shell_cwd(self.preferred_shell_cwd.clone())
        .with_spend_guard(guard)
        .with_request_timeout(self.request_timeout)
        .with_autonomy(autonomy)
        .with_contract_gate(contract_allows_act, contract_block_detail)
        .with_route(
            route.profile_id.clone(),
            route.reason.clone(),
            route.slot.as_str(),
            route.tool_effect_deny.clone(),
        )
        .with_risk_approvals(
            self.approved_risk_categories.clone(),
            self.approved_risk_tiers.clone(),
        )
        .with_max_tool_rounds(max_tool_rounds)
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

/// H2: Act/Automate must hold a claim (or waive) when ready tasks are queued.
pub fn enforce_claim_gate(
    root: &Path,
    autonomy: AutonomyLevel,
    lease_agent_id: Option<Uuid>,
    claimed_task_id: Option<&str>,
    waive_queue: bool,
) -> Result<(), AdeError> {
    if !autonomy.allows_mutating_tools() {
        return Ok(());
    }
    let coordinator = ade_workflow::tasks::TaskCoordinator::new(root);
    let ready = coordinator.ready_queued_count()?;
    if ready == 0 {
        return Ok(());
    }
    let satisfied = match (claimed_task_id, lease_agent_id) {
        (Some(task_id), Some(agent_id)) => coordinator.agent_holds_task(task_id, agent_id)?,
        (None, Some(agent_id)) => coordinator.agent_has_active_claim(agent_id)?,
        _ => false,
    };
    if satisfied {
        return Ok(());
    }
    if waive_queue {
        coordinator.log_queue_waive(
            lease_agent_id,
            ready,
            "free-form Apply while queue non-empty",
        )?;
        return Ok(());
    }
    Err(AdeError::Authorization(format!(
        "claim_gate: {ready} queued task(s) — Apply next or waive"
    )))
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
                    } else if matches!(error, AdeError::Budget(_)) {
                        "budget_exhausted"
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
    let id = HandoffManager::new(workspace_root).save_capsule(&capsule)?;
    // C3 write-before-compact: durable facts on disk before summary is the only memory.
    let _ = crate::handoff::write_continuity_last_write(workspace_root, &capsule, &id);
    if let Ok(Some(active)) = crate::goal::GoalStore::new(workspace_root).load_active() {
        let _ = crate::goal::GoalStore::new(workspace_root).attach_handoff(&active.id, &id);
    }
    Ok(())
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
