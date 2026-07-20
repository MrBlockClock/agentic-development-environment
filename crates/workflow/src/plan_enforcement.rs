use ade_core::error::AdeError;
use ade_core::plan::PlanReport;
use std::path::Path;

/// Enforces DEC-P-002: risky ADE work must go through PLAN before mutation.
pub struct PlanEnforcer;

impl Default for PlanEnforcer {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanEnforcer {
    pub fn new() -> Self {
        Self
    }

    /// True when change labels match risky ADE classes (schema, deploy, secrets, …).
    pub fn requires_plan(&self, changes: &[&str]) -> bool {
        changes.iter().any(|change| Self::path_is_risky(change))
    }

    fn path_is_risky(change: &str) -> bool {
        let risky = [
            "migration",
            "schema",
            "deploy",
            "secret",
            "api",
            "config",
            "multi-package",
            "multi-ade",
            "regulated",
            ".env",
            "credentials",
            "terraform",
            "dockerfile",
            "compose.yml",
            "compose.yaml",
        ];
        let lower = change.to_ascii_lowercase();
        risky.iter().any(|token| lower.contains(token))
    }

    /// Intent text uses a stricter keyword set so ordinary prompts do not trip
    /// the gate (e.g. "fix the API docs" is fine; "run the migration" is not).
    pub fn intent_requires_plan(intent: &str) -> bool {
        let strong = [
            "migration",
            "schema",
            "deploy",
            "secret",
            "regulated",
            "multi-package",
            "multi-ade",
            "credentials",
            "terraform",
            "dockerfile",
        ];
        let lower = intent.to_ascii_lowercase();
        strong.iter().any(|token| lower.contains(token))
    }

    /// Derive change labels from owned paths (intent is evaluated separately).
    pub fn labels_for(owned_paths: &[String], _intent: Option<&str>) -> Vec<String> {
        let mut labels = Vec::new();
        for path in owned_paths {
            labels.push(path.to_ascii_lowercase());
            if let Some(name) = Path::new(path).file_name().and_then(|name| name.to_str()) {
                labels.push(name.to_ascii_lowercase());
            }
        }
        labels.sort();
        labels.dedup();
        labels
    }

    pub fn plan_path(workspace: impl AsRef<Path>) -> std::path::PathBuf {
        workspace
            .as_ref()
            .join(".ade")
            .join("plan")
            .join("last.json")
    }

    /// Persist the latest plan under the workspace so runtime gates can find it.
    pub fn save_plan(workspace: impl AsRef<Path>, plan: &PlanReport) -> Result<(), AdeError> {
        let path = Self::plan_path(workspace);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_vec_pretty(plan)?)?;
        Ok(())
    }

    pub fn load_plan(workspace: impl AsRef<Path>) -> Result<Option<PlanReport>, AdeError> {
        let path = Self::plan_path(workspace);
        if !path.is_file() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&path)?;
        Ok(Some(serde_json::from_str(&raw)?))
    }

    /// Block risky agent/file work when no workspace PLAN artifact exists.
    pub fn ensure_approved_plan(
        &self,
        workspace: impl AsRef<Path>,
        owned_paths: &[String],
        intent: Option<&str>,
    ) -> Result<(), AdeError> {
        let labels = Self::labels_for(owned_paths, intent);
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let intent_risky = intent.is_some_and(Self::intent_requires_plan);
        if !self.requires_plan(&refs) && !intent_risky {
            return Ok(());
        }
        let workspace = workspace.as_ref();
        match Self::load_plan(workspace)? {
            Some(plan) if roots_compatible(&plan.audit_root, workspace) => Ok(()),
            Some(_) => Err(AdeError::Authorization(
                "risky change requires a PLAN for this workspace root — run `ade plan` again"
                    .into(),
            )),
            None => Err(AdeError::Authorization(format!(
                "risky change ({}) requires PLAN before act — run `ade plan` (writes {})",
                refs.first()
                    .copied()
                    .or(intent.map(str::trim))
                    .unwrap_or("risky"),
                Self::plan_path(workspace).display()
            ))),
        }
    }
}

fn roots_compatible(plan_root: &str, workspace: &Path) -> bool {
    let plan = std::path::PathBuf::from(plan_root);
    let left = plan.canonicalize().unwrap_or(plan);
    let right = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    left == right
        || left.to_string_lossy() == workspace.display().to_string()
        || plan_root == workspace.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ade_core::audit::{AuditMode, AuditRunner};
    use ade_core::plan::PlanBuilder;

    #[test]
    fn detects_risky_labels() {
        let enforcer = PlanEnforcer::new();
        assert!(enforcer.requires_plan(&["db/migration/001.sql"]));
        assert!(enforcer.requires_plan(&["rotate production secrets"]));
        assert!(!enforcer.requires_plan(&["docs/readme.md"]));
    }

    #[test]
    fn gates_risky_work_without_plan_artifact() {
        let root = std::env::temp_dir().join(format!("ade-plan-enf-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("AGENTS.md"), "# contract\n").unwrap();
        let enforcer = PlanEnforcer::new();
        let err = enforcer
            .ensure_approved_plan(&root, &["migrations/001.sql".into()], None)
            .unwrap_err();
        assert!(err.to_string().contains("requires PLAN"));

        let audit = AuditRunner::new(&root).run(AuditMode::EvaluateExisting);
        let plan = PlanBuilder::new().build(&audit);
        PlanEnforcer::save_plan(&root, &plan).unwrap();
        enforcer
            .ensure_approved_plan(&root, &["migrations/001.sql".into()], None)
            .unwrap();
        let _ = std::fs::remove_dir_all(root);
    }
}
