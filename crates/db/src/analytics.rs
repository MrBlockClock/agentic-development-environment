use ade_core::analytics::{AnalyticsEvent, ModelQualityMetrics};

pub struct AnalyticsStore;

impl AnalyticsStore {
    pub fn new() -> Self {
        Self
    }

    pub fn record_event(&self, _event: AnalyticsEvent) {
        // TODO: persist to Turso
    }

    pub fn model_quality(&self, _model: &str) -> Option<ModelQualityMetrics> {
        // TODO: aggregate from stored events
        None
    }
}
