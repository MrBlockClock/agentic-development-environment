pub struct Scheduler;

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(&self) {
        // TODO: periodic tasks: analytics upload, ignore drift check, health check
    }
}
