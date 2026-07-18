use ade_core::audit::{AuditMode, AuditRunner};
use ade_core::error::AdeError;
use ade_core::handoff::HandoffCapsule;
use ade_core::plan::PlanBuilder;
use ade_core::verify::VerifyGate;
use ade_workflow::verify::VerifyRunner;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

/// Minimal ADE MCP server over stdio JSON-RPC (initialize / tools/list / tools/call).
pub struct AdeMcpServer {
    root: PathBuf,
}

impl AdeMcpServer {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn serve_stdio(&self) -> Result<(), AdeError> {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        let reader = BufReader::new(stdin.lock());
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let request: Value = serde_json::from_str(&line)?;
            if let Some(response) = self.handle(&request)? {
                writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
                stdout.flush()?;
            }
        }
        Ok(())
    }

    fn handle(&self, request: &Value) -> Result<Option<Value>, AdeError> {
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let id = request.get("id").cloned();
        let result = match method {
            "initialize" => json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "ade", "version": "0.1.0" }
            }),
            "notifications/initialized" | "initialized" => return Ok(None),
            "tools/list" => json!({ "tools": self.tools() }),
            "tools/call" => {
                let name = request
                    .pointer("/params/name")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                self.call_tool(name)?
            }
            "ping" => json!({}),
            _ => {
                if id.is_none() {
                    return Ok(None);
                }
                return Ok(Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": format!("method not found: {method}") }
                })));
            }
        };
        if id.is_none() {
            return Ok(None);
        }
        Ok(Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result
        })))
    }

    fn tools(&self) -> Vec<Value> {
        vec![
            tool_def(
                "ade_audit_status",
                "Run a read-only AUDIT and return score + blockers",
            ),
            tool_def(
                "ade_plan_summary",
                "Build a PLAN from the latest AUDIT without mutating the workspace",
            ),
            tool_def(
                "ade_verify_g0",
                "Run verify gate G0 (golden path probe) and return the result",
            ),
            tool_def(
                "ade_handoff_latest",
                "Return the budgeted latest ade.handoff/v1 summary",
            ),
        ]
    }

    fn call_tool(&self, name: &str) -> Result<Value, AdeError> {
        let text = match name {
            "ade_audit_status" => {
                let report = AuditRunner::new(&self.root).run(AuditMode::EvaluateExisting);
                serde_json::to_string_pretty(&json!({
                    "score": report.score,
                    "score_max": report.score_max,
                    "blockers": report.blockers,
                    "mode": report.mode,
                }))?
            }
            "ade_plan_summary" => {
                let audit = AuditRunner::new(&self.root).run(AuditMode::EvaluateExisting);
                let plan = PlanBuilder::new().build(&audit);
                serde_json::to_string_pretty(&json!({
                    "phases": plan.phases.iter().map(|phase| json!({
                        "id": phase.id,
                        "title": phase.title,
                        "owned_paths": phase.owned_paths,
                    })).collect::<Vec<_>>(),
                }))?
            }
            "ade_verify_g0" => {
                let result = VerifyRunner::with_root(&self.root).run_gate_sync(VerifyGate::G0);
                serde_json::to_string_pretty(&result)?
            }
            "ade_handoff_latest" => crate::handoff::HandoffManager::new(&self.root)
                .load_latest()
                .map(|capsule: HandoffCapsule| capsule.prompt_summary(1_500))
                .unwrap_or_else(|_| "no handoff capsule yet".into()),
            other => {
                return Ok(json!({
                    "content": [{ "type": "text", "text": format!("unknown tool '{other}'") }],
                    "isError": true
                }))
            }
        };
        Ok(json!({
            "content": [{ "type": "text", "text": text }],
            "isError": false
        }))
    }
}

fn tool_def(name: &str, description: &str) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
    })
}
