//! User-level daemon lifecycle: start a detached ADE API process, report its
//! status, and stop it — all tracked under `.ade/daemon/` in the workspace.
//!
//! This deliberately avoids OS service managers (Windows Service, systemd),
//! which need elevated installs. `daemon_kit::PidFile` supplies cross-platform
//! pid liveness with stale-file cleanup.

use ade_core::error::AdeError;
use daemon_kit::PidFile;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const STATE_SCHEMA: &str = "ade.daemon-state/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonState {
    pub schema: String,
    pub pid: u32,
    pub bind: String,
    pub started_at: String,
    pub auth_required: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DaemonStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub bind: Option<String>,
    pub started_at: Option<String>,
    pub auth_required: Option<bool>,
    pub log_path: String,
}

pub struct DaemonLifecycle {
    workspace_root: PathBuf,
    directory: PathBuf,
}

impl DaemonLifecycle {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        let workspace_root = workspace_root.into();
        let directory = workspace_root.join(".ade").join("daemon");
        Self {
            workspace_root,
            directory,
        }
    }

    pub fn log_path(&self) -> PathBuf {
        self.directory.join("daemon.log")
    }

    fn pid_file(&self) -> PidFile {
        PidFile::new(self.directory.join("ade-daemon.pid"))
    }

    fn state_path(&self) -> PathBuf {
        self.directory.join("state.json")
    }

    pub fn status(&self) -> DaemonStatus {
        let pid = self.pid_file().alive_pid();
        let state = pid.and_then(|_| self.load_state());
        DaemonStatus {
            running: pid.is_some(),
            pid,
            bind: state.as_ref().map(|state| state.bind.clone()),
            started_at: state.as_ref().map(|state| state.started_at.clone()),
            auth_required: state.as_ref().map(|state| state.auth_required),
            log_path: self.log_path().display().to_string(),
        }
    }

    /// Spawn a detached `ade daemon run` child bound to `bind`. The optional
    /// token is passed via the child's environment, never argv.
    pub fn start_detached(
        &self,
        bind: SocketAddr,
        auth_token: Option<&str>,
    ) -> Result<DaemonState, AdeError> {
        if let Some(pid) = self.pid_file().alive_pid() {
            return Err(AdeError::Other(format!(
                "ade daemon already running (pid {pid}); run `ade daemon stop` first"
            )));
        }
        std::fs::create_dir_all(&self.directory)?;
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log_path())?;
        let exe = std::env::current_exe()
            .map_err(|error| AdeError::Other(format!("cannot locate ade executable: {error}")))?;

        let mut command = Command::new(exe);
        command
            .arg("daemon")
            .arg("run")
            .arg("--bind")
            .arg(bind.to_string())
            .arg("--root")
            .arg(self.workspace_root.as_os_str())
            .current_dir(&self.workspace_root)
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log);
        match auth_token {
            Some(token) => command.env("ADE_API_TOKEN", token),
            None => command.env_remove("ADE_API_TOKEN"),
        };
        detach(&mut command);

        let child = command
            .spawn()
            .map_err(|error| AdeError::Other(format!("failed to spawn ade daemon: {error}")))?;
        let state = DaemonState {
            schema: STATE_SCHEMA.into(),
            pid: child.id(),
            bind: bind.to_string(),
            started_at: chrono_now(),
            auth_required: auth_token.is_some(),
        };
        // The child writes the pidfile itself once it boots; seed it here so
        // status/stop work even during startup.
        std::fs::write(self.pid_file().path(), state.pid.to_string())?;
        std::fs::write(self.state_path(), serde_json::to_vec_pretty(&state)?)?;
        Ok(state)
    }

    pub fn stop(&self) -> Result<u32, AdeError> {
        let Some(pid) = self.pid_file().alive_pid() else {
            return Err(AdeError::Other("ade daemon is not running".into()));
        };
        kill_process(pid)?;
        self.pid_file().remove();
        let _ = std::fs::remove_file(self.state_path());
        Ok(pid)
    }

    /// Record the current process as the running daemon (used by `daemon run`).
    pub fn mark_running(&self, bind: SocketAddr, auth_required: bool) -> Result<(), AdeError> {
        std::fs::create_dir_all(&self.directory)?;
        self.pid_file()
            .write()
            .map_err(|error| AdeError::Other(format!("failed to write pid file: {error}")))?;
        let state = DaemonState {
            schema: STATE_SCHEMA.into(),
            pid: std::process::id(),
            bind: bind.to_string(),
            started_at: chrono_now(),
            auth_required,
        };
        std::fs::write(self.state_path(), serde_json::to_vec_pretty(&state)?)?;
        Ok(())
    }

    /// Remove pid/state markers on clean shutdown (used by `daemon run`).
    pub fn mark_stopped(&self) {
        self.pid_file().remove();
        let _ = std::fs::remove_file(self.state_path());
    }

    fn load_state(&self) -> Option<DaemonState> {
        let payload = std::fs::read_to_string(self.state_path()).ok()?;
        let state: DaemonState = serde_json::from_str(&payload).ok()?;
        (state.schema == STATE_SCHEMA).then_some(state)
    }
}

fn chrono_now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

#[cfg(not(windows))]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    // A new process group detaches the child from the parent's terminal
    // signals; a full double-fork is unnecessary here.
    command.process_group(0);
}

#[cfg(windows)]
fn kill_process(pid: u32) -> Result<(), AdeError> {
    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .output()
        .map_err(|error| AdeError::Other(format!("taskkill failed to launch: {error}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(AdeError::Other(format!(
            "taskkill failed for pid {pid}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

#[cfg(not(windows))]
fn kill_process(pid: u32) -> Result<(), AdeError> {
    let status = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .map_err(|error| AdeError::Other(format!("kill failed to launch: {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(AdeError::Other(format!("kill -TERM {pid} failed")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reports_not_running_without_markers() {
        let root = std::env::temp_dir().join(format!("ade-daemon-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let lifecycle = DaemonLifecycle::new(&root);
        let status = lifecycle.status();
        assert!(!status.running);
        assert!(status.pid.is_none());
        assert!(lifecycle.stop().is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn mark_running_and_stopped_roundtrip() {
        let root = std::env::temp_dir().join(format!("ade-daemon-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let lifecycle = DaemonLifecycle::new(&root);
        lifecycle
            .mark_running(SocketAddr::from(([127, 0, 0, 1], 3210)), true)
            .unwrap();
        let status = lifecycle.status();
        assert!(status.running);
        assert_eq!(status.pid, Some(std::process::id()));
        assert_eq!(status.bind.as_deref(), Some("127.0.0.1:3210"));
        assert_eq!(status.auth_required, Some(true));
        lifecycle.mark_stopped();
        assert!(!lifecycle.status().running);
        let _ = std::fs::remove_dir_all(root);
    }
}
