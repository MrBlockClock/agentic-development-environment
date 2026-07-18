pub struct PlanEnforcer;

impl PlanEnforcer {
    pub fn new() -> Self {
        Self
    }

    pub fn requires_plan(&self, changes: &[&str]) -> bool {
        let risky = [
            "migration", "schema", "deploy", "secret", "api", "config",
            "multi-package", "multi-ade", "regulated",
        ];
        changes.iter().any(|c| risky.iter().any(|r| c.contains(r)))
    }
}
