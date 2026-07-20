use crate::audit::{AuditFinding, AuditReport};
use crate::error::AdeError;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

pub const PLAN_SCHEMA: &str = "ade.plan.report/v1";

/// Phases of the AUDIT → PLAN → EXECUTE router.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Audit,
    Plan,
    Execute,
}

impl Phase {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Audit => "AUDIT",
            Self::Plan => "PLAN",
            Self::Execute => "EXECUTE",
        }
    }
}

/// A phased plan with gates and ownership, produced by the PLAN phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanReport {
    pub schema: String,
    /// The audit report this plan was derived from (root path).
    pub audit_root: String,
    /// Audit score at planning time, for the EXECUTE score delta.
    pub score_before: u32,
    pub score_max: u32,
    pub phases: Vec<PlanPhase>,
    pub requires_human: Vec<String>,
    pub human_summary_markdown: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanPhase {
    pub id: String,
    pub title: String,
    pub owned_paths: Vec<String>,
    pub gates: Vec<String>,
    pub depends_on: Vec<String>,
}

/// Default verify gates attached to every remediation phase.
const DEFAULT_GATES: [&str; 3] = [
    "G0: project root present",
    "G2: format + lint clean",
    "G3: tests pass",
];

/// Builds a `PlanReport` from an `AuditReport`. Read-mostly: never mutates the workspace.
pub struct PlanBuilder;

impl Default for PlanBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn build(&self, audit: &AuditReport) -> PlanReport {
        let mut phases = Vec::new();
        let mut requires_human = Vec::new();

        // Blockers always come first and always need a human decision.
        if !audit.blockers.is_empty() {
            phases.push(PlanPhase {
                id: "phase-0-blockers".into(),
                title: format!("Resolve {} audit blocker(s)", audit.blockers.len()),
                owned_paths: owned_paths_for_blockers(&audit.blockers),
                gates: DEFAULT_GATES.iter().map(|s| s.to_string()).collect(),
                depends_on: vec![],
            });
            for b in &audit.blockers {
                requires_human.push(format!("Approve blocker fix: {b}"));
            }
        }

        // One remediation phase per non-passing finding, ordered worst-first.
        let mut gaps: Vec<&AuditFinding> = audit
            .findings
            .iter()
            .filter(|f| f.points < f.points_max && f.severity != "info")
            .collect();
        gaps.sort_by_key(|f| f.points);

        let root_dep: Vec<String> = if phases.is_empty() {
            vec![]
        } else {
            vec![phases[0].id.clone()]
        };

        for (i, finding) in gaps.iter().enumerate() {
            phases.push(PlanPhase {
                id: format!("phase-{}-{}", i + 1, slug(&finding.layer)),
                title: format!(
                    "Close gap in {} ({}/{}): {}",
                    finding.layer, finding.points, finding.points_max, finding.detail
                ),
                owned_paths: vec![],
                gates: DEFAULT_GATES.iter().map(|s| s.to_string()).collect(),
                depends_on: root_dep.clone(),
            });
        }

        // EXECUTE may not start without explicit human approval of this plan.
        requires_human.push("Approve this plan before EXECUTE".into());

        let human_summary_markdown = Some(summary_markdown(audit, &phases, &requires_human));
        let phases = match order_phases(phases.clone()) {
            Ok(ordered) => ordered,
            Err(error) => {
                tracing::warn!(%error, "plan phase ordering failed; using declaration order");
                phases
            }
        };

        PlanReport {
            schema: PLAN_SCHEMA.into(),
            audit_root: audit.root.clone(),
            score_before: audit.score,
            score_max: audit.score_max,
            phases,
            requires_human,
            human_summary_markdown,
        }
    }
}

/// Topologically order plan phases by `depends_on` (Kahn). Rejects cycles and
/// unknown dependency ids.
pub fn order_phases(phases: Vec<PlanPhase>) -> Result<Vec<PlanPhase>, AdeError> {
    if phases.is_empty() {
        return Ok(phases);
    }

    let mut by_id = HashMap::new();
    for phase in &phases {
        if by_id.insert(phase.id.clone(), phase).is_some() {
            return Err(AdeError::PlanValidation(format!(
                "duplicate plan phase id '{}'",
                phase.id
            )));
        }
    }

    let ids: HashSet<&str> = by_id.keys().map(String::as_str).collect();
    let mut indegree: HashMap<&str, usize> = HashMap::new();
    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();

    for phase in &phases {
        indegree.entry(phase.id.as_str()).or_insert(0);
        for dep in &phase.depends_on {
            if !ids.contains(dep.as_str()) {
                return Err(AdeError::PlanValidation(format!(
                    "phase '{}' depends on unknown phase '{}'",
                    phase.id, dep
                )));
            }
            *indegree.entry(phase.id.as_str()).or_insert(0) += 1;
            dependents
                .entry(dep.as_str())
                .or_default()
                .push(phase.id.as_str());
        }
    }

    let mut queue: VecDeque<&str> = indegree
        .iter()
        .filter_map(|(id, degree)| (*degree == 0).then_some(*id))
        .collect();
    queue
        .make_contiguous()
        .sort_by_key(|id| phases.iter().position(|phase| phase.id == *id).unwrap_or(0));

    let mut ordered = Vec::with_capacity(phases.len());
    while let Some(id) = queue.pop_front() {
        ordered.push(by_id[id].clone());
        if let Some(children) = dependents.get(id) {
            let mut next = Vec::new();
            for child in children {
                if let Some(degree) = indegree.get_mut(child) {
                    *degree = degree.saturating_sub(1);
                    if *degree == 0 {
                        next.push(*child);
                    }
                }
            }
            next.sort_by_key(|id| phases.iter().position(|phase| phase.id == *id).unwrap_or(0));
            queue.extend(next);
        }
    }

    if ordered.len() != phases.len() {
        return Err(AdeError::PlanValidation(
            "plan phases contain a dependency cycle".into(),
        ));
    }
    Ok(ordered)
}

fn owned_paths_for_blockers(blockers: &[String]) -> Vec<String> {
    let mut paths = Vec::new();
    for b in blockers {
        if b.contains(".gitignore") {
            paths.push(".gitignore".to_string());
        }
        if b.contains(".cursorignore") {
            paths.push(".cursorignore".to_string());
        }
        if b.contains("AGENTS.md") {
            paths.push("AGENTS.md".to_string());
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn slug(layer: &str) -> String {
    layer
        .chars()
        .take_while(|c| !c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn summary_markdown(
    audit: &AuditReport,
    phases: &[PlanPhase],
    requires_human: &[String],
) -> String {
    let mut md = format!(
        "## PLAN — {} phase(s) from audit score {}/{}\n\n",
        phases.len(),
        audit.score,
        audit.score_max
    );
    for p in phases {
        md.push_str(&format!("- **{}** — {}\n", p.id, p.title));
    }
    if !requires_human.is_empty() {
        md.push_str("\n### Requires human\n\n");
        for r in requires_human {
            md.push_str(&format!("- {r}\n"));
        }
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{AuditMode, AuditRunner};

    fn audit_fixture(with_contract: bool) -> AuditReport {
        let dir = std::env::temp_dir().join(format!("ade-plan-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[workspace]\n").unwrap();
        std::fs::write(dir.join(".gitignore"), "target/\n").unwrap();
        std::fs::write(dir.join(".cursorignore"), "target/\n").unwrap();
        if with_contract {
            std::fs::write(dir.join("AGENTS.md"), "# contract\n").unwrap();
        }
        let report = AuditRunner::new(&dir).run(AuditMode::EvaluateExisting);
        let _ = std::fs::remove_dir_all(&dir);
        report
    }

    #[test]
    fn plan_orders_blockers_first_and_requires_approval() {
        let audit = audit_fixture(false);
        assert!(!audit.blockers.is_empty());
        let plan = PlanBuilder::new().build(&audit);
        assert_eq!(plan.schema, PLAN_SCHEMA);
        assert_eq!(plan.phases[0].id, "phase-0-blockers");
        assert!(plan.phases[0]
            .owned_paths
            .contains(&"AGENTS.md".to_string()));
        // Later phases depend on the blocker phase.
        assert!(plan.phases[1..]
            .iter()
            .all(|p| p.depends_on == vec!["phase-0-blockers".to_string()]));
        assert!(plan
            .requires_human
            .iter()
            .any(|r| r.contains("Approve this plan")));
    }

    #[test]
    fn order_phases_puts_dependencies_first() {
        let ordered = order_phases(vec![
            PlanPhase {
                id: "b".into(),
                title: "b".into(),
                owned_paths: vec![],
                gates: vec![],
                depends_on: vec!["a".into()],
            },
            PlanPhase {
                id: "a".into(),
                title: "a".into(),
                owned_paths: vec![],
                gates: vec![],
                depends_on: vec![],
            },
        ])
        .unwrap();
        assert_eq!(ordered[0].id, "a");
        assert_eq!(ordered[1].id, "b");
    }

    #[test]
    fn plan_without_blockers_has_independent_phases() {
        let audit = audit_fixture(true);
        assert!(audit.blockers.is_empty());
        let plan = PlanBuilder::new().build(&audit);
        assert!(plan.phases.iter().all(|p| p.id != "phase-0-blockers"));
        assert!(plan.phases.iter().all(|p| p.depends_on.is_empty()));
        assert_eq!(plan.score_before, audit.score);
    }

    #[test]
    fn every_phase_carries_verify_gates() {
        let plan = PlanBuilder::new().build(&audit_fixture(true));
        assert!(!plan.phases.is_empty());
        assert!(plan
            .phases
            .iter()
            .all(|p| p.gates.iter().any(|g| g.starts_with("G3"))));
    }
}
