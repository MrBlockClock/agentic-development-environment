//! Interactive PTY sessions for the Desktop Terminal (T1).

use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::ipc::Channel;
use uuid::Uuid;

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PtyEvent {
    Data { data: String },
    Exit { code: Option<u32> },
}

struct PtySession {
    writer: Mutex<Box<dyn Write + Send>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
}

#[derive(Clone, Default)]
pub struct PtyHub {
    sessions: Arc<Mutex<HashMap<String, Arc<PtySession>>>>,
}

impl PtyHub {
    pub fn spawn(
        &self,
        cwd: &Path,
        cols: u16,
        rows: u16,
        on_event: Channel<PtyEvent>,
    ) -> Result<String, String> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("open pty: {error}"))?;

        let mut cmd = default_shell_command();
        cmd.cwd(cwd);

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|error| format!("spawn shell: {error}"))?;
        let killer = child.clone_killer();

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| format!("clone reader: {error}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| format!("take writer: {error}"))?;

        let session_id = Uuid::new_v4().to_string();
        let session = Arc::new(PtySession {
            writer: Mutex::new(writer),
            master: Mutex::new(pair.master),
            killer: Mutex::new(killer),
        });

        self.sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(session_id.clone(), Arc::clone(&session));

        let sessions = Arc::clone(&self.sessions);
        let cleanup_id = session_id.clone();

        thread::Builder::new()
            .name(format!("ade-pty-read-{session_id}"))
            .spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let data = String::from_utf8_lossy(&buf[..n]).into_owned();
                            let _ = on_event.send(PtyEvent::Data { data });
                        }
                        Err(_) => break,
                    }
                }
                let code = child.wait().ok().map(|status| status.exit_code());
                let _ = on_event.send(PtyEvent::Exit { code });
                if let Ok(mut map) = sessions.lock() {
                    map.remove(&cleanup_id);
                }
            })
            .map_err(|error| format!("spawn reader thread: {error}"))?;

        Ok(session_id)
    }

    pub fn write(&self, session_id: &str, data: &str) -> Result<(), String> {
        let session = self.get(session_id)?;
        let mut writer = session
            .writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        writer
            .write_all(data.as_bytes())
            .map_err(|error| format!("pty write: {error}"))?;
        writer
            .flush()
            .map_err(|error| format!("pty flush: {error}"))?;
        Ok(())
    }

    pub fn resize(&self, session_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let session = self.get(session_id)?;
        let master = session
            .master
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        master
            .resize(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("pty resize: {error}"))?;
        Ok(())
    }

    pub fn kill(&self, session_id: &str) -> Result<(), String> {
        let session = {
            let mut map = self
                .sessions
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            map.remove(session_id)
                .ok_or_else(|| format!("unknown pty session: {session_id}"))?
        };
        let mut killer = session
            .killer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        killer
            .kill()
            .map_err(|error| format!("pty kill: {error}"))?;
        Ok(())
    }

    fn get(&self, session_id: &str) -> Result<Arc<PtySession>, String> {
        self.sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("unknown pty session: {session_id}"))
    }
}

fn default_shell_command() -> CommandBuilder {
    #[cfg(windows)]
    {
        if let Ok(shell) = std::env::var("COMSPEC") {
            if !shell.trim().is_empty() {
                return CommandBuilder::new(shell);
            }
        }
        CommandBuilder::new("powershell.exe")
    }
    #[cfg(not(windows))]
    {
        if let Ok(shell) = std::env::var("SHELL") {
            if !shell.trim().is_empty() {
                return CommandBuilder::new(shell);
            }
        }
        CommandBuilder::new("/bin/bash")
    }
}
