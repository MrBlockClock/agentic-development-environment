use ade_core::execute::ExecuteReport;
use ade_core::plan::Phase;

pub struct PhaseExecutor;

impl Default for PhaseExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl PhaseExecutor {
    pub fn new() -> Self {
        Self
    }

    pub fn execute(&self, _phase: &Phase) -> Result<ExecuteReport, String> {
        // TODO: execute approved phase, track changes, run verify
        Err("Not implemented".to_string())
    }
}
