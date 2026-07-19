use ade_core::error::AdeError;
use ade_workflow::parallel::LeaseManager;
use ade_workflow::tasks::TaskCoordinator;
use serde::Serialize;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct SchedulerTick {
    pub requeued_tasks: usize,
    pub released_leases: usize,
}

/// Periodic coordination housekeeping for the local daemon.
///
/// Agent execution remains explicitly configured by a caller. This scheduler
/// only recovers expired task claims and leases so abandoned workers cannot
/// permanently block the queue.
pub struct Scheduler {
    workspace_root: PathBuf,
    interval: Duration,
}

impl Default for Scheduler {
    fn default() -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::new(root)
    }
}

impl Scheduler {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            interval: Duration::from_secs(30),
        }
    }

    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    pub fn tick(&self) -> Result<SchedulerTick, AdeError> {
        // Requeue first: it attempts to release task-owned leases. The generic
        // stale lease sweep then removes any unrelated expired entries.
        let requeued_tasks = TaskCoordinator::new(&self.workspace_root).requeue_expired()?;
        let released_leases = LeaseManager::new(&self.workspace_root).release_stale()?;
        Ok(SchedulerTick {
            requeued_tasks,
            released_leases,
        })
    }

    pub async fn run(&self) {
        let mut interval = tokio::time::interval(self.interval);
        loop {
            interval.tick().await;
            match self.tick() {
                Ok(tick) if tick.requeued_tasks > 0 || tick.released_leases > 0 => {
                    tracing::info!(
                        requeued_tasks = tick.requeued_tasks,
                        released_leases = tick.released_leases,
                        "ADE scheduler recovered stale coordination state"
                    );
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(%error, "ADE scheduler tick failed");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ade_workflow::parallel::LeaseMode;
    use ade_workflow::tasks::EnqueueTask;
    use chrono::Duration as ChronoDuration;
    use uuid::Uuid;

    #[test]
    fn tick_requeues_expired_claims() {
        let root = std::env::temp_dir().join(format!("ade-scheduler-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let coordinator = TaskCoordinator::new(&root);
        coordinator
            .enqueue(EnqueueTask {
                goal: "recover me".into(),
                owned_paths: vec!["src/recover".into()],
                lease_mode: LeaseMode::Strong,
                depends_on: vec![],
            })
            .unwrap();
        coordinator
            .claim(Uuid::new_v4(), ChronoDuration::milliseconds(1))
            .unwrap()
            .unwrap();
        std::thread::sleep(Duration::from_millis(5));

        let tick = Scheduler::new(&root).tick().unwrap();
        assert_eq!(tick.requeued_tasks, 1);
        assert_eq!(
            coordinator.list().unwrap()[0].status,
            ade_workflow::tasks::TaskStatus::Queued
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
