use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct HealthReport {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub db_connected: bool,
    pub agents_available: Vec<String>,
}

pub struct HealthChecker;

impl HealthChecker {
    pub fn new() -> Self {
        Self
    }

    pub fn check(&self) -> HealthReport {
        HealthReport {
            status: "ok".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: 0,
            db_connected: false,
            agents_available: vec![],
        }
    }
}
