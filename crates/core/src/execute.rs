use crate::agents_contract::{AgentsContractContext, AgentsContractGenerator};
use crate::audit::{AuditMode, AuditRunner};
use crate::error::AdeError;
use crate::plan::PlanReport;
use crate::recipe::{builtin_recipe, StackRecipe};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

pub const EXECUTE_SCHEMA: &str = "ade.execute.report/v1";

/// Result of applying an approved PLAN (owned paths only).
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteReport {
    pub schema: String,
    pub mode: String,
    pub approved_plan_ref: String,
    pub phases_completed: Vec<String>,
    pub changed_paths: Vec<String>,
    pub verify_evidence: Vec<VerifyEvidence>,
    pub score_before: Option<u32>,
    pub score_after: Option<u32>,
    pub score_max: u32,
    pub improvements: Vec<String>,
    pub remaining_gaps: Vec<String>,
    pub requires_human: Vec<String>,
    pub ready_for_focused_work: bool,
    pub human_summary_markdown: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyEvidence {
    pub gate: String,
    pub command: String,
    pub status: String,
    pub notes: Option<String>,
}

/// Runtime options for EXECUTE. Approval is never stored on the plan itself.
#[derive(Debug, Clone)]
pub struct ExecuteOptions {
    /// Explicit human approval required before any mutation.
    pub approved: bool,
    /// Recipe used when writing a missing `AGENTS.md`.
    pub recipe_id: String,
    /// If non-empty, only these plan phase ids are considered.
    pub phase_ids: Vec<String>,
}

impl Default for ExecuteOptions {
    fn default() -> Self {
        Self {
            approved: false,
            recipe_id: "rust-api-turso".into(),
            phase_ids: vec![],
        }
    }
}

/// Applies approved plan phases. Never expands scope beyond `owned_paths`.
pub struct ExecuteRunner {
    root: PathBuf,
}

impl ExecuteRunner {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn run(&self, plan: &PlanReport, opts: &ExecuteOptions) -> Result<ExecuteReport, AdeError> {
        if !opts.approved {
            return Err(AdeError::Authorization(
                "EXECUTE requires explicit human approval (pass --approve)".into(),
            ));
        }

        let root_display = self.root.display().to_string();
        if !roots_match(&plan.audit_root, &root_display) {
            return Err(AdeError::PlanValidation(format!(
                "plan root '{}' does not match current root '{}'",
                plan.audit_root, root_display
            )));
        }

        let recipe = builtin_recipe(&opts.recipe_id)?;
        let mut phases_completed = Vec::new();
        let mut changed_paths = Vec::new();
        let mut improvements = Vec::new();
        let mut remaining_gaps = Vec::new();
        let mut requires_human = Vec::new();

        for phase in &plan.phases {
            if !opts.phase_ids.is_empty() && !opts.phase_ids.contains(&phase.id) {
                continue;
            }

            if phase.owned_paths.is_empty() {
                remaining_gaps.push(phase.title.clone());
                requires_human.push(format!(
                    "Phase {} has no owned_paths — needs focused agent work",
                    phase.id
                ));
                continue;
            }

            let mut phase_changed = false;
            for rel in &phase.owned_paths {
                ensure_safe_owned_path(rel)?;
                match self.remediate(rel, &recipe)? {
                    Remediation::Wrote(msg) => {
                        changed_paths.push(rel.clone());
                        improvements.push(msg);
                        phase_changed = true;
                    }
                    Remediation::AlreadyPresent => {
                        improvements.push(format!("{rel}: already present — skipped"));
                    }
                    Remediation::Unsupported => {
                        requires_human.push(format!(
                            "No safe remediation for '{rel}' in phase {}",
                            phase.id
                        ));
                        remaining_gaps.push(format!("{} ({rel})", phase.title));
                    }
                }
            }

            if phase_changed || phase.owned_paths.iter().all(|p| self.root.join(p).exists()) {
                phases_completed.push(phase.id.clone());
            }
        }

        let verify_evidence = self.verify_g0();
        let g0_ok = verify_evidence.iter().all(|v| v.status == "pass");

        let audit_after = AuditRunner::new(&self.root).run(AuditMode::EvaluateExisting);
        let score_after = audit_after.score;
        let score_max = audit_after.score_max.max(plan.score_max);
        let ready_for_focused_work =
            g0_ok && audit_after.blockers.is_empty() && remaining_gaps.is_empty();

        if !audit_after.blockers.is_empty() {
            for b in &audit_after.blockers {
                requires_human.push(format!("Remaining blocker after EXECUTE: {b}"));
            }
        }

        let human_summary_markdown = Some(summary_markdown(
            plan,
            &phases_completed,
            &changed_paths,
            plan.score_before,
            score_after,
            score_max,
            &remaining_gaps,
            &requires_human,
        ));

        Ok(ExecuteReport {
            schema: EXECUTE_SCHEMA.into(),
            mode: "apply_approved".into(),
            approved_plan_ref: format!("plan@{}", plan.audit_root),
            phases_completed,
            changed_paths,
            verify_evidence,
            score_before: Some(plan.score_before),
            score_after: Some(score_after),
            score_max,
            improvements,
            remaining_gaps,
            requires_human,
            ready_for_focused_work,
            human_summary_markdown,
        })
    }

    fn remediate(&self, rel: &str, recipe: &StackRecipe) -> Result<Remediation, AdeError> {
        let path = self.root.join(rel);
        if path.exists() {
            return Ok(Remediation::AlreadyPresent);
        }

        match rel {
            ".gitignore" => {
                std::fs::write(&path, DEFAULT_GITIGNORE)?;
                Ok(Remediation::Wrote(format!(
                    "wrote .gitignore ({})",
                    path.display()
                )))
            }
            ".cursorignore" => {
                std::fs::write(&path, DEFAULT_CURSORIGNORE)?;
                Ok(Remediation::Wrote(format!(
                    "wrote .cursorignore ({})",
                    path.display()
                )))
            }
            "AGENTS.md" => {
                let name = self
                    .root
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("project")
                    .to_string();
                let ctx =
                    AgentsContractContext::new(name).with_root(self.root.display().to_string());
                AgentsContractGenerator::write(&self.root, recipe, &ctx, false)?;
                Ok(Remediation::Wrote(format!(
                    "wrote AGENTS.md from recipe '{}'",
                    recipe.id
                )))
            }
            _ => Ok(Remediation::Unsupported),
        }
    }

    fn verify_g0(&self) -> Vec<VerifyEvidence> {
        let exists = self.root.exists();
        let has_manifest = self.root.join("Cargo.toml").exists()
            || self.root.join("package.json").exists()
            || self.root.join("pyproject.toml").exists();
        let status = if exists && has_manifest {
            "pass"
        } else if exists {
            "warn"
        } else {
            "fail"
        };
        vec![VerifyEvidence {
            gate: "G0".into(),
            command: "probe project root + manifest".into(),
            status: status.into(),
            notes: Some(format!(
                "root_exists={exists} manifest={}",
                if has_manifest { "found" } else { "missing" }
            )),
        }]
    }
}

enum Remediation {
    Wrote(String),
    AlreadyPresent,
    Unsupported,
}

const DEFAULT_GITIGNORE: &str = "\
# Generated by ADE EXECUTE — minimal safe defaults
target/
node_modules/
dist/
.env
.env.*
!.env.example
*.pem
*.key
";

const DEFAULT_CURSORIGNORE: &str = "\
# Generated by ADE EXECUTE — keep secrets and build artifacts out of AI index
target/
node_modules/
dist/
.env
.env.*
!.env.example
*.pem
*.key
**/*credentials*
";

fn ensure_safe_owned_path(rel: &str) -> Result<(), AdeError> {
    let path = Path::new(rel);
    if looks_absolute(rel) {
        return Err(AdeError::Execution(format!(
            "owned_path '{rel}' must be relative — refusing absolute path"
        )));
    }
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(AdeError::Execution(format!(
            "owned_path '{rel}' escapes project root via '..'"
        )));
    }
    if rel.is_empty() || rel.contains('\0') {
        return Err(AdeError::Execution("owned_path is empty or invalid".into()));
    }
    Ok(())
}

/// Reject host-absolute paths on every OS, including Windows drive/UNC forms on Unix
/// and Unix-rooted paths on Windows.
fn looks_absolute(value: &str) -> bool {
    let path = Path::new(value);
    if path.is_absolute() {
        return true;
    }
    let normalized = value.replace('\\', "/");
    if normalized.starts_with('/') || normalized.starts_with("~/") {
        return true;
    }
    let bytes = normalized.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn roots_match(plan_root: &str, current: &str) -> bool {
    let a = PathBuf::from(plan_root);
    let b = PathBuf::from(current);
    a == b
        || a.canonicalize()
            .ok()
            .zip(b.canonicalize().ok())
            .is_some_and(|(x, y)| x == y)
}

#[allow(clippy::too_many_arguments)]
fn summary_markdown(
    plan: &PlanReport,
    phases_completed: &[String],
    changed_paths: &[String],
    score_before: u32,
    score_after: u32,
    score_max: u32,
    remaining_gaps: &[String],
    requires_human: &[String],
) -> String {
    let mut md = format!(
        "## EXECUTE — score {}/{} → {}/{}\n\n",
        score_before, score_max, score_after, score_max
    );
    md.push_str(&format!(
        "- Phases completed: {}\n- Paths changed: {}\n",
        phases_completed.len(),
        changed_paths.len()
    ));
    if !changed_paths.is_empty() {
        md.push_str("\n### Changed paths\n\n");
        for p in changed_paths {
            md.push_str(&format!("- `{p}`\n"));
        }
    }
    if !remaining_gaps.is_empty() {
        md.push_str("\n### Remaining gaps\n\n");
        for g in remaining_gaps {
            md.push_str(&format!("- {g}\n"));
        }
    }
    if !requires_human.is_empty() {
        md.push_str("\n### Requires human\n\n");
        for r in requires_human {
            md.push_str(&format!("- {r}\n"));
        }
    }
    md.push_str(&format!("\n_Plan root: `{}`_\n", plan.audit_root));
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditRunner;
    use crate::plan::PlanBuilder;
    use std::fs;

    fn fixture(with_contract: bool) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ade-exec-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("Cargo.toml"), "[workspace]\n").unwrap();
        if with_contract {
            fs::write(dir.join(".gitignore"), "target/\n").unwrap();
            fs::write(dir.join(".cursorignore"), "target/\n").unwrap();
            fs::write(dir.join("AGENTS.md"), "# contract\n").unwrap();
        }
        dir
    }

    #[test]
    fn refuses_without_approval() {
        let dir = fixture(false);
        let audit = AuditRunner::new(&dir).run(AuditMode::EvaluateExisting);
        let plan = PlanBuilder::new().build(&audit);
        let err = ExecuteRunner::new(&dir)
            .run(&plan, &ExecuteOptions::default())
            .unwrap_err();
        assert!(err.to_string().contains("approval"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn applies_blocker_owned_paths_and_improves_score() {
        let dir = fixture(false);
        let audit = AuditRunner::new(&dir).run(AuditMode::EvaluateExisting);
        assert!(!audit.blockers.is_empty());
        let before = audit.score;
        let plan = PlanBuilder::new().build(&audit);

        let report = ExecuteRunner::new(&dir)
            .run(
                &plan,
                &ExecuteOptions {
                    approved: true,
                    recipe_id: "rust-api-turso".into(),
                    phase_ids: vec![],
                },
            )
            .unwrap();

        assert!(dir.join("AGENTS.md").exists());
        assert!(dir.join(".gitignore").exists());
        assert!(dir.join(".cursorignore").exists());
        assert!(report.changed_paths.contains(&"AGENTS.md".into()));
        assert!(report.score_after.unwrap() >= before);
        assert!(report
            .phases_completed
            .iter()
            .any(|p| p == "phase-0-blockers"));
        assert_eq!(report.schema, EXECUTE_SCHEMA);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_path_escape() {
        assert!(ensure_safe_owned_path("../secrets").is_err());
        assert!(ensure_safe_owned_path(r"C:\Windows").is_err());
        assert!(ensure_safe_owned_path("C:/Windows").is_err());
        assert!(ensure_safe_owned_path("//server/share").is_err());
        assert!(ensure_safe_owned_path("/etc/passwd").is_err());
        assert!(ensure_safe_owned_path("AGENTS.md").is_ok());
    }

    #[test]
    fn empty_owned_paths_remain_gaps() {
        let dir = fixture(true);
        let audit = AuditRunner::new(&dir).run(AuditMode::EvaluateExisting);
        let plan = PlanBuilder::new().build(&audit);
        // Force a synthetic phase with no ownership if the real plan is empty of gaps.
        let mut plan = plan;
        if plan.phases.is_empty() {
            plan.phases.push(crate::plan::PlanPhase {
                id: "phase-gap".into(),
                title: "Synthetic gap".into(),
                owned_paths: vec![],
                gates: vec![],
                depends_on: vec![],
            });
        }
        // Prefer an existing empty-owned phase when present.
        let has_empty = plan.phases.iter().any(|p| p.owned_paths.is_empty());
        if !has_empty {
            plan.phases.push(crate::plan::PlanPhase {
                id: "phase-gap".into(),
                title: "Synthetic gap".into(),
                owned_paths: vec![],
                gates: vec![],
                depends_on: vec![],
            });
        }

        let report = ExecuteRunner::new(&dir)
            .run(
                &plan,
                &ExecuteOptions {
                    approved: true,
                    recipe_id: "rust-api-turso".into(),
                    phase_ids: vec![],
                },
            )
            .unwrap();
        assert!(!report.remaining_gaps.is_empty() || !report.requires_human.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
