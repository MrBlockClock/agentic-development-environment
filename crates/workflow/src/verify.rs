use std::process::Command;
use ade_core::verify::{VerifyGate, VerifyResult};

pub struct VerifyRunner;

impl VerifyRunner {
    pub fn new() -> Self {
        Self
    }

    pub async fn run_gate(&self, _gate: VerifyGate) -> VerifyResult {
        // TODO: execute verify command, capture output
        VerifyResult {
            gate: "G0".to_string(),
            command: "echo placeholder".to_string(),
            exit_code: Some(0),
            stdout: Some("ok".to_string()),
            stderr: None,
            passed: true,
        }
    }

    pub fn available_gates(&self) -> Vec<VerifyGate> {
        VerifyGate::all()
    }
}
