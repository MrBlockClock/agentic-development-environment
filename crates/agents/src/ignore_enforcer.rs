use ade_core::error::AdeError;
use ade_core::ignore::{
    check_alignment, ensure_bootstrap_ignores, IgnoreAlignment, SensitivePathPolicy,
};
use std::path::{Path, PathBuf};

/// Runtime ignore drift detector across the six ADE surfaces.
pub struct IgnoreEnforcer {
    root: PathBuf,
}

impl IgnoreEnforcer {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn check_alignment(&self) -> Vec<IgnoreAlignment> {
        check_alignment(&self.root)
    }

    /// Secret and always-ignore paths must never be read or written by tools.
    pub fn path_is_blocked(&self, path: &str) -> bool {
        SensitivePathPolicy::path_is_blocked(path)
    }

    pub fn ensure_surfaces(&self) -> Result<(), AdeError> {
        ensure_bootstrap_ignores(&self.root)
    }
}

pub fn ensure_ignore_file(root: &Path, relative: &str) -> Result<PathBuf, AdeError> {
    ensure_bootstrap_ignores(root)?;
    Ok(root.join(relative))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ade_core::ignore::IgnoreStatus;

    #[test]
    fn detects_missing_and_drifted_gitignore() {
        let root = std::env::temp_dir().join(format!("ade-ignore-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let enforcer = IgnoreEnforcer::new(&root);
        let missing = enforcer.check_alignment();
        assert!(missing.iter().any(|item| {
            item.surface == ".gitignore" && matches!(item.status, IgnoreStatus::Missing)
        }));
        std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
        let drifted = IgnoreEnforcer::new(&root).check_alignment();
        assert!(drifted.iter().any(|item| {
            item.surface == ".gitignore" && matches!(item.status, IgnoreStatus::Drifted)
        }));
        ensure_ignore_file(&root, ".gitignore").unwrap();
        let synced = IgnoreEnforcer::new(&root).check_alignment();
        assert!(synced.iter().any(|item| {
            item.surface == ".gitignore" && matches!(item.status, IgnoreStatus::Synced)
        }));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn blocks_secret_paths() {
        let enforcer = IgnoreEnforcer::new(".");
        assert!(enforcer.path_is_blocked(".env"));
        assert!(enforcer.path_is_blocked("secrets/credentials.json"));
        assert!(!enforcer.path_is_blocked("src/lib.rs"));
    }
}
