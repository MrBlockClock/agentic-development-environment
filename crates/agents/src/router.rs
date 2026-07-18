use ade_core::audit::{AuditMode, AuditReport};
use ade_core::execute::ExecuteReport;
use ade_core::plan::PlanReport;

pub enum Phase {
    Audit,
    Plan,
    Execute,
}

pub struct PhaseRouter;

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

    pub async fn run_audit(&self, _mode: AuditMode) -> AuditReport {
        unimplemented!("AUDIT phase implementation")
    }

    pub async fn run_plan(&self, _audit: &AuditReport) -> PlanReport {
        unimplemented!("PLAN phase implementation")
    }

    pub async fn run_execute(
        &self,
        _plan: &PlanReport,
    ) -> ExecuteReport {
        unimplemented!("EXECUTE phase implementation")
    }
}
