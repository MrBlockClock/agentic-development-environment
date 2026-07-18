use crate::runtime::{run_until_signal, ServiceConfig};
use ade_core::error::AdeError;
use daemon_kit::{Daemon, DaemonConfig, Result as DaemonResult};

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

    pub fn start(&self, foreground: bool) -> DaemonResult<()> {
        self.inner.start(foreground, || {
            // TODO: start HTTP API, agent worker, scheduler
            Ok(())
        })
    }

    pub fn stop(&self) -> DaemonResult<()> {
        self.inner.stop()
    }

    /// Run the actual local API runtime in the foreground.
    ///
    /// Service installation remains delegated to daemon-kit; foreground
    /// execution is shared with the CLI and uses graceful Ctrl+C shutdown.
    pub async fn run_foreground(&self, config: ServiceConfig) -> Result<(), AdeError> {
        run_until_signal(config).await
    }
}

impl Default for AdeDaemon {
    fn default() -> Self {
        Self::new()
    }
}
