use daemon_kit::Daemon;

pub struct AdeDaemon;

impl Daemon for AdeDaemon {
    fn name(&self) -> &str {
        "ade-daemon"
    }

    fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: start HTTP API, agent worker, scheduler
        Ok(())
    }

    fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}
