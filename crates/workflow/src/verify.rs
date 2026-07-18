use ade_core::verify::{VerifyGate, VerifyResult};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub struct VerifyRunner {
    root: PathBuf,
}

impl Default for VerifyRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl VerifyRunner {
    pub fn new() -> Self {
        Self {
            root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub async fn run_gate(&self, gate: VerifyGate) -> VerifyResult {
        match gate {
            VerifyGate::G0 => self.run_g0(),
            VerifyGate::G1 => self.run_g1(),
            VerifyGate::G2 => self.run_g2(),
            VerifyGate::G3 => self.run_g3(),
            VerifyGate::G4 => self.run_g4(),
            VerifyGate::G5 => unavailable(
                gate,
                "manual browser or hardware evidence is required for this project",
            ),
        }
    }

    pub async fn run_through(&self, max_gate: VerifyGate) -> Vec<VerifyResult> {
        let mut results = Vec::new();
        for gate in VerifyGate::all() {
            if gate_number(gate) > gate_number(max_gate) {
                break;
            }
            let result = self.run_gate(gate).await;
            let passed = result.passed;
            results.push(result);
            if !passed {
                break;
            }
        }
        results
    }

    pub fn available_gates(&self) -> Vec<VerifyGate> {
        VerifyGate::all()
    }

    fn run_g0(&self) -> VerifyResult {
        if self.root.join("Cargo.toml").is_file() {
            return self.run_commands(
                VerifyGate::G0,
                vec![CommandSpec::new("cargo", &["locate-project"])],
            );
        }
        if self.root.join("package.json").is_file() {
            return self.run_commands(
                VerifyGate::G0,
                vec![CommandSpec::new(node_command(), &["--version"])],
            );
        }
        if self.root.join("pyproject.toml").is_file() {
            return self.run_commands(
                VerifyGate::G0,
                vec![CommandSpec::new(python_command(), &["--version"])],
            );
        }
        failure(
            VerifyGate::G0,
            "probe project manifest",
            "no project manifest found (expected Cargo.toml, package.json, or pyproject.toml)",
        )
    }

    fn run_g1(&self) -> VerifyResult {
        let required = ["AGENTS.md"];
        let missing = required
            .iter()
            .filter(|path| !self.root.join(path).is_file())
            .copied()
            .collect::<Vec<_>>();
        VerifyResult {
            gate: VerifyGate::G1.id().into(),
            command: "check AGENTS.md contract".into(),
            exit_code: Some(if missing.is_empty() { 0 } else { 1 }),
            stdout: missing
                .is_empty()
                .then(|| "AGENTS.md contract present".into()),
            stderr: (!missing.is_empty()).then(|| format!("missing: {}", missing.join(", "))),
            passed: missing.is_empty(),
        }
    }

    fn run_g2(&self) -> VerifyResult {
        if self.root.join("Cargo.toml").is_file() {
            return self.run_commands(
                VerifyGate::G2,
                vec![
                    CommandSpec::new("cargo", &["fmt", "--check"]),
                    CommandSpec::new("cargo", &["clippy", "--", "-D", "warnings"]),
                ],
            );
        }
        if self.root.join("package.json").is_file() {
            return self.run_commands(
                VerifyGate::G2,
                vec![
                    CommandSpec::new(npm_command(), &["run", "format", "--if-present"]),
                    CommandSpec::new(npm_command(), &["run", "lint", "--if-present"]),
                ],
            );
        }
        unavailable(
            VerifyGate::G2,
            "no supported Rust or Node format/lint commands were detected",
        )
    }

    fn run_g3(&self) -> VerifyResult {
        if self.root.join("Cargo.toml").is_file() {
            return self.run_commands(
                VerifyGate::G3,
                vec![CommandSpec::new("cargo", &["test", "--workspace"])],
            );
        }
        if self.root.join("package.json").is_file() {
            return self.run_commands(
                VerifyGate::G3,
                vec![CommandSpec::new(npm_command(), &["test", "--", "--run"])],
            );
        }
        unavailable(VerifyGate::G3, "no supported test command was detected")
    }

    fn run_g4(&self) -> VerifyResult {
        let powershell = self.root.join("scripts").join("verify-full.ps1");
        let shell = self.root.join("scripts").join("verify-full.sh");
        if powershell.is_file() {
            return self.run_commands(
                VerifyGate::G4,
                vec![CommandSpec::new(
                    powershell_command(),
                    &["-NoProfile", "-File", &powershell.display().to_string()],
                )],
            );
        }
        if shell.is_file() {
            return self.run_commands(
                VerifyGate::G4,
                vec![CommandSpec::new(
                    "bash",
                    &[shell.display().to_string().as_str()],
                )],
            );
        }
        unavailable(
            VerifyGate::G4,
            "no scripts/verify-full.ps1 or scripts/verify-full.sh integration gate found",
        )
    }

    fn run_commands(&self, gate: VerifyGate, commands: Vec<CommandSpec>) -> VerifyResult {
        let command_label = commands
            .iter()
            .map(CommandSpec::display)
            .collect::<Vec<_>>()
            .join(" && ");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_code = Some(0);

        for spec in commands {
            match spec.run(&self.root) {
                Ok(output) => {
                    append_output(&mut stdout, &output.stdout);
                    append_output(&mut stderr, &output.stderr);
                    exit_code = output.status.code();
                    if !output.status.success() {
                        return VerifyResult {
                            gate: gate.id().into(),
                            command: command_label,
                            exit_code,
                            stdout: output_text(stdout),
                            stderr: output_text(stderr),
                            passed: false,
                        };
                    }
                }
                Err(error) => {
                    stderr.extend_from_slice(error.to_string().as_bytes());
                    return VerifyResult {
                        gate: gate.id().into(),
                        command: command_label,
                        exit_code: None,
                        stdout: output_text(stdout),
                        stderr: output_text(stderr),
                        passed: false,
                    };
                }
            }
        }

        VerifyResult {
            gate: gate.id().into(),
            command: command_label,
            exit_code,
            stdout: output_text(stdout),
            stderr: output_text(stderr),
            passed: true,
        }
    }
}

struct CommandSpec {
    program: String,
    args: Vec<String>,
}

impl CommandSpec {
    fn new(program: impl Into<String>, args: &[&str]) -> Self {
        Self {
            program: program.into(),
            args: args.iter().map(|arg| (*arg).to_string()).collect(),
        }
    }

    fn display(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn run(&self, root: &Path) -> std::io::Result<Output> {
        Command::new(&self.program)
            .args(&self.args)
            .current_dir(root)
            .output()
    }
}

fn append_output(buffer: &mut Vec<u8>, output: &[u8]) {
    if !buffer.is_empty() && !output.is_empty() {
        buffer.push(b'\n');
    }
    buffer.extend_from_slice(output);
}

fn output_text(bytes: Vec<u8>) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    let text = String::from_utf8_lossy(&bytes);
    Some(text.chars().take(64_000).collect())
}

fn failure(gate: VerifyGate, command: &str, message: &str) -> VerifyResult {
    VerifyResult {
        gate: gate.id().into(),
        command: command.into(),
        exit_code: Some(1),
        stdout: None,
        stderr: Some(message.into()),
        passed: false,
    }
}

fn unavailable(gate: VerifyGate, message: &str) -> VerifyResult {
    VerifyResult {
        gate: gate.id().into(),
        command: "manual/project-specific evidence".into(),
        exit_code: None,
        stdout: None,
        stderr: Some(message.into()),
        passed: false,
    }
}

fn gate_number(gate: VerifyGate) -> u8 {
    match gate {
        VerifyGate::G0 => 0,
        VerifyGate::G1 => 1,
        VerifyGate::G2 => 2,
        VerifyGate::G3 => 3,
        VerifyGate::G4 => 4,
        VerifyGate::G5 => 5,
    }
}

#[cfg(windows)]
fn npm_command() -> &'static str {
    "npm.cmd"
}

#[cfg(not(windows))]
fn npm_command() -> &'static str {
    "npm"
}

#[cfg(windows)]
fn node_command() -> &'static str {
    "node.exe"
}

#[cfg(not(windows))]
fn node_command() -> &'static str {
    "node"
}

#[cfg(windows)]
fn python_command() -> &'static str {
    "python.exe"
}

#[cfg(not(windows))]
fn python_command() -> &'static str {
    "python3"
}

#[cfg(windows)]
fn powershell_command() -> &'static str {
    "pwsh.exe"
}

#[cfg(not(windows))]
fn powershell_command() -> &'static str {
    "pwsh"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!("ade-verify-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[tokio::test]
    async fn g0_fails_without_project_manifest() {
        let root = fixture();
        let result = VerifyRunner::with_root(&root)
            .run_gate(VerifyGate::G0)
            .await;
        assert!(!result.passed);
        assert!(result.stderr.unwrap().contains("manifest"));
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn g1_requires_agent_contract() {
        let root = fixture();
        let runner = VerifyRunner::with_root(&root);
        assert!(!runner.run_gate(VerifyGate::G1).await.passed);
        fs::write(root.join("AGENTS.md"), "# Contract\n").unwrap();
        assert!(runner.run_gate(VerifyGate::G1).await.passed);
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn run_through_stops_at_first_failure() {
        let root = fixture();
        let results = VerifyRunner::with_root(&root)
            .run_through(VerifyGate::G3)
            .await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].gate, "G0");
        let _ = fs::remove_dir_all(root);
    }
}
