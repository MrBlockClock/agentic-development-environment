pub struct Scheduler;

impl Scheduler {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(&self) {
        // TODO: periodic tasks: analytics upload, ignore drift check, health check
    }
}
