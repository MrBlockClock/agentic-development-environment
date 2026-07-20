use ade_core::error::AdeError;
use ade_core::plan::{order_phases, PlanPhase};

/// Thin workflow wrapper around core plan DAG ordering.
pub struct DagBuilder;

impl Default for DagBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DagBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn build(&self, phases: Vec<PlanPhase>) -> Result<Vec<PlanPhase>, AdeError> {
        order_phases(phases)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phase(id: &str, deps: &[&str]) -> PlanPhase {
        PlanPhase {
            id: id.into(),
            title: id.into(),
            owned_paths: vec![],
            gates: vec![],
            depends_on: deps.iter().map(|dep| (*dep).into()).collect(),
        }
    }

    #[test]
    fn orders_dependencies_first() {
        let ordered = DagBuilder::new()
            .build(vec![
                phase("b", &["a"]),
                phase("c", &["b"]),
                phase("a", &[]),
            ])
            .unwrap();
        let ids: Vec<_> = ordered.iter().map(|phase| phase.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn rejects_cycles_and_unknown_deps() {
        assert!(DagBuilder::new()
            .build(vec![phase("a", &["b"]), phase("b", &["a"])])
            .is_err());
        assert!(DagBuilder::new()
            .build(vec![phase("a", &["missing"])])
            .is_err());
    }
}
