use crate::autonomy::AutonomyLevel;
use crate::model_profile::{route, ModelProfileCatalog, RouteDecision, RouteInput};
use ade_core::money::Money;

pub struct TaskModelInsight {
    pub model: String,
    pub accept_rate: f32,
    pub avg_tokens: u64,
    pub best_for: Vec<String>,
    pub worst_for: Vec<String>,
}

pub struct ModelSelector {
    catalog: ModelProfileCatalog,
}

impl Default for ModelSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelSelector {
    pub fn new() -> Self {
        Self {
            catalog: ModelProfileCatalog::builtins(),
        }
    }

    pub fn with_catalog(catalog: ModelProfileCatalog) -> Self {
        Self { catalog }
    }

    /// Suggest a profile id for the task (H3). Returns role-default when history is empty.
    pub fn suggest_model(
        &self,
        task_description: &str,
        _history: &[TaskModelInsight],
    ) -> Option<String> {
        let lower = task_description.to_ascii_lowercase();
        let autonomy = if lower.contains("verify") || lower.contains("review") {
            AutonomyLevel::Propose
        } else if lower.contains("apply")
            || lower.contains("implement")
            || lower.contains("fix")
            || lower.contains("write")
        {
            AutonomyLevel::Act
        } else {
            AutonomyLevel::Propose
        };
        let decision = route(
            &self.catalog,
            &RouteInput {
                provider: String::new(),
                model: String::new(),
                autonomy,
                max_tool_rounds: 16,
                session_cap: None,
                slot_override: None,
            },
        );
        Some(decision.profile_id)
    }

    pub fn route_turn(
        &self,
        provider: &str,
        model: &str,
        autonomy: AutonomyLevel,
        max_tool_rounds: usize,
        session_cap: Option<Money>,
    ) -> RouteDecision {
        route(
            &self.catalog,
            &RouteInput {
                provider: provider.into(),
                model: model.into(),
                autonomy,
                max_tool_rounds,
                session_cap,
                slot_override: None,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggest_no_longer_returns_none() {
        let selector = ModelSelector::new();
        let id = selector
            .suggest_model("implement lease conflict UX", &[])
            .expect("suggestion");
        assert_eq!(id, "worker-strong");
    }
}
