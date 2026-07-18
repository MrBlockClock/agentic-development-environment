use daemon_kit::{Daemon, DaemonConfig, Result};

pub struct AdeDaemon {
    inner: Daemon,
}

impl AdeDaemon {
    pub fn new() -> Self {
        let config = DaemonConfig::new("ade-daemon")
            .description("ADE background service")
            .service_args(vec!["daemon".to_string(), "--foreground".to_string()]);
        Self {
            inner: Daemon::new(config),
        }
    }

    pub fn name(&self) -> &'static str {
        "ade-daemon"
    }

    pub fn start(&self, foreground: bool) -> Result<()> {
        self.inner.start(foreground, || {
            // TODO: start HTTP API, agent worker, scheduler
            Ok(())
        })
    }

    pub fn stop(&self) -> Result<()> {
        self.inner.stop()
    }
}

impl Default for AdeDaemon {
    fn default() -> Self {
        Self::new()
    }
}
