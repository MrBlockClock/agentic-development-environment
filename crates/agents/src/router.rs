use ade_core::audit::{AuditMode, AuditReport};
use ade_core::error::AdeError;
use ade_core::execute::{ExecuteOptions, ExecuteReport, ExecuteRunner};
use ade_core::plan::PlanReport;

pub enum Phase {
    Audit,
    Plan,
    Execute,
}

pub struct PhaseRouter;

impl Default for PhaseRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl PhaseRouter {
    pub fn new() -> Self {
        Self
    }

    pub fn route(&self, health_known: bool, plan_approved: bool) -> Phase {
        if !health_known {
            Phase::Audit
        } else if !plan_approved {
            Phase::Plan
        } else {
            Phase::Execute
        }
    }

    pub async fn run_audit(&self, mode: AuditMode) -> AuditReport {
        let root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        ade_core::audit::AuditRunner::new(root).run(mode)
    }

    pub async fn run_plan(&self, audit: &AuditReport) -> PlanReport {
        ade_core::plan::PlanBuilder::new().build(audit)
    }

    pub async fn run_execute(
        &self,
        plan: &PlanReport,
        opts: &ExecuteOptions,
    ) -> Result<ExecuteReport, AdeError> {
        let root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        ExecuteRunner::new(root).run(plan, opts)
    }
}
