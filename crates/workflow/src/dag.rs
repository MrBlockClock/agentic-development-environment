//! Topological ordering for plan phases by `depends_on`.
//!
//! Uses owned [`String`] keys throughout so the sorter stays free of
//! borrow-checker / rust-analyzer lifetime noise.

use ade_core::error::AdeError;
use ade_core::plan::PlanPhase;
use std::collections::{HashMap, HashSet, VecDeque};

/// Orders plan phases so every dependency appears before its dependents.
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

/// Kahn topo-sort. Unknown dependency ids and cycles are hard errors.
pub fn order_phases(phases: Vec<PlanPhase>) -> Result<Vec<PlanPhase>, AdeError> {
    if phases.is_empty() {
        return Ok(phases);
    }

    let mut index_by_id: HashMap<String, usize> = HashMap::with_capacity(phases.len());
    for (index, phase) in phases.iter().enumerate() {
        if index_by_id.insert(phase.id.clone(), index).is_some() {
            return Err(AdeError::PlanValidation(format!(
                "duplicate plan phase id '{}'",
                phase.id
            )));
        }
    }

    let known: HashSet<String> = index_by_id.keys().cloned().collect();
    let mut indegree: HashMap<String, usize> = HashMap::with_capacity(phases.len());
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

    for phase in &phases {
        indegree.entry(phase.id.clone()).or_insert(0);
        for dep in &phase.depends_on {
            if !known.contains(dep) {
                return Err(AdeError::PlanValidation(format!(
                    "phase '{}' depends on unknown phase '{}'",
                    phase.id, dep
                )));
            }
            *indegree.entry(phase.id.clone()).or_insert(0) += 1;
            dependents
                .entry(dep.clone())
                .or_default()
                .push(phase.id.clone());
        }
    }

    let mut ready: Vec<String> = indegree
        .iter()
        .filter(|&(_, degree)| *degree == 0)
        .map(|(id, _)| id.clone())
        .collect();
    ready.sort_by_key(|id| index_by_id.get(id).copied().unwrap_or(usize::MAX));

    let mut queue: VecDeque<String> = ready.into();
    let mut ordered: Vec<PlanPhase> = Vec::with_capacity(phases.len());

    while let Some(id) = queue.pop_front() {
        let index = index_by_id[&id];
        ordered.push(phases[index].clone());

        let mut unlocked = Vec::new();
        if let Some(children) = dependents.get(&id) {
            for child in children {
                if let Some(degree) = indegree.get_mut(child) {
                    *degree = degree.saturating_sub(1);
                    if *degree == 0 {
                        unlocked.push(child.clone());
                    }
                }
            }
        }
        unlocked.sort_by_key(|id| index_by_id.get(id).copied().unwrap_or(usize::MAX));
        queue.extend(unlocked);
    }

    if ordered.len() != phases.len() {
        return Err(AdeError::PlanValidation(
            "plan phases contain a dependency cycle".into(),
        ));
    }
    Ok(ordered)
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
    fn preserves_stable_order_among_ready_nodes() {
        let ordered = DagBuilder::new()
            .build(vec![phase("a", &[]), phase("c", &[]), phase("b", &[])])
            .unwrap();
        let ids: Vec<_> = ordered.iter().map(|phase| phase.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "c", "b"]);
    }

    #[test]
    fn rejects_cycles_and_unknown_deps() {
        assert!(DagBuilder::new()
            .build(vec![phase("a", &["b"]), phase("b", &["a"])])
            .is_err());
        assert!(DagBuilder::new()
            .build(vec![phase("a", &["missing"])])
            .is_err());
        assert!(DagBuilder::new()
            .build(vec![phase("a", &[]), phase("a", &[])])
            .is_err());
    }
}
