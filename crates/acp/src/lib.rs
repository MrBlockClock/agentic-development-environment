//! ADE Agent Client Protocol soft shell (Z1).
//!
//! Speaks ACP JSON-RPC over stdio so hosts like Zed can run ADE as a BYO agent.
//! Maps Suggest / Apply / Automate modes, surfaces leases + verify guidance, and
//! streams session updates. Full live-LLM turns remain on Desktop; this is the
//! coding-eyes bridge (DEC-A-014).
//!
//! See `hosts/zed/README.md` and `docs/decisions/DEC-A-010-multi-host-agent-os.md`.

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub const SCHEMA: &str = "ade.acp/v1";
pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct AcpScaffoldInfo {
    pub schema: &'static str,
    pub agent_name: &'static str,
    pub status: &'static str,
    pub next: &'static str,
    pub protocol_version: u16,
}

pub fn scaffold_info() -> AcpScaffoldInfo {
    AcpScaffoldInfo {
        schema: SCHEMA,
        agent_name: "ADE",
        status: "soft_shell",
        next: "Zed Agent Panel · modes Suggest/Apply/Automate · Desktop for full harness turns",
        protocol_version: PROTOCOL_VERSION,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdeMode {
    Suggest,
    Apply,
    Automate,
}

impl AdeMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Suggest => "suggest",
            Self::Apply => "apply",
            Self::Automate => "automate",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Suggest => "Suggest (Planner)",
            Self::Apply => "Apply (Worker)",
            Self::Automate => "Automate (Worker+Verify)",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "suggest" | "propose" | "planner" | "ask" => Some(Self::Suggest),
            "apply" | "act" | "worker" | "code" => Some(Self::Apply),
            "automate" | "auto" => Some(Self::Automate),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
struct SessionState {
    #[allow(dead_code)]
    id: String,
    cwd: PathBuf,
    mode: AdeMode,
    cancelled: bool,
}

struct AgentState {
    sessions: HashMap<String, SessionState>,
    initialized: bool,
}

/// Run the ACP agent on stdin/stdout.
///
/// `--probe` prints soft-shell info JSON and exits. Otherwise runs the JSON-RPC loop.
pub async fn run_acp_agent(probe_only: bool) -> Result<(), AdeAcpError> {
    let info = scaffold_info();
    if probe_only {
        let line =
            serde_json::to_string(&info).map_err(|error| AdeAcpError::Json(error.to_string()))?;
        println!("{line}");
        return Ok(());
    }

    tracing::info!(target: "ade_acp", "ADE ACP soft shell starting (protocol v{PROTOCOL_VERSION})");
    run_stdio_loop()
}

fn run_stdio_loop() -> Result<(), AdeAcpError> {
    let state = Arc::new(Mutex::new(AgentState {
        sessions: HashMap::new(),
        initialized: false,
    }));
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader
            .read_line(&mut line)
            .map_err(|e| AdeAcpError::Io(e.to_string()))?;
        if n == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(err) => {
                write_json(&json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("parse error: {err}") }
                }))?;
                continue;
            }
        };

        // Notification (no id)
        if msg.get("id").is_none() {
            if let Some(method) = msg.get("method").and_then(Value::as_str) {
                handle_notification(state.clone(), method, msg.get("params"))?;
            }
            continue;
        }

        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        let method = msg
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let params = msg.get("params").cloned().unwrap_or(Value::Null);

        match dispatch_request(state.clone(), &method, params) {
            Ok(result) => write_json(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result
            }))?,
            Err(err) => write_json(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": err
            }))?,
        }
    }
    Ok(())
}

fn handle_notification(
    state: Arc<Mutex<AgentState>>,
    method: &str,
    params: Option<&Value>,
) -> Result<(), AdeAcpError> {
    if method == "session/cancel" {
        let sid = params
            .and_then(|p| {
                p.get("sessionId")
                    .or_else(|| p.get("session_id"))
                    .and_then(Value::as_str)
            })
            .unwrap_or("");
        if let Ok(mut st) = state.lock() {
            if let Some(session) = st.sessions.get_mut(sid) {
                session.cancelled = true;
            }
        }
    }
    Ok(())
}

fn dispatch_request(
    state: Arc<Mutex<AgentState>>,
    method: &str,
    params: Value,
) -> Result<Value, Value> {
    match method {
        "initialize" => Ok(handle_initialize(state, params)),
        "authenticate" => Ok(json!({})),
        "session/new" => handle_session_new(state, params),
        "session/set_mode" => handle_set_mode(state, params),
        "session/prompt" => handle_prompt(state, params),
        "shutdown" | "exit" => Ok(json!({})),
        other => Err(json!({
            "code": -32601,
            "message": format!("method not found: {other}")
        })),
    }
}

fn handle_initialize(state: Arc<Mutex<AgentState>>, _params: Value) -> Value {
    if let Ok(mut st) = state.lock() {
        st.initialized = true;
    }
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "agentCapabilities": {
            "loadSession": false,
            "promptCapabilities": {
                "image": false,
                "audio": false,
                "embeddedContext": true
            }
        },
        "agentInfo": {
            "name": "ADE",
            "title": "ADE Harness",
            "version": env!("CARGO_PKG_VERSION")
        },
        "authMethods": []
    })
}

fn handle_session_new(state: Arc<Mutex<AgentState>>, params: Value) -> Result<Value, Value> {
    let cwd = params
        .get("cwd")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let id = Uuid::new_v4().to_string();
    if let Ok(mut st) = state.lock() {
        st.sessions.insert(
            id.clone(),
            SessionState {
                id: id.clone(),
                cwd,
                mode: AdeMode::Suggest,
                cancelled: false,
            },
        );
    }
    Ok(json!({
        "sessionId": id,
        "modes": {
            "currentModeId": "suggest",
            "availableModes": [
                { "id": "suggest", "name": "Suggest", "description": "Planner / inspect-only (Propose)" },
                { "id": "apply", "name": "Apply", "description": "Worker under leases (Act)" },
                { "id": "automate", "name": "Automate", "description": "Worker + required verify" }
            ]
        }
    }))
}

fn handle_set_mode(state: Arc<Mutex<AgentState>>, params: Value) -> Result<Value, Value> {
    let sid = params
        .get("sessionId")
        .or_else(|| params.get("session_id"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let mode_id = params
        .get("modeId")
        .or_else(|| params.get("mode_id"))
        .and_then(Value::as_str)
        .unwrap_or("suggest");
    let mode = AdeMode::parse(mode_id)
        .ok_or_else(|| json!({ "code": -32602, "message": format!("unknown mode '{mode_id}'") }))?;
    let mut st = state
        .lock()
        .map_err(|_| json!({ "code": -32603, "message": "lock poisoned" }))?;
    let session = st
        .sessions
        .get_mut(sid)
        .ok_or_else(|| json!({ "code": -32002, "message": format!("unknown session {sid}") }))?;
    session.mode = mode;
    Ok(json!({}))
}

fn handle_prompt(state: Arc<Mutex<AgentState>>, params: Value) -> Result<Value, Value> {
    let sid = params
        .get("sessionId")
        .or_else(|| params.get("session_id"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let prompt_text = extract_prompt_text(&params);
    let (mode, cwd, cancelled) = {
        let st = state
            .lock()
            .map_err(|_| json!({ "code": -32603, "message": "lock poisoned" }))?;
        let session = st.sessions.get(&sid).ok_or_else(
            || json!({ "code": -32002, "message": format!("unknown session {sid}") }),
        )?;
        (session.mode, session.cwd.clone(), session.cancelled)
    };
    if cancelled {
        if let Ok(mut st) = state.lock() {
            if let Some(s) = st.sessions.get_mut(&sid) {
                s.cancelled = false;
            }
        }
        return Ok(json!({ "stopReason": "cancelled" }));
    }

    let reply = build_soft_shell_reply(mode, &cwd, &prompt_text);
    emit_agent_message(&sid, &reply)?;
    Ok(json!({ "stopReason": "end_turn" }))
}

fn extract_prompt_text(params: &Value) -> String {
    let Some(blocks) = params.get("prompt").and_then(Value::as_array) else {
        return params
            .get("prompt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
    };
    let mut parts = Vec::new();
    for block in blocks {
        if let Some(t) = block.get("text").and_then(Value::as_str) {
            parts.push(t.to_string());
            continue;
        }
        if block.get("type").and_then(Value::as_str) == Some("text") {
            if let Some(t) = block.get("text").and_then(Value::as_str) {
                parts.push(t.to_string());
            }
        }
    }
    parts.join("\n")
}

fn build_soft_shell_reply(mode: AdeMode, cwd: &Path, prompt: &str) -> String {
    let lower = prompt.to_ascii_lowercase();
    let mut lines = vec![
        format!(
            "**ADE soft shell** · mode **{}** · cwd `{}`",
            mode.label(),
            cwd.display()
        ),
        String::new(),
        "Harness map:".into(),
        format!(
            "- Autonomy: {} (Planner≠Worker; leases on Apply)",
            mode.as_str()
        ),
        "- Contract (G1): Act tools need eng-goal AC + out-of-scope + verify (or waive)".into(),
        "- Risk (G2): publish/infra/migrate/secrets need confirm".into(),
        "- Channel: tool-blob mask · ~70% boundary capsule · Continuity".into(),
        String::new(),
    ];

    if lower.contains("verify") || lower.contains("/verify") {
        lines.push(
            "Verify reminder: run `ade verify --gate G3 --through` (or Desktop Automate).".into(),
        );
        lines.push(String::new());
    }
    if lower.contains("lease") || mode != AdeMode::Suggest {
        lines.push(
            "Leases: Apply/Automate claim write leases under `.ade/leases/`. Conflict → Wait / Isolate / Rotate / Suggest."
                .into(),
        );
        lines.push(String::new());
    }
    if mode == AdeMode::Suggest {
        lines.push(
            "Suggest is inspect-only. Switch mode to **Apply** for writes (or use ADE Desktop control plane for full turns)."
                .into(),
        );
    } else if mode == AdeMode::Automate {
        lines.push(
            "Automate requires verify-on-complete. Prefer Desktop for live provider turns with SpendGuard."
                .into(),
        );
    } else {
        lines.push(
            "Apply mode selected. Full mutating turns with providers run best from ADE Desktop; this ACP shell is coding eyes + mode/lease guidance."
                .into(),
        );
    }

    if !prompt.trim().is_empty() {
        lines.push(String::new());
        lines.push(format!("You said: {}", truncate(prompt.trim(), 400)));
    }
    lines.push(String::new());
    lines.push(
        "_Open ADE Desktop for live agent turns · `ade eval --gold` for harness races · `hosts/zed/README.md`._"
            .into(),
    );
    lines.join("\n")
}

fn truncate(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    let head: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{head}…")
}

fn emit_agent_message(session_id: &str, text: &str) -> Result<(), Value> {
    // session/update notification — AgentMessageChunk
    write_json(&json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {
            "sessionId": session_id,
            "update": {
                "sessionUpdate": "agent_message_chunk",
                "content": {
                    "type": "text",
                    "text": text
                }
            }
        }
    }))
    .map_err(|e| json!({ "code": -32603, "message": e.to_string() }))
}

fn write_json(value: &Value) -> Result<(), AdeAcpError> {
    let mut stdout = std::io::stdout().lock();
    let line =
        serde_json::to_string(value).map_err(|error| AdeAcpError::Json(error.to_string()))?;
    writeln!(stdout, "{line}").map_err(|e| AdeAcpError::Io(e.to_string()))?;
    stdout.flush().map_err(|e| AdeAcpError::Io(e.to_string()))?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum AdeAcpError {
    #[error("JSON error: {0}")]
    Json(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("ACP stdio agent not implemented yet (scaffold)")]
    NotImplemented,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_info_has_schema() {
        let info = scaffold_info();
        assert_eq!(info.schema, "ade.acp/v1");
        assert_eq!(info.agent_name, "ADE");
        assert_eq!(info.status, "soft_shell");
        assert_eq!(info.protocol_version, 1);
    }

    #[test]
    fn modes_parse() {
        assert_eq!(AdeMode::parse("suggest"), Some(AdeMode::Suggest));
        assert_eq!(AdeMode::parse("apply"), Some(AdeMode::Apply));
        assert_eq!(AdeMode::parse("automate"), Some(AdeMode::Automate));
    }

    #[test]
    fn initialize_and_session_roundtrip() {
        let state = Arc::new(Mutex::new(AgentState {
            sessions: HashMap::new(),
            initialized: false,
        }));
        let init = dispatch_request(state.clone(), "initialize", json!({})).unwrap();
        assert_eq!(init["protocolVersion"], 1);
        assert_eq!(init["agentInfo"]["name"], "ADE");

        let created =
            dispatch_request(state.clone(), "session/new", json!({ "cwd": "." })).unwrap();
        let sid = created["sessionId"].as_str().unwrap().to_string();
        assert!(!sid.is_empty());

        dispatch_request(
            state.clone(),
            "session/set_mode",
            json!({ "sessionId": sid, "modeId": "apply" }),
        )
        .unwrap();
        let st = state.lock().unwrap();
        assert_eq!(st.sessions.get(&sid).unwrap().mode, AdeMode::Apply);
    }

    #[test]
    fn extract_prompt_from_blocks() {
        let params = json!({
            "prompt": [
                { "type": "text", "text": "hello" },
                { "type": "text", "text": "world" }
            ]
        });
        assert_eq!(extract_prompt_text(&params), "hello\nworld");
    }
}
