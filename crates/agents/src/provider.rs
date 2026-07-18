use std::collections::HashMap;

pub struct ProviderConfig {
    pub name: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub models: Vec<ModelConfig>,
}

pub struct ModelConfig {
    pub id: String,
    pub name: String,
    pub context_limit: u64,
    pub output_limit: u64,
    pub cost_per_input_mtok: f64,
    pub cost_per_output_mtok: f64,
}

pub struct ProviderManager;

impl ProviderManager {
    pub fn new() -> Self {
        Self
    }

    pub fn list_providers(&self) -> Vec<String> {
        vec![]
    }

    pub fn select_model(
        &self,
        _task_type: &str,
        _budget: Option<f64>,
    ) -> Option<String> {
        None
    }
}
