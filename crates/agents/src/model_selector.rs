use std::collections::HashMap;

pub struct TaskModelInsight {
    pub model: String,
    pub accept_rate: f32,
    pub avg_tokens: u64,
    pub best_for: Vec<String>,
    pub worst_for: Vec<String>,
}

pub struct ModelSelector;

impl ModelSelector {
    pub fn new() -> Self {
        Self
    }

    pub fn suggest_model(
        &self,
        _task_description: &str,
        _history: &[TaskModelInsight],
    ) -> Option<String> {
        None
    }
}
