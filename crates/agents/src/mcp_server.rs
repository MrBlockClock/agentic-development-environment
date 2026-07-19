use ade_core::audit::{AuditMode, AuditRunner};
use ade_core::error::AdeError;
use ade_core::handoff::HandoffCapsule;
use ade_core::plan::PlanBuilder;
use ade_core::verify::VerifyGate;
use ade_workflow::parallel::{LeaseManager, LeaseMode};
use ade_workflow::tasks::TaskCoordinator;
use ade_workflow::verify::VerifyRunner;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Arc;

/// Minimal ADE MCP server over stdio JSON-RPC (initialize / tools/list / tools/call).
pub struct AdeMcpServer {
    root: PathBuf,
    auth_token: Option<Arc<str>>,
}

impl AdeMcpServer {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            auth_token: None,
        }
    }

    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        let token = token.into();
        if !token.trim().is_empty() {
            self.auth_token = Some(Arc::from(token));
        }
        self
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
        if matches!(method, "tools/list" | "tools/call") && !self.authorized(request) {
            if id.is_none() {
                return Ok(None);
            }
            return Ok(Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32001,
                    "message": "valid ADE MCP token required"
                }
            })));
        }
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
                let arguments = request
                    .pointer("/params/arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                self.call_tool(name, &arguments)?
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

    fn authorized(&self, request: &Value) -> bool {
        let Some(expected) = self.auth_token.as_deref() else {
            return true;
        };
        let supplied = request
            .pointer("/params/_meta/adeToken")
            .and_then(Value::as_str);
        supplied.is_some_and(|token| constant_time_eq(token.as_bytes(), expected.as_bytes()))
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
            tool_def(
                "ade_lease_list",
                "List active path leases without mutating ownership",
            ),
            tool_def(
                "ade_task_list",
                "List coordinated tasks and their dependency/claim status without mutation",
            ),
            tool_def_with_schema(
                "ade_lease_acquire",
                "Acquire a durable path lease for an agent (mutating; requires approve=true)",
                json!({
                    "type": "object",
                    "properties": {
                        "agent_id": { "type": "string", "description": "UUID of the requesting agent" },
                        "path": { "type": "string", "description": "Relative workspace path to lease" },
                        "mode": { "type": "string", "enum": ["observe", "cooperative", "strong", "exclusive"] },
                        "ttl_secs": { "type": "integer", "minimum": 1 },
                        "approve": { "type": "boolean", "description": "Explicit human approval for this ownership change" }
                    },
                    "required": ["agent_id", "path", "approve"],
                    "additionalProperties": false
                }),
            ),
            tool_def_with_schema(
                "ade_lease_renew",
                "Renew (heartbeat) an active lease held by the same agent (mutating; requires approve=true)",
                json!({
                    "type": "object",
                    "properties": {
                        "agent_id": { "type": "string", "description": "UUID of the lease holder" },
                        "lease_id": { "type": "string" },
                        "ttl_secs": { "type": "integer", "minimum": 1 },
                        "approve": { "type": "boolean", "description": "Explicit human approval for this ownership change" }
                    },
                    "required": ["agent_id", "lease_id", "approve"],
                    "additionalProperties": false
                }),
            ),
            tool_def_with_schema(
                "ade_lease_release",
                "Release a lease held by the same agent (mutating; requires approve=true)",
                json!({
                    "type": "object",
                    "properties": {
                        "agent_id": { "type": "string", "description": "UUID of the lease holder" },
                        "lease_id": { "type": "string" },
                        "approve": { "type": "boolean", "description": "Explicit human approval for this ownership change" }
                    },
                    "required": ["agent_id", "lease_id", "approve"],
                    "additionalProperties": false
                }),
            ),
        ]
    }

    fn call_tool(&self, name: &str, arguments: &Value) -> Result<Value, AdeError> {
        if matches!(
            name,
            "ade_lease_acquire" | "ade_lease_renew" | "ade_lease_release"
        ) {
            return self.call_mutating_tool(name, arguments);
        }
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
            "ade_lease_list" => {
                let leases = LeaseManager::new(&self.root).list()?;
                serde_json::to_string_pretty(&leases)?
            }
            "ade_task_list" => {
                let tasks = TaskCoordinator::new(&self.root).list()?;
                serde_json::to_string_pretty(&tasks)?
            }
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

    /// Lease mutations require an explicit `approve: true` argument and are
    /// always bound to the supplied agent identity, mirroring the CLI gates.
    fn call_mutating_tool(&self, name: &str, arguments: &Value) -> Result<Value, AdeError> {
        if arguments.get("approve").and_then(Value::as_bool) != Some(true) {
            return Ok(tool_error(format!(
                "'{name}' mutates ownership; call again with approve=true"
            )));
        }
        let Some(agent_id) = arguments
            .get("agent_id")
            .and_then(Value::as_str)
            .and_then(|raw| uuid::Uuid::parse_str(raw).ok())
        else {
            return Ok(tool_error("'agent_id' must be a valid UUID".into()));
        };
        let ttl_secs = arguments
            .get("ttl_secs")
            .and_then(Value::as_i64)
            .unwrap_or(28_800);
        let manager = LeaseManager::new(&self.root);

        let outcome = match name {
            "ade_lease_acquire" => {
                let Some(path) = arguments.get("path").and_then(Value::as_str) else {
                    return Ok(tool_error("'path' is required".into()));
                };
                let mode = arguments
                    .get("mode")
                    .and_then(Value::as_str)
                    .unwrap_or("strong");
                LeaseMode::parse(mode).and_then(|mode| {
                    manager.acquire(agent_id, path, mode, chrono::Duration::seconds(ttl_secs))
                })
            }
            "ade_lease_renew" => {
                let Some(lease_id) = arguments.get("lease_id").and_then(Value::as_str) else {
                    return Ok(tool_error("'lease_id' is required".into()));
                };
                manager.renew(agent_id, lease_id, chrono::Duration::seconds(ttl_secs))
            }
            "ade_lease_release" => {
                let Some(lease_id) = arguments.get("lease_id").and_then(Value::as_str) else {
                    return Ok(tool_error("'lease_id' is required".into()));
                };
                let held = manager
                    .list()?
                    .into_iter()
                    .find(|lease| lease.id == lease_id);
                match held {
                    Some(lease) if lease.agent_id == agent_id => {
                        manager.release(lease_id)?;
                        return Ok(json!({
                            "content": [{ "type": "text", "text": format!("released lease {lease_id}") }],
                            "isError": false
                        }));
                    }
                    Some(lease) => Err(AdeError::Authorization(format!(
                        "lease '{lease_id}' is held by {}, not {agent_id}",
                        lease.agent_id
                    ))),
                    None => Err(AdeError::Other(format!("lease '{lease_id}' is not active"))),
                }
            }
            _ => unreachable!("guarded by caller"),
        };

        match outcome {
            Ok(lease) => Ok(json!({
                "content": [{ "type": "text", "text": serde_json::to_string_pretty(&lease)? }],
                "isError": false
            })),
            Err(error) => Ok(tool_error(error.to_string())),
        }
    }
}

fn tool_error(message: String) -> Value {
    json!({
        "content": [{ "type": "text", "text": message }],
        "isError": true
    })
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (&left, &right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

fn tool_def(name: &str, description: &str) -> Value {
    tool_def_with_schema(
        name,
        description,
        json!({ "type": "object", "properties": {}, "additionalProperties": false }),
    )
}

fn tool_def_with_schema(name: &str, description: &str, schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": schema
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ade_workflow::parallel::{LeaseManager, LeaseMode};
    use chrono::Duration;
    use uuid::Uuid;

    #[test]
    fn exposes_active_leases_as_read_only_tool() {
        let root = std::env::temp_dir().join(format!("ade-mcp-leases-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        LeaseManager::new(&root)
            .acquire(
                Uuid::new_v4(),
                "src/feature",
                LeaseMode::Strong,
                Duration::minutes(5),
            )
            .unwrap();
        let server = AdeMcpServer::new(&root);
        assert!(server
            .tools()
            .iter()
            .any(|tool| tool["name"] == "ade_lease_list"));
        assert!(server
            .tools()
            .iter()
            .any(|tool| tool["name"] == "ade_task_list"));
        let result = server.call_tool("ade_lease_list", &json!({})).unwrap();
        assert!(result.to_string().contains("src/feature"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn lease_mutations_require_approval_and_holder_identity() {
        let root = std::env::temp_dir().join(format!("ade-mcp-mutate-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let server = AdeMcpServer::new(&root);
        let holder = Uuid::new_v4();

        // Refused without approve=true.
        let refused = server
            .call_tool(
                "ade_lease_acquire",
                &json!({ "agent_id": holder.to_string(), "path": "src/api" }),
            )
            .unwrap();
        assert_eq!(refused["isError"], true);

        // Acquire with approval succeeds.
        let acquired = server
            .call_tool(
                "ade_lease_acquire",
                &json!({
                    "agent_id": holder.to_string(),
                    "path": "src/api",
                    "mode": "strong",
                    "ttl_secs": 300,
                    "approve": true
                }),
            )
            .unwrap();
        assert_eq!(acquired["isError"], false);
        let lease: ade_workflow::parallel::PathLease =
            serde_json::from_str(acquired["content"][0]["text"].as_str().unwrap()).unwrap();

        // Renew works for the holder, fails for a stranger.
        let renewed = server
            .call_tool(
                "ade_lease_renew",
                &json!({
                    "agent_id": holder.to_string(),
                    "lease_id": lease.id,
                    "ttl_secs": 600,
                    "approve": true
                }),
            )
            .unwrap();
        assert_eq!(renewed["isError"], false);
        let stranger_release = server
            .call_tool(
                "ade_lease_release",
                &json!({
                    "agent_id": Uuid::new_v4().to_string(),
                    "lease_id": lease.id,
                    "approve": true
                }),
            )
            .unwrap();
        assert_eq!(stranger_release["isError"], true);

        // Holder can release.
        let released = server
            .call_tool(
                "ade_lease_release",
                &json!({
                    "agent_id": holder.to_string(),
                    "lease_id": lease.id,
                    "approve": true
                }),
            )
            .unwrap();
        assert_eq!(released["isError"], false);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn token_protects_tool_discovery_and_calls() {
        let server = AdeMcpServer::new(".").with_auth_token("test-token");
        let denied = server
            .handle(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/list",
                "params": {}
            }))
            .unwrap()
            .unwrap();
        assert_eq!(denied["error"]["code"], -32001);

        let allowed = server
            .handle(&json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list",
                "params": { "_meta": { "adeToken": "test-token" } }
            }))
            .unwrap()
            .unwrap();
        assert!(allowed["result"]["tools"].is_array());
    }
}
