use crate::runtime::{run_until_signal, ServiceConfig};
use ade_core::error::AdeError;
use daemon_kit::{Daemon, DaemonConfig, Result as DaemonResult};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

pub struct AdeDaemon {
    inner: Daemon,
    workspace_root: PathBuf,
    bind: SocketAddr,
}

impl AdeDaemon {
    /// Configure an OS-managed service that runs `ade daemon run` against a
    /// pinned workspace root. Workspace-local `DaemonLifecycle` remains the
    /// non-elevated path; this wraps daemon-kit install/uninstall only.
    pub fn for_workspace(workspace_root: impl Into<PathBuf>, bind: SocketAddr) -> Self {
        let workspace_root = workspace_root.into();
        let root = workspace_root
            .canonicalize()
            .unwrap_or_else(|_| workspace_root.clone());
        let service_dir = root.join(".ade").join("daemon").join("service");
        let config = DaemonConfig::new("ade-daemon")
            .description("ADE background coordination service")
            .pid_dir(&service_dir)
            .log_file(service_dir.join("service.log"))
            .service_args(vec![
                "daemon".into(),
                "run".into(),
                "--bind".into(),
                bind.to_string(),
                "--root".into(),
                root.display().to_string(),
            ]);
        Self {
            inner: Daemon::new(config),
            workspace_root: root,
            bind,
        }
    }

    pub fn new() -> Self {
        Self::for_workspace(
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            SocketAddr::from(([127, 0, 0, 1], 3210)),
        )
    }

    pub fn name(&self) -> &'static str {
        "ade-daemon"
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn bind(&self) -> SocketAddr {
        self.bind
    }

    pub fn install_service(&self) -> DaemonResult<()> {
        let _ = std::fs::create_dir_all(
            self.workspace_root
                .join(".ade")
                .join("daemon")
                .join("service"),
        );
        self.inner.install_service()
    }

    pub fn uninstall_service(&self) -> DaemonResult<()> {
        self.inner.uninstall_service()
    }

    pub fn is_service_installed(&self) -> bool {
        self.inner.is_service_installed()
    }

    pub fn start(&self, foreground: bool) -> DaemonResult<()> {
        self.inner.start(foreground, || {
            // OS service entrypoints should invoke `ade daemon run` via the
            // installed service args. This sync start path is retained for
            // daemon-kit compatibility and is not used by the CLI.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configures_workspace_scoped_service() {
        let root = std::env::temp_dir().join(format!("ade-daemon-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let daemon = AdeDaemon::for_workspace(&root, SocketAddr::from(([127, 0, 0, 1], 3210)));
        assert_eq!(daemon.name(), "ade-daemon");
        assert_eq!(daemon.bind().port(), 3210);
        assert!(daemon.workspace_root().ends_with(root.file_name().unwrap()));
        let _ = std::fs::remove_dir_all(root);
    }
}
