pub struct AnomalyDetector;

impl Default for AnomalyDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl AnomalyDetector {
    pub fn new() -> Self {
        Self
    }

    pub fn check_cost_spike(&self, _user_cost: f64, _baseline: f64) -> bool {
        _user_cost > _baseline * 2.0
    }

    pub fn check_quality_drop(&self, _accept_rate: f32, _baseline: f32) -> bool {
        _accept_rate < _baseline * 0.85
    }
}
