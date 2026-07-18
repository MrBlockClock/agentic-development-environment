use uuid::Uuid;

pub enum LeaseMode {
    Observe,
    Cooperative,
    Strong,
    Exclusive,
}

pub struct PathLease {
    pub agent_id: Uuid,
    pub path: String,
    pub mode: LeaseMode,
}

pub struct WorktreeManager;

impl Default for WorktreeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WorktreeManager {
    pub fn new() -> Self {
        Self
    }

    pub fn lease_path(&self, _agent_id: Uuid, _path: &str, _mode: LeaseMode) -> Result<(), String> {
        // TODO: check for conflicts, register lease
        Ok(())
    }
}
