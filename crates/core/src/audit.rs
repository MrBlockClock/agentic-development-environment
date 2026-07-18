use crate::ignore::{IgnoreAlignment, IgnoreStatus, IgnoreSurface};
use crate::layer::AdLayer;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const AUDIT_SCHEMA: &str = "ade.audit.report/v1";

/// How the AUDIT phase was invoked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditMode {
    /// Score an existing project/environment.
    #[default]
    EvaluateExisting,
    /// Assess a greenfield/bootstrap setup.
    Bootstrap,
}

impl AuditMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EvaluateExisting => "evaluate_existing",
            Self::Bootstrap => "bootstrap",
        }
    }
}

impl std::str::FromStr for AuditMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "evaluate_existing" | "evaluate" | "existing" => Ok(Self::EvaluateExisting),
            "bootstrap" | "greenfield" => Ok(Self::Bootstrap),
            other => Err(format!(
                "unknown audit mode '{other}' (expected evaluate_existing|bootstrap)"
            )),
        }
    }
}

/// Read-only discovery + scoring result produced by the AUDIT phase.
#[derive(Debug, Serialize, Deserialize)]
pub struct AuditReport {
    pub schema: String,
    pub mode: String,
    pub root: String,
    pub score: u32,
    pub score_max: u32,
    pub findings: Vec<AuditFinding>,
    pub ignore_alignment: Vec<IgnoreAlignment>,
    pub blockers: Vec<String>,
    pub human_summary_markdown: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuditFinding {
    pub layer: String,
    pub severity: String,
    pub detail: String,
    pub points: u32,
    pub points_max: u32,
}

/// Read-only AUDIT runner. Never mutates the workspace.
pub struct AuditRunner {
    root: PathBuf,
}

impl AuditRunner {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn run(&self, mode: AuditMode) -> AuditReport {
        let mut findings = Vec::new();
        let mut blockers = Vec::new();

        for layer in AdLayer::all() {
            findings.push(self.score_layer(&layer, mode));
        }

        let ignore_alignment = self.check_ignore_surfaces();
        for align in &ignore_alignment {
            if matches!(align.status, IgnoreStatus::Missing)
                && matches!(
                    align.surface.as_str(),
                    ".gitignore" | "AGENTS.md policy" | ".cursorignore"
                )
            {
                blockers.push(format!("Missing ignore/policy surface: {}", align.surface));
            }
        }

        if !self.exists("AGENTS.md") {
            blockers.push("AGENTS.md missing — no agent contract".into());
        }

        let score: u32 = findings.iter().map(|f| f.points).sum();
        let score_max: u32 = findings.iter().map(|f| f.points_max).sum();
        let human_summary_markdown = Some(self.summary_markdown(mode, score, score_max, &findings));

        AuditReport {
            schema: AUDIT_SCHEMA.into(),
            mode: mode.as_str().into(),
            root: self.root.display().to_string(),
            score,
            score_max,
            findings,
            ignore_alignment,
            blockers,
            human_summary_markdown,
        }
    }

    fn score_layer(&self, layer: &AdLayer, mode: AuditMode) -> AuditFinding {
        let points_max = 8;
        let (points, severity, detail) = match layer {
            AdLayer::L0 => (
                6,
                "info",
                "Hardware not probed in this AUDIT pass (desktop local-first assumed)".into(),
            ),
            AdLayer::L1 => self.check_l1_shell(),
            AdLayer::L2 => self.check_files(
                &["Cargo.toml", "rust-toolchain.toml"],
                "Canonical Rust workspace + toolchain pin",
            ),
            AdLayer::L3 => self.check_files(&["AGENTS.md"], "ADE contract / portfolio rules"),
            AdLayer::L4 => self.check_any(
                &[".ade", "docs/platform"],
                "Project brain (.ade or platform docs)",
            ),
            AdLayer::L5 => self.check_files(
                &[".gitignore", ".cursorignore"],
                "Context hygiene ignore surfaces",
            ),
            AdLayer::L6 => {
                if mode == AuditMode::Bootstrap {
                    (4, "info", "MCP/tools deferred for bootstrap mode".into())
                } else {
                    self.check_any(
                        &["apps/desktop", "crates/agents"],
                        "Agent/tool surface present",
                    )
                }
            }
            AdLayer::L7 => self.check_files(
                &[".env.example"],
                "Provider/config template (BYOK — keys stay out of repo)",
            ),
            AdLayer::L8 => self.check_files(
                &["Cargo.toml"],
                "Quality gates declared via workspace Cargo.toml",
            ),
            AdLayer::L9 => {
                if self.exists("AGENTS.md") {
                    (8, "ok", "Verify ladder documented in AGENTS.md".into())
                } else {
                    (2, "warn", "No AGENTS.md verify ladder".into())
                }
            }
            AdLayer::L10 => self.check_any(
                &[".ade/handoff", "crates/core/src/handoff.rs"],
                "Continuity / handoff capsule surface",
            ),
            AdLayer::L11 => {
                if self.exists("AGENTS.md") {
                    (8, "ok", "Authority order present via AGENTS.md".into())
                } else {
                    (0, "error", "No governance contract".into())
                }
            }
        };

        AuditFinding {
            layer: format!("{:?} ({})", layer, layer.name()),
            severity: severity.into(),
            detail,
            points,
            points_max,
        }
    }

    fn check_l1_shell(&self) -> (u32, &'static str, String) {
        let mut ok = 0u32;
        let mut bits = Vec::new();
        for (name, cmd) in [("cargo", "cargo"), ("rustc", "rustc"), ("node", "node")] {
            if command_exists(cmd) {
                ok += 1;
                bits.push(format!("{name}=ok"));
            } else {
                bits.push(format!("{name}=missing"));
            }
        }
        let points = match ok {
            3 => 8,
            2 => 5,
            1 => 3,
            _ => 0,
        };
        let severity = if ok == 3 {
            "ok"
        } else if ok > 0 {
            "warn"
        } else {
            "error"
        };
        (
            points,
            severity,
            format!("Toolchain probe: {}", bits.join(", ")),
        )
    }

    fn check_files(&self, files: &[&str], label: &str) -> (u32, &'static str, String) {
        let missing: Vec<_> = files.iter().copied().filter(|f| !self.exists(f)).collect();
        if missing.is_empty() {
            (8, "ok", format!("{label}: all present"))
        } else if missing.len() < files.len() {
            (
                4,
                "warn",
                format!("{label}: missing {}", missing.join(", ")),
            )
        } else {
            (
                0,
                "error",
                format!("{label}: missing {}", missing.join(", ")),
            )
        }
    }

    fn check_any(&self, paths: &[&str], label: &str) -> (u32, &'static str, String) {
        if paths.iter().any(|p| self.exists(p)) {
            (8, "ok", format!("{label}: found"))
        } else {
            (2, "warn", format!("{label}: not found"))
        }
    }

    fn check_ignore_surfaces(&self) -> Vec<IgnoreAlignment> {
        IgnoreSurface::all()
            .into_iter()
            .map(|surface| {
                let (path, status, missing) = match surface {
                    IgnoreSurface::Git => {
                        if self.exists(".gitignore") {
                            (surface.name(), IgnoreStatus::Synced, vec![])
                        } else {
                            (
                                surface.name(),
                                IgnoreStatus::Missing,
                                vec![".gitignore".into()],
                            )
                        }
                    }
                    IgnoreSurface::AiIndex => {
                        if self.exists(".cursorignore") {
                            (surface.name(), IgnoreStatus::Synced, vec![])
                        } else {
                            (
                                surface.name(),
                                IgnoreStatus::Missing,
                                vec![".cursorignore".into()],
                            )
                        }
                    }
                    IgnoreSurface::Docker => {
                        if self.exists("Dockerfile") || self.exists("docker") {
                            if self.exists(".dockerignore") {
                                (surface.name(), IgnoreStatus::Synced, vec![])
                            } else {
                                (
                                    surface.name(),
                                    IgnoreStatus::Drifted,
                                    vec![".dockerignore".into()],
                                )
                            }
                        } else {
                            (surface.name(), IgnoreStatus::NotApplicable, vec![])
                        }
                    }
                    IgnoreSurface::AgentPolicy => {
                        if self.exists("AGENTS.md") {
                            (surface.name(), IgnoreStatus::Synced, vec![])
                        } else {
                            (
                                surface.name(),
                                IgnoreStatus::Missing,
                                vec!["AGENTS.md".into()],
                            )
                        }
                    }
                    IgnoreSurface::BackupSync | IgnoreSurface::CiPublish => {
                        (surface.name(), IgnoreStatus::NotApplicable, vec![])
                    }
                };
                IgnoreAlignment {
                    surface: path.into(),
                    status,
                    missing_patterns: missing,
                }
            })
            .collect()
    }

    fn exists(&self, rel: &str) -> bool {
        self.root.join(rel).exists()
    }

    fn summary_markdown(
        &self,
        mode: AuditMode,
        score: u32,
        score_max: u32,
        findings: &[AuditFinding],
    ) -> String {
        let mut md = format!(
            "# AUDIT report\n\n- Mode: `{}`\n- Root: `{}`\n- Score: **{}/{}**\n\n",
            mode.as_str(),
            self.root.display(),
            score,
            score_max
        );
        md.push_str("## Findings\n\n");
        for f in findings {
            md.push_str(&format!(
                "- **{}** [{}] {}/{} — {}\n",
                f.layer, f.severity, f.points, f.points_max, f.detail
            ));
        }
        md
    }
}

fn command_exists(cmd: &str) -> bool {
    #[cfg(windows)]
    let probe = format!("{cmd}.exe");
    #[cfg(not(windows))]
    let probe = cmd.to_string();

    std::process::Command::new(if cfg!(windows) { "where" } else { "which" })
        .arg(&probe)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn audit_scores_this_fixture() {
        let dir = std::env::temp_dir().join(format!("ade-audit-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("Cargo.toml"), "[workspace]\nmembers=[]\n").unwrap();
        fs::write(
            dir.join("rust-toolchain.toml"),
            "[toolchain]\nchannel=\"stable\"\n",
        )
        .unwrap();
        fs::write(dir.join("AGENTS.md"), "# contract\n").unwrap();
        fs::write(dir.join(".gitignore"), "target/\n").unwrap();
        fs::write(dir.join(".cursorignore"), "target/\n").unwrap();
        fs::write(dir.join(".env.example"), "ADE_ENV=local\n").unwrap();
        fs::create_dir_all(dir.join(".ade/handoff")).unwrap();

        let report = AuditRunner::new(&dir).run(AuditMode::EvaluateExisting);
        assert_eq!(report.schema, AUDIT_SCHEMA);
        assert!(report.score > 0);
        assert!(report.score <= report.score_max);
        assert!(report.blockers.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }
}
