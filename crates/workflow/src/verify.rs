use ade_core::verify::{VerifyGate, VerifyResult, VerifyStatus};
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
        self.run_gate_sync(gate)
    }

    pub fn run_gate_sync(&self, gate: VerifyGate) -> VerifyResult {
        match gate {
            VerifyGate::G0 => self.run_g0(),
            VerifyGate::G1 => self.run_g1(),
            VerifyGate::G2 => self.run_g2(),
            VerifyGate::G3 => self.run_g3(),
            VerifyGate::G4 => self.run_g4(),
            VerifyGate::G5 => self.run_g5(),
        }
    }

    pub async fn run_through(&self, max_gate: VerifyGate) -> Vec<VerifyResult> {
        let mut results = Vec::new();
        for gate in VerifyGate::all() {
            if gate_number(gate) > gate_number(max_gate) {
                break;
            }
            let result = self.run_gate(gate).await;
            let stop = !result.passed && result.status != VerifyStatus::Unavailable;
            results.push(result);
            if stop {
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
            status: if missing.is_empty() {
                VerifyStatus::Pass
            } else {
                VerifyStatus::Fail
            },
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

    fn run_g5(&self) -> VerifyResult {
        let evidence_dir = self.root.join(".ade").join("verify");
        let _ = std::fs::create_dir_all(&evidence_dir);

        let powershell = self.root.join("scripts").join("g5-evidence.ps1");
        let shell = self.root.join("scripts").join("g5-evidence.sh");
        if powershell.is_file() {
            let result = self.run_commands(
                VerifyGate::G5,
                vec![CommandSpec::new(
                    powershell_command(),
                    &["-NoProfile", "-File", &powershell.display().to_string()],
                )],
            );
            return persist_g5_evidence(&evidence_dir, result);
        }
        if shell.is_file() {
            let result = self.run_commands(
                VerifyGate::G5,
                vec![CommandSpec::new(
                    "bash",
                    &[shell.display().to_string().as_str()],
                )],
            );
            return persist_g5_evidence(&evidence_dir, result);
        }

        match detect_recipe_g5(&self.root) {
            ade_core::recipe::RecipeG5::Playwright => {
                let has_playwright = self.root.join("playwright.config.ts").is_file()
                    || self.root.join("playwright.config.js").is_file()
                    || self.root.join("playwright.config.mjs").is_file()
                    || package_json_mentions_playwright(&self.root);
                if has_playwright {
                    let result = self.run_commands(
                        VerifyGate::G5,
                        vec![CommandSpec::new(
                            npx_command(),
                            &["playwright", "test", "--reporter=line"],
                        )],
                    );
                    return persist_g5_evidence(&evidence_dir, result);
                }
                return unavailable(
                    VerifyGate::G5,
                    "recipe G5 is Playwright but no playwright.config.* was found",
                );
            }
            ade_core::recipe::RecipeG5::BinarySmoke
            | ade_core::recipe::RecipeG5::HttpContract
            | ade_core::recipe::RecipeG5::UpstreamTests
            | ade_core::recipe::RecipeG5::InstallSmoke
            | ade_core::recipe::RecipeG5::ParityProbes => {
                if self.root.join("Cargo.toml").is_file() {
                    let result = self.run_commands(
                        VerifyGate::G5,
                        vec![CommandSpec::new(
                            "cargo",
                            &["test", "--workspace", "--", "--nocapture"],
                        )],
                    );
                    return persist_g5_evidence(&evidence_dir, result);
                }
                return unavailable(
                    VerifyGate::G5,
                    "recipe G5 expects cargo/binary evidence but Cargo.toml is missing",
                );
            }
            ade_core::recipe::RecipeG5::PlaytestChecklist
            | ade_core::recipe::RecipeG5::ReproducibilityNote
            | ade_core::recipe::RecipeG5::DeviceChecklist
            | ade_core::recipe::RecipeG5::HardwareSignoff
            | ade_core::recipe::RecipeG5::PlanChecklist => {
                let checklist = self.root.join("scripts").join("g5-checklist.md");
                if checklist.is_file() {
                    return unavailable(
                        VerifyGate::G5,
                        "recipe G5 requires human sign-off — complete scripts/g5-checklist.md then add scripts/g5-evidence.*",
                    );
                }
                return unavailable(
                    VerifyGate::G5,
                    "recipe G5 requires human checklist/sign-off evidence (scripts/g5-checklist.md)",
                );
            }
            ade_core::recipe::RecipeG5::None => {}
        }

        let has_playwright = self.root.join("playwright.config.ts").is_file()
            || self.root.join("playwright.config.js").is_file()
            || self.root.join("playwright.config.mjs").is_file()
            || package_json_mentions_playwright(&self.root);
        if has_playwright {
            let result = self.run_commands(
                VerifyGate::G5,
                vec![CommandSpec::new(
                    npx_command(),
                    &["playwright", "test", "--reporter=line"],
                )],
            );
            return persist_g5_evidence(&evidence_dir, result);
        }

        if self.root.join("Cargo.toml").is_file() {
            let result = self.run_commands(
                VerifyGate::G5,
                vec![CommandSpec::new(
                    "cargo",
                    &["test", "--workspace", "--", "--nocapture"],
                )],
            );
            return persist_g5_evidence(&evidence_dir, result);
        }

        unavailable(
            VerifyGate::G5,
            "no recipe G5 profile, Playwright config, scripts/g5-evidence.*, or Cargo tests found",
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
                            status: VerifyStatus::Fail,
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
                        status: VerifyStatus::Fail,
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
            status: VerifyStatus::Pass,
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

fn detect_recipe_g5(root: &Path) -> ade_core::recipe::RecipeG5 {
    let recipe_meta = root.join(".ade").join("recipe.json");
    if let Ok(raw) = std::fs::read_to_string(&recipe_meta) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(g5) = value
                .get("g5")
                .and_then(|value| serde_json::from_value(value.clone()).ok())
            {
                return g5;
            }
            if let Some(id) = value.get("id").and_then(|value| value.as_str()) {
                if let Ok(recipe) = ade_core::recipe::builtin_recipe(id) {
                    return recipe.g5;
                }
            }
        }
    }

    if let Ok(agents) = std::fs::read_to_string(root.join("AGENTS.md")) {
        if let Some(id) = agents
            .lines()
            .find_map(|line| {
                line.split_once("stack recipe `")
                    .and_then(|(_, rest)| rest.split('`').next())
            })
            .filter(|id| !id.is_empty())
        {
            if let Ok(recipe) = ade_core::recipe::builtin_recipe(id) {
                return recipe.g5;
            }
        }
    }

    ade_core::recipe::RecipeG5::None
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
        status: VerifyStatus::Fail,
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
        status: VerifyStatus::Unavailable,
    }
}

fn persist_g5_evidence(evidence_dir: &Path, result: VerifyResult) -> VerifyResult {
    let payload = serde_json::json!({
        "gate": result.gate,
        "command": result.command,
        "passed": result.passed,
        "status": result.status,
        "exit_code": result.exit_code,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "artifacts": {
            "report": evidence_dir.join("g5-evidence.json").display().to_string(),
            "playwright_report": "playwright-report",
            "test_results": "test-results",
        }
    });
    let path = evidence_dir.join("g5-evidence.json");
    let _ = std::fs::write(
        &path,
        serde_json::to_vec_pretty(&payload).unwrap_or_default(),
    );
    result
}

fn package_json_mentions_playwright(root: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(root.join("package.json")) else {
        return false;
    };
    text.contains("playwright")
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
fn npx_command() -> &'static str {
    "npx.cmd"
}

#[cfg(not(windows))]
fn npx_command() -> &'static str {
    "npx"
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

    #[tokio::test]
    async fn g5_follows_recipe_checklist_profile() {
        let root = fixture();
        fs::write(
            root.join("AGENTS.md"),
            "# Contract\n\nGenerated by ADE from stack recipe `embedded-hil` (`Embedded HIL`).\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("scripts")).unwrap();
        fs::write(root.join("scripts").join("g5-checklist.md"), "# signoff\n").unwrap();
        let result = VerifyRunner::with_root(&root)
            .run_gate(VerifyGate::G5)
            .await;
        assert_eq!(result.status, VerifyStatus::Unavailable);
        assert!(result.stderr.unwrap_or_default().contains("human sign-off"));
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn g5_unavailable_without_evidence_sources() {
        let root = fixture();
        fs::write(root.join("AGENTS.md"), "# Contract\n").unwrap();
        let result = VerifyRunner::with_root(&root)
            .run_gate(VerifyGate::G5)
            .await;
        assert!(!result.passed);
        assert_eq!(result.status, VerifyStatus::Unavailable);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unavailable_status_is_not_a_hard_stop() {
        let result = unavailable(VerifyGate::G4, "missing integration script");
        assert!(!result.passed);
        assert_eq!(result.status, VerifyStatus::Unavailable);
        let stop = !result.passed && result.status != VerifyStatus::Unavailable;
        assert!(!stop);
    }

    #[tokio::test]
    async fn g5_persists_evidence_json_on_script_path() {
        let root = fixture();
        fs::create_dir_all(root.join("scripts")).unwrap();
        #[cfg(windows)]
        {
            fs::write(
                root.join("scripts/g5-evidence.ps1"),
                "Write-Output 'g5-ok'; exit 0\n",
            )
            .unwrap();
        }
        #[cfg(not(windows))]
        {
            fs::write(
                root.join("scripts/g5-evidence.sh"),
                "#!/bin/sh\necho g5-ok\n",
            )
            .unwrap();
            let _ = std::process::Command::new("chmod")
                .args(["+x", root.join("scripts/g5-evidence.sh").to_str().unwrap()])
                .status();
        }
        let result = VerifyRunner::with_root(&root)
            .run_gate(VerifyGate::G5)
            .await;
        assert!(root.join(".ade/verify/g5-evidence.json").is_file());
        // Script may fail if pwsh/bash missing; evidence file must still exist.
        let _ = result;
        let _ = fs::remove_dir_all(root);
    }
}
