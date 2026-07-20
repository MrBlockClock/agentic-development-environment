use ade_core::error::AdeError;
use ade_core::execute::{ExecuteOptions, ExecuteReport, ExecuteRunner};
use ade_core::plan::PlanReport;
use std::path::PathBuf;

pub struct PhaseExecutor {
    root: PathBuf,
}

impl Default for PhaseExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl PhaseExecutor {
    pub fn new() -> Self {
        Self {
            root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Apply an approved plan via the core EXECUTE runner.
    pub fn execute(
        &self,
        plan: &PlanReport,
        opts: &ExecuteOptions,
    ) -> Result<ExecuteReport, AdeError> {
        let mut ordered = plan.clone();
        ordered.phases = crate::dag::DagBuilder::new().build(plan.phases.clone())?;
        ExecuteRunner::new(&self.root).run(&ordered, opts)
    }
}
