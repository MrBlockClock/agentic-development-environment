//! Explicit AgentTurn workers that claim lease-backed tasks and execute them.
//!
//! Workers are started by an operator (`ade worker run --approve`), not by the
//! daemon. They reuse the same provider/model/spend surface as `ade agent`.

use ade_agents::session::AgentEvent;
use ade_agents::spend::SpendCaps;
use ade_agents::turn::{AgentTurnBuilder, AgentTurnSpec};
use ade_core::error::AdeError;
use ade_core::money::Money;
use ade_db::repo::{AdeDatabase, DbConfig};
use ade_db::usage_ledger::UsageLedgerStore;
use ade_workflow::parallel::WorktreeManager;
use ade_workflow::tasks::TaskCoordinator;
use chrono::Duration as ChronoDuration;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    pub workspace_root: PathBuf,
    pub agent_id: Uuid,
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub input_cost_per_mtok: Money,
    pub output_cost_per_mtok: Money,
    pub context_limit: u64,
    pub output_limit: u64,
    pub profile: String,
    pub ttl_secs: i64,
    pub poll_interval: Duration,
    pub provision_worktree: bool,
    pub cleanup_worktree: bool,
    /// When true, claim+execute at most one task then return (dogfood / CI).
    pub once: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkerTick {
    pub claimed: bool,
    pub task_id: Option<String>,
    pub status: String,
    pub detail: Option<String>,
}

pub struct AgentTurnWorker {
    config: WorkerConfig,
}

impl AgentTurnWorker {
    pub fn new(config: WorkerConfig) -> Self {
        Self { config }
    }

    pub async fn run(&self) -> Result<(), AdeError> {
        loop {
            let tick = self.run_once().await?;
            if self.config.once {
                if !tick.claimed {
                    return Err(AdeError::Other(
                        "worker --once: no dependency-ready task to claim".into(),
                    ));
                }
                if tick.status == "failed" {
                    return Err(AdeError::Other(
                        tick.detail
                            .unwrap_or_else(|| "worker --once task failed".into()),
                    ));
                }
                return Ok(());
            }
            if !tick.claimed {
                tokio::time::sleep(self.config.poll_interval).await;
            }
        }
    }

    pub async fn run_once(&self) -> Result<WorkerTick, AdeError> {
        let coordinator = TaskCoordinator::new(&self.config.workspace_root);
        let _ = coordinator.requeue_expired()?;
        let ttl = ChronoDuration::seconds(self.config.ttl_secs.max(1));
        let Some(task) = coordinator.claim(self.config.agent_id, ttl)? else {
            return Ok(WorkerTick {
                claimed: false,
                task_id: None,
                status: "idle".into(),
                detail: None,
            });
        };

        coordinator.start(&task.id, self.config.agent_id)?;
        let worktree = if self.config.provision_worktree {
            Some(self.provision_worktree(&task.id)?)
        } else {
            None
        };
        let execution_root = worktree
            .as_ref()
            .cloned()
            .unwrap_or_else(|| self.config.workspace_root.clone());

        let heartbeat = {
            let root = self.config.workspace_root.clone();
            let task_id = task.id.clone();
            let agent_id = self.config.agent_id;
            let ttl_secs = self.config.ttl_secs.max(1);
            let interval = Duration::from_secs((ttl_secs as u64 / 3).max(1));
            tokio::spawn(async move {
                let coordinator = TaskCoordinator::new(root);
                loop {
                    tokio::time::sleep(interval).await;
                    if coordinator
                        .heartbeat(&task_id, agent_id, ChronoDuration::seconds(ttl_secs))
                        .is_err()
                    {
                        break;
                    }
                }
            })
        };

        let outcome = self
            .execute_task(&task.goal, &task.owned_paths, &execution_root)
            .await;
        heartbeat.abort();

        let tick = match outcome {
            Ok(detail) => {
                coordinator.complete(&task.id, self.config.agent_id)?;
                if self.config.cleanup_worktree {
                    if let Some(path) = &worktree {
                        let _ =
                            WorktreeManager::new(&self.config.workspace_root).remove(path, true);
                    }
                }
                let worktree_note = worktree
                    .as_ref()
                    .map(|path| format!(" · worktree {}", path.display()))
                    .unwrap_or_default();
                WorkerTick {
                    claimed: true,
                    task_id: Some(task.id),
                    status: "completed".into(),
                    detail: Some(format!("{detail}{worktree_note}")),
                }
            }
            Err(error) => {
                let _ = coordinator.fail(&task.id, self.config.agent_id, error.to_string());
                WorkerTick {
                    claimed: true,
                    task_id: Some(task.id),
                    status: "failed".into(),
                    detail: Some(error.to_string()),
                }
            }
        };
        Ok(tick)
    }

    async fn execute_task(
        &self,
        goal: &str,
        owned_paths: &[String],
        execution_root: &Path,
    ) -> Result<String, AdeError> {
        let config = ade_core::config::AdeConfig::load()?;
        let database = AdeDatabase::open(&DbConfig::from_ade_config(&config)).await?;
        let ledger = UsageLedgerStore::new(database.connect()?);
        let mut builder = AgentTurnBuilder::new(AgentTurnSpec {
            prompt: goal.to_string(),
            provider: self.config.provider.clone(),
            base_url: self.config.base_url.clone(),
            model: self.config.model.clone(),
            input_cost_per_mtok: self.config.input_cost_per_mtok,
            output_cost_per_mtok: self.config.output_cost_per_mtok,
            context_limit: self.config.context_limit,
            output_limit: self.config.output_limit,
            profile: self.config.profile.clone(),
            workspace_root: execution_root.to_path_buf(),
            owned_paths: owned_paths.to_vec(),
            handoff_chars: 1_500,
        })
        .ledger(ledger)
        .spend_caps(SpendCaps::from_env())
        .lease_agent(self.config.agent_id)
        .actor(format!("worker:{}", self.config.agent_id))
        .autonomy(ade_agents::autonomy::AutonomyLevel::Act);
        if execution_root != self.config.workspace_root {
            builder = builder.coordination_root(self.config.workspace_root.clone());
        }
        let service = builder.prepare().await?;
        let mut events = service.start();
        let mut failure = None;
        let mut completed = None;
        while let Some(event) = events.recv().await {
            match event {
                AgentEvent::Completed { result } => completed = Some(result),
                AgentEvent::Failed { error } | AgentEvent::Cancelled { reason: error } => {
                    failure = Some(error);
                }
                _ => {}
            }
        }
        if let Some(error) = failure {
            return Err(AdeError::Other(error));
        }
        let result = completed.ok_or_else(|| {
            AdeError::Other("worker turn ended without a completion event".into())
        })?;
        Ok(format!(
            "{} / {} · {} in + {} out · ${}",
            result.provider,
            result.model,
            result.usage.input_tokens,
            result.usage.output_tokens,
            Money::from_micros(result.cost_micros).format_usd()
        ))
    }

    fn provision_worktree(&self, task_id: &str) -> Result<PathBuf, AdeError> {
        let short = task_id.chars().take(8).collect::<String>();
        let path = self
            .config
            .workspace_root
            .join(".ade")
            .join("worktrees")
            .join(task_id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let branch = format!("ade/task-{short}");
        let info = WorktreeManager::new(&self.config.workspace_root).add(&path, &branch, None)?;
        Ok(PathBuf::from(info.path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ade_workflow::parallel::LeaseMode;
    use ade_workflow::tasks::EnqueueTask;

    #[tokio::test]
    async fn idle_when_queue_is_empty() {
        let root = std::env::temp_dir().join(format!("ade-worker-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let worker = AgentTurnWorker::new(WorkerConfig {
            workspace_root: root.clone(),
            agent_id: Uuid::new_v4(),
            provider: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-4.1-mini".into(),
            input_cost_per_mtok: Money::ZERO,
            output_cost_per_mtok: Money::ZERO,
            context_limit: 8_192,
            output_limit: 16,
            profile: "local".into(),
            ttl_secs: 60,
            poll_interval: Duration::from_millis(10),
            provision_worktree: false,
            cleanup_worktree: false,
            once: false,
        });
        let tick = worker.run_once().await.unwrap();
        assert!(!tick.claimed);
        assert_eq!(tick.status, "idle");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn claim_start_complete_without_provider() {
        let root = std::env::temp_dir().join(format!("ade-worker-claim-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let coordinator = TaskCoordinator::new(&root);
        let task = coordinator
            .enqueue(EnqueueTask {
                goal: "noop".into(),
                owned_paths: vec!["src/worker".into()],
                lease_mode: LeaseMode::Strong,
                depends_on: vec![],
            })
            .unwrap();
        let agent = Uuid::new_v4();
        let claimed = coordinator
            .claim(agent, ChronoDuration::minutes(5))
            .unwrap()
            .unwrap();
        assert_eq!(claimed.id, task.id);
        coordinator.start(&task.id, agent).unwrap();
        coordinator.complete(&task.id, agent).unwrap();
        let _ = std::fs::remove_dir_all(root);
    }
}
