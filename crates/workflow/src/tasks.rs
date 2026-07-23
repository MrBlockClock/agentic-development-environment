//! Durable, workspace-local coordination for multi-agent tasks.
//!
//! A task is not runnable until its dependencies are complete and its claimed
//! agent holds leases for every declared owned path. The registry is protected
//! by a cross-process file lock so independent ADE processes cannot claim the
//! same task.

use crate::parallel::{LeaseManager, LeaseMode};
use ade_core::error::AdeError;
use chrono::{DateTime, Duration, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const TASK_REGISTRY_SCHEMA: &str = "ade.task.registry/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Claimed,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub id: String,
    pub goal: String,
    pub owned_paths: Vec<String>,
    pub lease_mode: LeaseMode,
    pub depends_on: Vec<String>,
    pub status: TaskStatus,
    pub agent_id: Option<Uuid>,
    pub lease_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub heartbeat_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub failure: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EnqueueTask {
    pub goal: String,
    pub owned_paths: Vec<String>,
    pub lease_mode: LeaseMode,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TaskRegistry {
    schema: String,
    tasks: Vec<AgentTask>,
}

pub struct TaskCoordinator {
    root: PathBuf,
}

impl TaskCoordinator {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn enqueue(&self, input: EnqueueTask) -> Result<AgentTask, AdeError> {
        let goal = input.goal.trim();
        if goal.is_empty() {
            return Err(AdeError::PlanValidation(
                "task goal must not be empty".into(),
            ));
        }
        if matches!(input.lease_mode, LeaseMode::Observe) && !input.owned_paths.is_empty() {
            return Err(AdeError::PlanValidation(
                "observe tasks cannot declare writable owned_paths".into(),
            ));
        }
        let mut owned_paths = input.owned_paths;
        owned_paths.sort();
        owned_paths.dedup();
        let mut depends_on = input.depends_on;
        depends_on.sort();
        depends_on.dedup();

        self.with_registry_mut(|registry| {
            for dependency in &depends_on {
                if !registry.tasks.iter().any(|task| task.id == *dependency) {
                    return Err(AdeError::NotFound(format!(
                        "task dependency '{dependency}' does not exist"
                    )));
                }
            }
            let task = AgentTask {
                id: Uuid::new_v4().to_string(),
                goal: goal.to_string(),
                owned_paths,
                lease_mode: input.lease_mode,
                depends_on,
                status: TaskStatus::Queued,
                agent_id: None,
                lease_ids: vec![],
                created_at: Utc::now(),
                claimed_at: None,
                heartbeat_at: None,
                expires_at: None,
                finished_at: None,
                failure: None,
            };
            registry.tasks.push(task.clone());
            Ok(task)
        })
    }

    pub fn list(&self) -> Result<Vec<AgentTask>, AdeError> {
        self.with_registry(|registry| Ok(registry.tasks.clone()))
    }

    /// Count tasks that are queued and whose dependencies are complete.
    pub fn ready_queued_count(&self) -> Result<usize, AdeError> {
        self.with_registry_mut(|registry| {
            self.requeue_expired_locked(registry)?;
            Ok(self.ready_queued_indices(registry).len())
        })
    }

    /// True when this agent holds a Claimed or Running task.
    pub fn agent_has_active_claim(&self, agent_id: Uuid) -> Result<bool, AdeError> {
        self.with_registry_mut(|registry| {
            self.requeue_expired_locked(registry)?;
            Ok(registry.tasks.iter().any(|task| {
                task.agent_id == Some(agent_id)
                    && matches!(task.status, TaskStatus::Claimed | TaskStatus::Running)
            }))
        })
    }

    /// True when `task_id` is Claimed/Running and held by `agent_id`.
    pub fn agent_holds_task(&self, task_id: &str, agent_id: Uuid) -> Result<bool, AdeError> {
        self.with_registry_mut(|registry| {
            self.requeue_expired_locked(registry)?;
            Ok(registry.tasks.iter().any(|task| {
                task.id == task_id
                    && task.agent_id == Some(agent_id)
                    && matches!(task.status, TaskStatus::Claimed | TaskStatus::Running)
            }))
        })
    }

    /// Append an auditable queue-waive line (free-form Apply while tasks are ready).
    pub fn log_queue_waive(
        &self,
        agent_id: Option<Uuid>,
        ready_count: usize,
        reason: &str,
    ) -> Result<(), AdeError> {
        let dir = self.root.join(".ade").join("tasks");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("queue-waives.jsonl");
        let line = serde_json::json!({
            "schema": "ade.task.queue-waive/v1",
            "at": Utc::now().to_rfc3339(),
            "agent_id": agent_id.map(|id| id.to_string()),
            "ready_count": ready_count,
            "reason": reason.trim(),
        });
        use std::io::Write;
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        writeln!(file, "{line}")?;
        Ok(())
    }

    fn ready_queued_indices(&self, registry: &TaskRegistry) -> Vec<usize> {
        let completed = registry
            .tasks
            .iter()
            .filter(|task| task.status == TaskStatus::Completed)
            .map(|task| task.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        registry
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| {
                task.status == TaskStatus::Queued
                    && task
                        .depends_on
                        .iter()
                        .all(|dependency| completed.contains(dependency.as_str()))
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// Claim the oldest ready task for `agent_id`.
    ///
    /// The task-registry lock remains held while leases are acquired. If any
    /// lease fails, already-acquired leases are rolled back and the task stays
    /// queued.
    pub fn claim(&self, agent_id: Uuid, ttl: Duration) -> Result<Option<AgentTask>, AdeError> {
        validate_ttl(ttl)?;
        self.with_registry_mut(|registry| {
            self.requeue_expired_locked(registry)?;
            let Some(index) = self.ready_queued_indices(registry).into_iter().next() else {
                return Ok(None);
            };

            let task = &registry.tasks[index];
            let lease_manager = LeaseManager::new(&self.root);
            let mut lease_ids = Vec::new();
            for path in &task.owned_paths {
                match lease_manager.acquire(agent_id, path, task.lease_mode, ttl) {
                    Ok(lease) => lease_ids.push(lease.id),
                    Err(error) => {
                        for lease_id in &lease_ids {
                            let _ = lease_manager.release(lease_id);
                        }
                        return Err(error);
                    }
                }
            }

            let now = Utc::now();
            let task = &mut registry.tasks[index];
            task.status = TaskStatus::Claimed;
            task.agent_id = Some(agent_id);
            task.lease_ids = lease_ids;
            task.claimed_at = Some(now);
            task.heartbeat_at = Some(now);
            task.expires_at = Some(now + ttl);
            Ok(Some(task.clone()))
        })
    }

    /// Claim a specific queued task (G3 Apply-one).
    pub fn claim_id(
        &self,
        task_id: &str,
        agent_id: Uuid,
        ttl: Duration,
    ) -> Result<AgentTask, AdeError> {
        validate_ttl(ttl)?;
        self.with_registry_mut(|registry| {
            self.requeue_expired_locked(registry)?;
            let completed = registry
                .tasks
                .iter()
                .filter(|task| task.status == TaskStatus::Completed)
                .map(|task| task.id.as_str())
                .collect::<std::collections::HashSet<_>>();
            let index = task_index(registry, task_id)?;
            let task = &registry.tasks[index];
            if task.status != TaskStatus::Queued {
                return Err(AdeError::Other(format!(
                    "task '{task_id}' is not queued (status={:?})",
                    task.status
                )));
            }
            if !task
                .depends_on
                .iter()
                .all(|dependency| completed.contains(dependency.as_str()))
            {
                return Err(AdeError::Other(format!(
                    "task '{task_id}' has incomplete dependencies"
                )));
            }

            let owned_paths = task.owned_paths.clone();
            let lease_mode = task.lease_mode;
            let lease_manager = LeaseManager::new(&self.root);
            let mut lease_ids = Vec::new();
            for path in &owned_paths {
                match lease_manager.acquire(agent_id, path, lease_mode, ttl) {
                    Ok(lease) => lease_ids.push(lease.id),
                    Err(error) => {
                        for lease_id in &lease_ids {
                            let _ = lease_manager.release(lease_id);
                        }
                        return Err(error);
                    }
                }
            }

            let now = Utc::now();
            let task = &mut registry.tasks[index];
            task.status = TaskStatus::Claimed;
            task.agent_id = Some(agent_id);
            task.lease_ids = lease_ids;
            task.claimed_at = Some(now);
            task.heartbeat_at = Some(now);
            task.expires_at = Some(now + ttl);
            Ok(task.clone())
        })
    }

    /// Enqueue PLAN phases as tasks (idempotent by `[phase.id]` goal prefix).
    /// Returns newly queued tasks only.
    pub fn sync_from_plan(
        &self,
        phases: &[ade_core::plan::PlanPhase],
    ) -> Result<Vec<AgentTask>, AdeError> {
        if phases.is_empty() {
            return Ok(Vec::new());
        }
        self.with_registry_mut(|registry| {
            let existing_keys: std::collections::HashSet<String> = registry
                .tasks
                .iter()
                .filter(|task| !task.status.is_terminal())
                .filter_map(|task| plan_phase_key_from_goal(&task.goal))
                .collect();

            let mut phase_to_task: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            for task in &registry.tasks {
                if let Some(key) = plan_phase_key_from_goal(&task.goal) {
                    phase_to_task.insert(key, task.id.clone());
                }
            }

            let mut created_ids = Vec::new();
            for phase in phases {
                let key = phase.id.trim();
                if key.is_empty() || existing_keys.contains(key) {
                    continue;
                }
                let mut owned_paths = phase.owned_paths.clone();
                owned_paths.sort();
                owned_paths.dedup();
                if owned_paths.is_empty() {
                    continue;
                }
                let goal = format!("[{}] {}", phase.id, phase.title.trim());
                let task = AgentTask {
                    id: Uuid::new_v4().to_string(),
                    goal,
                    owned_paths,
                    lease_mode: LeaseMode::Strong,
                    depends_on: vec![],
                    status: TaskStatus::Queued,
                    agent_id: None,
                    lease_ids: vec![],
                    created_at: Utc::now(),
                    claimed_at: None,
                    heartbeat_at: None,
                    expires_at: None,
                    finished_at: None,
                    failure: None,
                };
                phase_to_task.insert(key.to_string(), task.id.clone());
                created_ids.push(task.id.clone());
                registry.tasks.push(task);
            }

            // Second pass: wire phase.depends_on → task ids (including newly created).
            for phase in phases {
                let Some(task_id) = phase_to_task.get(phase.id.trim()) else {
                    continue;
                };
                let depends_on = phase
                    .depends_on
                    .iter()
                    .filter_map(|dep| phase_to_task.get(dep.trim()).cloned())
                    .collect::<Vec<_>>();
                if depends_on.is_empty() {
                    continue;
                }
                if let Some(task) = registry.tasks.iter_mut().find(|t| t.id == *task_id) {
                    if !task.status.is_terminal() {
                        task.depends_on = depends_on;
                    }
                }
            }

            let created = registry
                .tasks
                .iter()
                .filter(|task| created_ids.iter().any(|id| id == &task.id))
                .cloned()
                .collect();
            Ok(created)
        })
    }

    pub fn start(&self, task_id: &str, agent_id: Uuid) -> Result<AgentTask, AdeError> {
        self.transition_owned(task_id, agent_id, |task| {
            if task.status != TaskStatus::Claimed {
                return Err(AdeError::Other(format!(
                    "task '{}' must be claimed before it can start",
                    task.id
                )));
            }
            task.status = TaskStatus::Running;
            Ok(())
        })
    }

    pub fn heartbeat(
        &self,
        task_id: &str,
        agent_id: Uuid,
        ttl: Duration,
    ) -> Result<AgentTask, AdeError> {
        validate_ttl(ttl)?;
        self.with_registry_mut(|registry| {
            self.requeue_expired_locked(registry)?;
            let index = task_index(registry, task_id)?;
            assert_holder(&registry.tasks[index], agent_id)?;
            if !matches!(
                registry.tasks[index].status,
                TaskStatus::Claimed | TaskStatus::Running
            ) {
                return Err(AdeError::Other(format!("task '{task_id}' is not active")));
            }

            let lease_ids = registry.tasks[index].lease_ids.clone();
            let lease_manager = LeaseManager::new(&self.root);
            let active = lease_manager.list()?;
            for lease_id in &lease_ids {
                let lease = active
                    .iter()
                    .find(|lease| lease.id == *lease_id)
                    .ok_or_else(|| {
                        AdeError::Authorization(format!(
                            "task '{task_id}' lost lease '{lease_id}'; it must be reclaimed"
                        ))
                    })?;
                if lease.agent_id != agent_id {
                    return Err(AdeError::Authorization(format!(
                        "task lease '{lease_id}' is no longer held by agent {agent_id}"
                    )));
                }
            }
            for lease_id in &lease_ids {
                lease_manager.renew(agent_id, lease_id, ttl)?;
            }

            let now = Utc::now();
            let task = &mut registry.tasks[index];
            task.heartbeat_at = Some(now);
            task.expires_at = Some(now + ttl);
            Ok(task.clone())
        })
    }

    pub fn complete(&self, task_id: &str, agent_id: Uuid) -> Result<AgentTask, AdeError> {
        self.finish(task_id, agent_id, TaskStatus::Completed, None)
    }

    pub fn fail(
        &self,
        task_id: &str,
        agent_id: Uuid,
        failure: impl Into<String>,
    ) -> Result<AgentTask, AdeError> {
        let failure = failure.into();
        if failure.trim().is_empty() {
            return Err(AdeError::Other(
                "task failure reason must not be empty".into(),
            ));
        }
        self.finish(task_id, agent_id, TaskStatus::Failed, Some(failure))
    }

    pub fn cancel(&self, task_id: &str) -> Result<AgentTask, AdeError> {
        self.with_registry_mut(|registry| {
            let index = task_index(registry, task_id)?;
            let task = &registry.tasks[index];
            if task.status.is_terminal() {
                return Err(AdeError::Other(format!(
                    "task '{task_id}' is already terminal"
                )));
            }
            release_task_leases(&self.root, task);
            let task = &mut registry.tasks[index];
            task.status = TaskStatus::Cancelled;
            task.lease_ids.clear();
            task.expires_at = None;
            task.finished_at = Some(Utc::now());
            Ok(task.clone())
        })
    }

    /// Requeue stale claims and release any still-active leases.
    pub fn requeue_expired(&self) -> Result<usize, AdeError> {
        self.with_registry_mut(|registry| self.requeue_expired_locked(registry))
    }

    fn finish(
        &self,
        task_id: &str,
        agent_id: Uuid,
        status: TaskStatus,
        failure: Option<String>,
    ) -> Result<AgentTask, AdeError> {
        self.with_registry_mut(|registry| {
            self.requeue_expired_locked(registry)?;
            let index = task_index(registry, task_id)?;
            assert_holder(&registry.tasks[index], agent_id)?;
            if !matches!(
                registry.tasks[index].status,
                TaskStatus::Claimed | TaskStatus::Running
            ) {
                return Err(AdeError::Other(format!("task '{task_id}' is not active")));
            }
            release_task_leases(&self.root, &registry.tasks[index]);
            let task = &mut registry.tasks[index];
            task.status = status;
            task.lease_ids.clear();
            task.expires_at = None;
            task.finished_at = Some(Utc::now());
            task.failure = failure;
            Ok(task.clone())
        })
    }

    fn transition_owned(
        &self,
        task_id: &str,
        agent_id: Uuid,
        transition: impl FnOnce(&mut AgentTask) -> Result<(), AdeError>,
    ) -> Result<AgentTask, AdeError> {
        self.with_registry_mut(|registry| {
            self.requeue_expired_locked(registry)?;
            let index = task_index(registry, task_id)?;
            assert_holder(&registry.tasks[index], agent_id)?;
            transition(&mut registry.tasks[index])?;
            Ok(registry.tasks[index].clone())
        })
    }

    fn requeue_expired_locked(&self, registry: &mut TaskRegistry) -> Result<usize, AdeError> {
        let now = Utc::now();
        let stale = registry
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| {
                matches!(task.status, TaskStatus::Claimed | TaskStatus::Running)
                    && task.expires_at.is_some_and(|expiry| expiry <= now)
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        for index in &stale {
            release_task_leases(&self.root, &registry.tasks[*index]);
            let task = &mut registry.tasks[*index];
            task.status = TaskStatus::Queued;
            task.agent_id = None;
            task.lease_ids.clear();
            task.claimed_at = None;
            task.heartbeat_at = None;
            task.expires_at = None;
            task.failure = None;
        }
        Ok(stale.len())
    }

    fn with_registry<T>(
        &self,
        operation: impl FnOnce(&TaskRegistry) -> Result<T, AdeError>,
    ) -> Result<T, AdeError> {
        let lock = self.open_lock()?;
        lock.lock_shared()?;
        let registry = self.load_unlocked()?;
        operation(&registry)
    }

    fn with_registry_mut<T>(
        &self,
        operation: impl FnOnce(&mut TaskRegistry) -> Result<T, AdeError>,
    ) -> Result<T, AdeError> {
        let lock = self.open_lock()?;
        lock.lock_exclusive()?;
        let mut registry = self.load_unlocked()?;
        let result = operation(&mut registry)?;
        self.save_unlocked(&registry)?;
        Ok(result)
    }

    fn directory(&self) -> PathBuf {
        self.root.join(".ade").join("tasks")
    }

    fn open_lock(&self) -> Result<File, AdeError> {
        std::fs::create_dir_all(self.directory())?;
        Ok(OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(self.directory().join("registry.lock"))?)
    }

    fn load_unlocked(&self) -> Result<TaskRegistry, AdeError> {
        let path = self.directory().join("registry.json");
        if !path.is_file() {
            return Ok(TaskRegistry {
                schema: TASK_REGISTRY_SCHEMA.into(),
                tasks: vec![],
            });
        }
        let registry: TaskRegistry = serde_json::from_slice(&std::fs::read(path)?)?;
        if registry.schema != TASK_REGISTRY_SCHEMA {
            return Err(AdeError::Other("unsupported task registry schema".into()));
        }
        Ok(registry)
    }

    fn save_unlocked(&self, registry: &TaskRegistry) -> Result<(), AdeError> {
        let directory = self.directory();
        std::fs::create_dir_all(&directory)?;
        let target = directory.join("registry.json");
        let temporary = directory.join(format!("registry-{}.tmp", Uuid::new_v4()));
        std::fs::write(&temporary, serde_json::to_vec_pretty(registry)?)?;
        replace_file(&temporary, &target)?;
        Ok(())
    }
}

fn plan_phase_key_from_goal(goal: &str) -> Option<String> {
    let trimmed = goal.trim();
    if !trimmed.starts_with('[') {
        return None;
    }
    let end = trimmed.find(']')?;
    let key = trimmed[1..end].trim();
    if key.is_empty() {
        None
    } else {
        Some(key.to_string())
    }
}

fn validate_ttl(ttl: Duration) -> Result<(), AdeError> {
    if ttl <= Duration::zero() {
        return Err(AdeError::Other("task ttl must be positive".into()));
    }
    Ok(())
}

fn task_index(registry: &TaskRegistry, task_id: &str) -> Result<usize, AdeError> {
    Uuid::parse_str(task_id)
        .map_err(|error| AdeError::Other(format!("invalid task id: {error}")))?;
    registry
        .tasks
        .iter()
        .position(|task| task.id == task_id)
        .ok_or_else(|| AdeError::NotFound(format!("task '{task_id}'")))
}

fn assert_holder(task: &AgentTask, agent_id: Uuid) -> Result<(), AdeError> {
    if task.agent_id == Some(agent_id) {
        Ok(())
    } else {
        Err(AdeError::Authorization(format!(
            "task '{}' is held by {}, not {agent_id}",
            task.id,
            task.agent_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "no agent".into())
        )))
    }
}

fn release_task_leases(root: &Path, task: &AgentTask) {
    let manager = LeaseManager::new(root);
    for lease_id in &task.lease_ids {
        let _ = manager.release(lease_id);
    }
}

fn replace_file(source: &Path, target: &Path) -> Result<(), AdeError> {
    #[cfg(windows)]
    if target.exists() {
        std::fs::remove_file(target)?;
    }
    std::fs::rename(source, target)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!("ade-tasks-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn input(goal: &str, paths: &[&str], depends_on: Vec<String>) -> EnqueueTask {
        EnqueueTask {
            goal: goal.into(),
            owned_paths: paths.iter().map(|path| (*path).into()).collect(),
            lease_mode: LeaseMode::Strong,
            depends_on,
        }
    }

    #[test]
    fn dependencies_block_claim_until_completed() {
        let root = fixture();
        let coordinator = TaskCoordinator::new(&root);
        let first = coordinator
            .enqueue(input("first", &["src/first"], vec![]))
            .unwrap();
        let second = coordinator
            .enqueue(input("second", &["src/second"], vec![first.id.clone()]))
            .unwrap();
        let agent = Uuid::new_v4();

        let claimed = coordinator
            .claim(agent, Duration::minutes(5))
            .unwrap()
            .unwrap();
        assert_eq!(claimed.id, first.id);
        assert!(coordinator
            .claim(Uuid::new_v4(), Duration::minutes(5))
            .unwrap()
            .is_none());
        coordinator.complete(&first.id, agent).unwrap();
        let claimed_second = coordinator
            .claim(Uuid::new_v4(), Duration::minutes(5))
            .unwrap()
            .unwrap();
        assert_eq!(claimed_second.id, second.id);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn lease_conflict_leaves_task_queued() {
        let root = fixture();
        let coordinator = TaskCoordinator::new(&root);
        let holder = Uuid::new_v4();
        LeaseManager::new(&root)
            .acquire(
                holder,
                "src/shared",
                LeaseMode::Strong,
                Duration::minutes(5),
            )
            .unwrap();
        let task = coordinator
            .enqueue(input("conflict", &["src/shared/file.rs"], vec![]))
            .unwrap();
        assert!(coordinator
            .claim(Uuid::new_v4(), Duration::minutes(5))
            .is_err());
        assert_eq!(
            coordinator
                .list()
                .unwrap()
                .into_iter()
                .find(|candidate| candidate.id == task.id)
                .unwrap()
                .status,
            TaskStatus::Queued
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn stale_claim_releases_scope_and_can_be_reclaimed() {
        let root = fixture();
        let coordinator = TaskCoordinator::new(&root);
        coordinator
            .enqueue(input("stale", &["src/stale"], vec![]))
            .unwrap();
        let first_agent = Uuid::new_v4();
        coordinator
            .claim(first_agent, Duration::milliseconds(1))
            .unwrap()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let second_agent = Uuid::new_v4();
        let reclaimed = coordinator
            .claim(second_agent, Duration::minutes(5))
            .unwrap()
            .unwrap();
        assert_eq!(reclaimed.agent_id, Some(second_agent));
        assert_eq!(
            LeaseManager::new(&root)
                .resolve_owned_paths(second_agent, &["src/stale".into()])
                .unwrap(),
            vec!["src/stale"]
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn concurrent_claimers_only_claim_once() {
        let root = fixture();
        TaskCoordinator::new(&root)
            .enqueue(input("single", &["src/single"], vec![]))
            .unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let handles = (0..2)
            .map(|_| {
                let root = root.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    TaskCoordinator::new(root)
                        .claim(Uuid::new_v4(), Duration::minutes(5))
                        .unwrap()
                        .is_some()
                })
            })
            .collect::<Vec<_>>();
        let claimed = handles
            .into_iter()
            .map(|handle| usize::from(handle.join().unwrap()))
            .sum::<usize>();
        assert_eq!(claimed, 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn heartbeat_renews_task_and_leases_for_holder_only() {
        let root = fixture();
        let coordinator = TaskCoordinator::new(&root);
        let task = coordinator
            .enqueue(input("heartbeat", &["src/heartbeat"], vec![]))
            .unwrap();
        let holder = Uuid::new_v4();
        let claimed = coordinator
            .claim(holder, Duration::minutes(1))
            .unwrap()
            .unwrap();
        let renewed = coordinator
            .heartbeat(&task.id, holder, Duration::minutes(30))
            .unwrap();
        assert!(renewed.expires_at > claimed.expires_at);
        assert!(coordinator
            .heartbeat(&task.id, Uuid::new_v4(), Duration::minutes(30))
            .is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sync_from_plan_is_idempotent_and_wires_deps() {
        let root = fixture();
        let coordinator = TaskCoordinator::new(&root);
        let phases = vec![
            ade_core::plan::PlanPhase {
                id: "p1".into(),
                title: "First".into(),
                owned_paths: vec!["src/a".into()],
                gates: vec![],
                depends_on: vec![],
            },
            ade_core::plan::PlanPhase {
                id: "p2".into(),
                title: "Second".into(),
                owned_paths: vec!["src/b".into()],
                gates: vec![],
                depends_on: vec!["p1".into()],
            },
        ];
        let created = coordinator.sync_from_plan(&phases).unwrap();
        assert_eq!(created.len(), 2);
        let again = coordinator.sync_from_plan(&phases).unwrap();
        assert!(again.is_empty());
        let listed = coordinator.list().unwrap();
        let p2 = listed.iter().find(|t| t.goal.starts_with("[p2]")).unwrap();
        let p1 = listed.iter().find(|t| t.goal.starts_with("[p1]")).unwrap();
        assert_eq!(p2.depends_on, vec![p1.id.clone()]);

        let agent = Uuid::new_v4();
        assert!(coordinator
            .claim_id(&p2.id, agent, Duration::minutes(5))
            .is_err());
        let claimed = coordinator
            .claim_id(&p1.id, agent, Duration::minutes(5))
            .unwrap();
        assert_eq!(claimed.status, TaskStatus::Claimed);
        let _ = std::fs::remove_dir_all(root);
    }
}
