use crate::error::AdeError;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IgnoreSurface {
    Git,
    AiIndex,
    Docker,
    AgentPolicy,
    BackupSync,
    CiPublish,
}

impl IgnoreSurface {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Git => ".gitignore",
            Self::AiIndex => ".cursorignore",
            Self::Docker => ".dockerignore",
            Self::AgentPolicy => "AGENTS.md policy",
            Self::BackupSync => "Backup/Sync exclusions",
            Self::CiPublish => "CI/Publish filters",
        }
    }

    pub fn all() -> Vec<IgnoreSurface> {
        vec![
            Self::Git,
            Self::AiIndex,
            Self::Docker,
            Self::AgentPolicy,
            Self::BackupSync,
            Self::CiPublish,
        ]
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IgnoreAlignment {
    pub surface: String,
    pub status: IgnoreStatus,
    pub missing_patterns: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IgnoreStatus {
    Synced,
    Drifted,
    Missing,
    NotApplicable,
}

/// Patterns that must appear on git/AI/docker ignore surfaces.
pub fn always_ignore_patterns() -> &'static [&'static str] {
    &[
        ".env",
        ".env.*",
        "!.env.example",
        "*.pem",
        "*.key",
        "*credentials*.json",
        "node_modules/",
        "target/",
        "dist/",
        ".venv/",
        "test-results/",
        "playwright-report/",
        "**/storageState.json",
        "*.db",
        "*.sqlite*",
        "*.tursodb",
    ]
}

/// Patterns that must remain visible to agents and CI.
pub fn never_ignore_patterns() -> &'static [&'static str] {
    &[
        "AGENTS.md",
        "Cargo.lock",
        "package-lock.json",
        "pnpm-lock.yaml",
        "yarn.lock",
        "rust-toolchain.toml",
        ".nvmrc",
        ".env.example",
    ]
}

/// Canonical secret / always-ignored path policy shared by authority, handoff, audit, and tools.
#[derive(Debug, Default, Clone, Copy)]
pub struct SensitivePathPolicy;

impl SensitivePathPolicy {
    pub fn is_secret_path(path: &str) -> bool {
        let normalized = normalize_path(path);
        normalized.split('/').any(|part| {
            part == ".env"
                || (part.starts_with(".env.") && part != ".env.example")
                || part.ends_with(".pem")
                || part.ends_with(".key")
                || (part.contains("credential") && part.ends_with(".json"))
        })
    }

    pub fn is_always_ignored_path(path: &str) -> bool {
        let normalized = normalize_path(path);
        let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
        if Self::is_secret_path(&normalized) || file_name.eq_ignore_ascii_case("storagestate.json")
        {
            return true;
        }
        for pattern in always_ignore_patterns() {
            if pattern.starts_with('!') {
                continue;
            }
            if gitignore_match(pattern, &normalized) {
                return true;
            }
        }
        false
    }

    pub fn path_is_blocked(path: &str) -> bool {
        Self::is_secret_path(path) || Self::is_always_ignored_path(path)
    }
}

/// Merge always-ignore patterns into the primary ignore surfaces.
pub fn ensure_bootstrap_ignores(root: &Path) -> Result<(), AdeError> {
    ensure_ignore_file(root, ".gitignore")?;
    ensure_ignore_file(root, ".cursorignore")?;
    if root.join("Dockerfile").exists()
        || root.join("docker").exists()
        || root.join("docker-compose.yml").exists()
        || root.join(".dockerignore").exists()
    {
        ensure_ignore_file(root, ".dockerignore")?;
    }
    Ok(())
}

/// Compute the merged ignore-file body without writing to disk.
pub fn merge_ignore_content(existing: &str) -> (String, bool) {
    let mut lines = existing.lines().map(str::to_string).collect::<Vec<_>>();
    let mut changed = false;
    if !existing.contains("ADE always-ignore") {
        if !lines.is_empty() && !lines.last().map(|line| line.is_empty()).unwrap_or(false) {
            lines.push(String::new());
        }
        lines.push("# ADE always-ignore (do not remove)".into());
        changed = true;
    }
    for pattern in always_ignore_patterns() {
        if !lines.iter().any(|line| line.trim() == *pattern) {
            lines.push((*pattern).into());
            changed = true;
        }
    }
    let mut body = lines.join("\n");
    if !body.ends_with('\n') {
        body.push('\n');
    }
    // Creating a missing file always counts as a change.
    if existing.is_empty() {
        changed = true;
    }
    (body, changed)
}

/// Score ignore-surface alignment for a workspace root.
pub fn check_alignment(root: &Path) -> Vec<IgnoreAlignment> {
    IgnoreSurface::all()
        .into_iter()
        .map(|surface| align_surface(root, surface))
        .collect()
}

fn align_surface(root: &Path, surface: IgnoreSurface) -> IgnoreAlignment {
    match surface {
        IgnoreSurface::Git => file_alignment(root, ".gitignore", true),
        IgnoreSurface::AiIndex => file_alignment(root, ".cursorignore", true),
        IgnoreSurface::Docker => {
            if root.join("Dockerfile").exists()
                || root.join("docker").exists()
                || root.join("docker-compose.yml").exists()
            {
                file_alignment(root, ".dockerignore", true)
            } else {
                IgnoreAlignment {
                    surface: surface.name().into(),
                    status: IgnoreStatus::NotApplicable,
                    missing_patterns: vec![],
                }
            }
        }
        IgnoreSurface::AgentPolicy => {
            if root.join("AGENTS.md").is_file() {
                IgnoreAlignment {
                    surface: surface.name().into(),
                    status: IgnoreStatus::Synced,
                    missing_patterns: vec![],
                }
            } else {
                IgnoreAlignment {
                    surface: surface.name().into(),
                    status: IgnoreStatus::Missing,
                    missing_patterns: vec!["AGENTS.md".into()],
                }
            }
        }
        IgnoreSurface::BackupSync => optional_marker(
            root,
            surface,
            &[".ade/backup-exclusions", ".rsyncignore", ".syncthingignore"],
        ),
        IgnoreSurface::CiPublish => optional_marker(
            root,
            surface,
            &[".ade/ci-publish-filters", ".gitattributes"],
        ),
    }
}

fn file_alignment(root: &Path, relative: &str, required: bool) -> IgnoreAlignment {
    let path = root.join(relative);
    if !path.is_file() {
        return IgnoreAlignment {
            surface: relative.into(),
            status: if required {
                IgnoreStatus::Missing
            } else {
                IgnoreStatus::NotApplicable
            },
            missing_patterns: if required {
                always_ignore_patterns()
                    .iter()
                    .map(|pattern| (*pattern).into())
                    .collect()
            } else {
                vec![]
            },
        };
    }
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let missing = missing_required_patterns(&content);
    IgnoreAlignment {
        surface: relative.into(),
        status: if missing.is_empty() {
            IgnoreStatus::Synced
        } else {
            IgnoreStatus::Drifted
        },
        missing_patterns: missing,
    }
}

fn optional_marker(root: &Path, surface: IgnoreSurface, candidates: &[&str]) -> IgnoreAlignment {
    if candidates.iter().any(|path| root.join(path).exists()) {
        IgnoreAlignment {
            surface: surface.name().into(),
            status: IgnoreStatus::Synced,
            missing_patterns: vec![],
        }
    } else {
        IgnoreAlignment {
            surface: surface.name().into(),
            status: IgnoreStatus::NotApplicable,
            missing_patterns: vec![],
        }
    }
}

fn missing_required_patterns(content: &str) -> Vec<String> {
    let normalized = content
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect::<Vec<_>>();
    always_ignore_patterns()
        .iter()
        .filter(|pattern| {
            !normalized
                .iter()
                .any(|line| line == *pattern || line.ends_with(*pattern))
        })
        .map(|pattern| (*pattern).into())
        .collect()
}

fn ensure_ignore_file(root: &Path, relative: &str) -> Result<(), AdeError> {
    let path = root.join(relative);
    let existing = if path.exists() {
        std::fs::read_to_string(&path)?
    } else {
        String::new()
    };
    let (body, changed) = merge_ignore_content(&existing);
    if changed || !path.exists() {
        std::fs::write(&path, body)?;
    }
    Ok(())
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

/// Minimal gitignore-style matcher for ADE always-ignore patterns.
fn gitignore_match(pattern: &str, path: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() || pattern.starts_with('#') || pattern.starts_with('!') {
        return false;
    }
    let path = path.trim_start_matches("./");
    if let Some(rest) = pattern.strip_prefix("**/") {
        return path.ends_with(rest)
            || path.contains(&format!("/{rest}"))
            || wildcard_match(rest, path.rsplit('/').next().unwrap_or(path));
    }
    if pattern.ends_with('/') {
        let dir = pattern.trim_end_matches('/');
        return path == dir
            || path.starts_with(&format!("{dir}/"))
            || path.contains(&format!("/{dir}/"));
    }
    if pattern.contains('*') {
        let file_name = path.rsplit('/').next().unwrap_or(path);
        return wildcard_match(pattern, file_name) || wildcard_match(pattern, path);
    }
    path == pattern
        || path.ends_with(&format!("/{pattern}"))
        || path.contains(&format!("/{pattern}/"))
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let mut table = vec![vec![false; value.len() + 1]; pattern.len() + 1];
    table[0][0] = true;
    for index in 1..=pattern.len() {
        if pattern[index - 1] == b'*' {
            table[index][0] = table[index - 1][0];
        }
        for offset in 1..=value.len() {
            table[index][offset] = match pattern[index - 1] {
                b'*' => table[index - 1][offset] || table[index][offset - 1],
                b'?' => table[index - 1][offset - 1],
                byte => {
                    byte.eq_ignore_ascii_case(&value[offset - 1]) && table[index - 1][offset - 1]
                }
            };
        }
    }
    table[pattern.len()][value.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_and_ignored_paths_are_blocked() {
        assert!(SensitivePathPolicy::is_secret_path(".env"));
        assert!(SensitivePathPolicy::is_secret_path(
            "secrets/credentials.json"
        ));
        assert!(SensitivePathPolicy::path_is_blocked(
            "node_modules/pkg/index.js"
        ));
        assert!(SensitivePathPolicy::path_is_blocked(
            "playwright/storageState.json"
        ));
        assert!(!SensitivePathPolicy::path_is_blocked("src/lib.rs"));
        assert!(!SensitivePathPolicy::is_secret_path(".env.example"));
    }

    #[test]
    fn reports_missing_gitignore_patterns() {
        let root = std::env::temp_dir().join(format!("ade-ignore-core-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let missing = check_alignment(&root);
        assert!(missing.iter().any(|item| {
            item.surface == ".gitignore" && matches!(item.status, IgnoreStatus::Missing)
        }));
        std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
        let drifted = check_alignment(&root);
        assert!(drifted.iter().any(|item| {
            item.surface == ".gitignore" && matches!(item.status, IgnoreStatus::Drifted)
        }));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn dockerignore_accepts_exact_always_ignore_forms() {
        let root = std::env::temp_dir().join(format!("ade-ignore-docker-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("Dockerfile"), "FROM scratch\n").unwrap();
        std::fs::write(root.join(".dockerignore"), "target/\n").unwrap();
        let before = check_alignment(&root);
        assert!(before.iter().any(|item| {
            item.surface == ".dockerignore" && matches!(item.status, IgnoreStatus::Drifted)
        }));
        ensure_bootstrap_ignores(&root).unwrap();
        let after = check_alignment(&root);
        assert!(after.iter().any(|item| {
            item.surface == ".dockerignore" && matches!(item.status, IgnoreStatus::Synced)
        }));
        let _ = std::fs::remove_dir_all(root);
    }
}
