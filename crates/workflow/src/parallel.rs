use ade_core::error::AdeError;
use chrono::{DateTime, Duration, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

pub const LEASE_REGISTRY_SCHEMA: &str = "ade.lease.registry/v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LeaseMode {
    Observe,
    Cooperative,
    Strong,
    Exclusive,
}

impl LeaseMode {
    pub fn parse(value: &str) -> Result<Self, AdeError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "observe" => Ok(Self::Observe),
            "cooperative" => Ok(Self::Cooperative),
            "strong" => Ok(Self::Strong),
            "exclusive" => Ok(Self::Exclusive),
            other => Err(AdeError::Other(format!(
                "unknown lease mode '{other}' (observe|cooperative|strong|exclusive)"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::Cooperative => "cooperative",
            Self::Strong => "strong",
            Self::Exclusive => "exclusive",
        }
    }

    fn compatible_with(self, other: Self) -> bool {
        use LeaseMode::*;
        match (self, other) {
            (Exclusive, _) | (_, Exclusive) => false,
            (Observe, Observe) | (Observe, Cooperative) | (Observe, Strong) => true,
            (Cooperative, Observe) | (Cooperative, Cooperative) => true,
            (Strong, Observe) => true,
            (Strong, Cooperative) | (Cooperative, Strong) | (Strong, Strong) => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PathLease {
    pub id: String,
    pub agent_id: Uuid,
    pub path: String,
    pub mode: LeaseMode,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorktreeInfo {
    pub path: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub bare: bool,
    pub locked: bool,
    pub prunable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LeaseRegistry {
    schema: String,
    leases: Vec<PathLease>,
}

/// Durable path-lease registry stored under `.ade/leases/registry.json`.
pub struct LeaseManager {
    root: PathBuf,
}

impl LeaseManager {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn list(&self) -> Result<Vec<PathLease>, AdeError> {
        self.with_registry_mut(|registry| {
            purge_expired(registry);
            Ok(registry.leases.clone())
        })
    }

    /// Resolve the write scope for one agent from active leases.
    ///
    /// Observe leases never grant writes. If `requested` is non-empty, every
    /// requested path must be contained by one of the agent's writable leases.
    pub fn resolve_owned_paths(
        &self,
        agent_id: Uuid,
        requested: &[String],
    ) -> Result<Vec<String>, AdeError> {
        let mut writable = self
            .list()?
            .into_iter()
            .filter(|lease| lease.agent_id == agent_id && !matches!(lease.mode, LeaseMode::Observe))
            .map(|lease| lease.path)
            .collect::<Vec<_>>();
        writable.sort();
        writable.dedup();

        if requested.is_empty() {
            return Ok(writable);
        }

        let mut resolved = Vec::new();
        for path in requested {
            let normalized = normalize_lease_path(path)?;
            if !writable
                .iter()
                .any(|leased| path_is_within(&normalized, leased))
            {
                return Err(AdeError::Authorization(format!(
                    "requested owned_path '{normalized}' is not covered by an active writable lease for agent {agent_id}"
                )));
            }
            resolved.push(normalized);
        }
        resolved.sort();
        resolved.dedup();
        Ok(resolved)
    }

    pub fn acquire(
        &self,
        agent_id: Uuid,
        path: &str,
        mode: LeaseMode,
        ttl: Duration,
    ) -> Result<PathLease, AdeError> {
        let path = normalize_lease_path(path)?;
        if ttl <= Duration::zero() {
            return Err(AdeError::Other("lease ttl must be positive".into()));
        }
        let protected = is_protected_path(&path);
        if protected && matches!(mode, LeaseMode::Observe | LeaseMode::Cooperative) {
            return Err(AdeError::Authorization(format!(
                "protected path '{path}' requires strong or exclusive lease"
            )));
        }

        self.with_registry_mut(|registry| {
            purge_expired(registry);

            for existing in &registry.leases {
                if !paths_overlap(&path, &existing.path) {
                    continue;
                }
                if existing.agent_id == agent_id && existing.path == path {
                    return Err(AdeError::Other(format!(
                        "agent {agent_id} already holds a lease on '{path}'"
                    )));
                }
                if !mode.compatible_with(existing.mode) {
                    return Err(AdeError::Authorization(format!(
                        "lease conflict on '{}': requested {} incompatible with existing {} held by {}",
                        existing.path,
                        mode.as_str(),
                        existing.mode.as_str(),
                        existing.agent_id
                    )));
                }
                if (protected || existing.protected)
                    && !matches!(mode, LeaseMode::Observe)
                    && !matches!(existing.mode, LeaseMode::Observe)
                {
                    return Err(AdeError::Authorization(format!(
                        "protected path '{}' is already leased and must be serialized",
                        existing.path
                    )));
                }
            }

            let now = Utc::now();
            let lease = PathLease {
                id: Uuid::new_v4().to_string(),
                agent_id,
                path,
                mode,
                created_at: now,
                expires_at: now + ttl,
                protected,
            };
            registry.leases.push(lease.clone());
            Ok(lease)
        })
    }

    pub fn release(&self, lease_id: &str) -> Result<bool, AdeError> {
        validate_lease_id(lease_id)?;
        self.with_registry_mut(|registry| {
            purge_expired(registry);
            let before = registry.leases.len();
            registry.leases.retain(|lease| lease.id != lease_id);
            Ok(registry.leases.len() != before)
        })
    }

    /// Extend an active lease's expiry (heartbeat). Only the holding agent may
    /// renew, and expired leases cannot be revived — they must be re-acquired
    /// so conflict checks run again.
    pub fn renew(
        &self,
        agent_id: Uuid,
        lease_id: &str,
        ttl: Duration,
    ) -> Result<PathLease, AdeError> {
        validate_lease_id(lease_id)?;
        if ttl <= Duration::zero() {
            return Err(AdeError::Other("lease ttl must be positive".into()));
        }
        self.with_registry_mut(|registry| {
            purge_expired(registry);
            let lease = registry
                .leases
                .iter_mut()
                .find(|lease| lease.id == lease_id)
                .ok_or_else(|| {
                    AdeError::Other(format!(
                        "lease '{lease_id}' is not active (expired or released)"
                    ))
                })?;
            if lease.agent_id != agent_id {
                return Err(AdeError::Authorization(format!(
                    "lease '{lease_id}' is held by {}, not {agent_id}",
                    lease.agent_id
                )));
            }
            lease.expires_at = Utc::now() + ttl;
            Ok(lease.clone())
        })
    }

    pub fn release_stale(&self) -> Result<usize, AdeError> {
        self.with_registry_mut(|registry| {
            let before = registry.leases.len();
            purge_expired(registry);
            Ok(before.saturating_sub(registry.leases.len()))
        })
    }

    fn with_registry_mut<T>(
        &self,
        operation: impl FnOnce(&mut LeaseRegistry) -> Result<T, AdeError>,
    ) -> Result<T, AdeError> {
        let lock = self.open_lock()?;
        lock.lock_exclusive()?;
        let mut registry = self.load_unlocked()?;
        let result = operation(&mut registry)?;
        self.save_unlocked(&registry)?;
        Ok(result)
    }

    fn open_lock(&self) -> Result<File, AdeError> {
        let directory = self.root.join(".ade").join("leases");
        std::fs::create_dir_all(&directory)?;
        Ok(OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(directory.join("registry.lock"))?)
    }

    fn load_unlocked(&self) -> Result<LeaseRegistry, AdeError> {
        let path = self.registry_path();
        if !path.is_file() {
            return Ok(LeaseRegistry {
                schema: LEASE_REGISTRY_SCHEMA.into(),
                leases: vec![],
            });
        }
        let payload = std::fs::read_to_string(&path)?;
        let registry: LeaseRegistry = serde_json::from_str(&payload)?;
        if registry.schema != LEASE_REGISTRY_SCHEMA {
            return Err(AdeError::Other("unsupported lease registry schema".into()));
        }
        Ok(registry)
    }

    fn save_unlocked(&self, registry: &LeaseRegistry) -> Result<(), AdeError> {
        let directory = self.root.join(".ade").join("leases");
        std::fs::create_dir_all(&directory)?;
        let payload = serde_json::to_vec_pretty(registry)?;
        let path = self.registry_path();
        let temporary = directory.join("registry.json.tmp");
        std::fs::write(&temporary, payload)?;
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        std::fs::rename(temporary, path)?;
        Ok(())
    }

    fn registry_path(&self) -> PathBuf {
        self.root.join(".ade").join("leases").join("registry.json")
    }

    #[cfg(test)]
    fn save_for_tests(&self, registry: &LeaseRegistry) -> Result<(), AdeError> {
        let lock = self.open_lock()?;
        lock.lock_exclusive()?;
        self.save_unlocked(registry)
    }
}

/// Git worktree orchestration with argument-safe process calls.
pub struct WorktreeManager {
    root: PathBuf,
}

impl Default for WorktreeManager {
    fn default() -> Self {
        Self::new(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }
}

impl WorktreeManager {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn list(&self) -> Result<Vec<WorktreeInfo>, AdeError> {
        ensure_git_repo(&self.root)?;
        let stdout = git_output(&self.root, &["worktree", "list", "--porcelain"])?;
        Ok(parse_worktree_list(&stdout))
    }

    pub fn add(
        &self,
        path: &Path,
        branch: &str,
        start_point: Option<&str>,
    ) -> Result<WorktreeInfo, AdeError> {
        ensure_git_repo(&self.root)?;
        validate_branch_name(branch)?;
        let absolute = canonicalize_worktree_path(&self.root, path)?;
        if absolute.exists() {
            return Err(AdeError::Other(format!(
                "worktree path already exists: {}",
                absolute.display()
            )));
        }
        if let Some(parent) = absolute.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut args = vec![
            "worktree".into(),
            "add".into(),
            absolute.display().to_string(),
            "-b".into(),
            branch.into(),
        ];
        if let Some(start) = start_point {
            validate_branch_name(start)?;
            args.push(start.into());
        }
        let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        git_output(&self.root, &arg_refs)?;
        self.list()?
            .into_iter()
            .find(|item| same_worktree_path(Path::new(&item.path), &absolute))
            .ok_or_else(|| AdeError::Other("worktree created but not listed by git".into()))
    }

    pub fn remove(&self, path: &Path, force: bool) -> Result<(), AdeError> {
        ensure_git_repo(&self.root)?;
        let absolute = canonicalize_worktree_path(&self.root, path)?;
        if absolute
            == self
                .root
                .canonicalize()
                .unwrap_or_else(|_| self.root.clone())
        {
            return Err(AdeError::Authorization(
                "refusing to remove the primary worktree".into(),
            ));
        }
        if !force && worktree_is_dirty(&absolute)? {
            return Err(AdeError::Other(format!(
                "worktree is dirty; refuse implicit remove ({})",
                absolute.display()
            )));
        }
        let path_arg = absolute.display().to_string();
        if force {
            git_output(&self.root, &["worktree", "remove", "--force", &path_arg])?;
        } else {
            git_output(&self.root, &["worktree", "remove", &path_arg])?;
        }
        Ok(())
    }

    /// Backward-compatible helper used by older call sites.
    pub fn lease_path(&self, agent_id: Uuid, path: &str, mode: LeaseMode) -> Result<(), AdeError> {
        LeaseManager::new(&self.root)
            .acquire(agent_id, path, mode, Duration::hours(8))
            .map(|_| ())
    }
}

fn purge_expired(registry: &mut LeaseRegistry) -> bool {
    let now = Utc::now();
    let before = registry.leases.len();
    registry.leases.retain(|lease| lease.expires_at > now);
    before != registry.leases.len()
}

fn normalize_lease_path(path: &str) -> Result<String, AdeError> {
    let normalized = path.replace('\\', "/");
    let trimmed = normalized.trim().trim_matches('/').to_string();
    if trimmed.is_empty()
        || trimmed.contains('\0')
        || trimmed.split('/').any(|part| part == "..")
        || looks_host_absolute(path)
        || trimmed.starts_with("~/")
    {
        return Err(AdeError::Authorization(format!(
            "refusing unsafe lease path '{path}'"
        )));
    }
    Ok(trimmed)
}

fn looks_host_absolute(value: &str) -> bool {
    let path = Path::new(value);
    if path.is_absolute() {
        return true;
    }
    let normalized = value.replace('\\', "/");
    if normalized.starts_with('/') || normalized.starts_with("~/") {
        return true;
    }
    let bytes = normalized.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn paths_overlap(a: &str, b: &str) -> bool {
    a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
}

fn path_is_within(candidate: &str, parent: &str) -> bool {
    candidate == parent || candidate.starts_with(&format!("{parent}/"))
}

fn is_protected_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "cargo.lock"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "composer.lock"
            | "gemfile.lock"
    ) || lower == "migrations"
        || lower.starts_with("migrations/")
}

fn validate_lease_id(id: &str) -> Result<(), AdeError> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err(AdeError::NotFound("invalid lease id".into()));
    }
    Ok(())
}

fn validate_branch_name(branch: &str) -> Result<(), AdeError> {
    if branch.is_empty()
        || branch.len() > 128
        || branch.starts_with('-')
        || branch.contains("..")
        || branch.contains([' ', '\\', '\0', '~', '^', ':', '?', '*', '['])
    {
        return Err(AdeError::Other(format!(
            "refusing unsafe git branch/ref '{branch}'"
        )));
    }
    Ok(())
}

fn ensure_git_repo(root: &Path) -> Result<(), AdeError> {
    let output = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(root)
        .output()
        .map_err(|error| AdeError::Other(format!("failed to invoke git: {error}")))?;
    if !output.status.success() {
        return Err(AdeError::Other(format!(
            "not a git repository: {}",
            root.display()
        )));
    }
    Ok(())
}

fn git_output(root: &Path, args: &[&str]) -> Result<String, AdeError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| AdeError::Other(format!("failed to invoke git: {error}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(AdeError::Other(if stderr.is_empty() {
            format!("git {:?} failed", args)
        } else {
            stderr
        }));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn canonicalize_worktree_path(root: &Path, path: &Path) -> Result<PathBuf, AdeError> {
    if path.as_os_str().is_empty() {
        return Err(AdeError::Other("worktree path is required".into()));
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let normalized = absolute.components().collect::<PathBuf>();
    if normalized
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(AdeError::Authorization(
            "refusing worktree path that escapes via ..".into(),
        ));
    }
    Ok(normalized)
}

fn same_worktree_path(left: &Path, right: &Path) -> bool {
    if normalize_path_key(left) == normalize_path_key(right) {
        return true;
    }
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(a), Ok(b)) => normalize_path_key(&a) == normalize_path_key(&b),
        _ => false,
    }
}

fn normalize_path_key(path: &Path) -> String {
    let mut key = path.to_string_lossy().replace('\\', "/");
    while key.ends_with('/') && key.len() > 1 {
        key.pop();
    }
    // Git on Windows may emit different drive-letter casing than PathBuf.
    if cfg!(windows) {
        key.make_ascii_lowercase();
    }
    key
}

fn worktree_is_dirty(path: &Path) -> Result<bool, AdeError> {
    if !path.exists() {
        return Ok(false);
    }
    let output = git_output(path, &["status", "--porcelain"])?;
    Ok(!output.trim().is_empty())
}

fn parse_worktree_list(stdout: &str) -> Vec<WorktreeInfo> {
    let mut items = Vec::new();
    let mut current: Option<WorktreeInfo> = None;
    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(item) = current.take() {
                items.push(item);
            }
            current = Some(WorktreeInfo {
                path: path.to_string(),
                head: None,
                branch: None,
                bare: false,
                locked: false,
                prunable: false,
            });
            continue;
        }
        let Some(item) = current.as_mut() else {
            continue;
        };
        if let Some(head) = line.strip_prefix("HEAD ") {
            item.head = Some(head.to_string());
        } else if let Some(branch) = line.strip_prefix("branch ") {
            item.branch = Some(
                branch
                    .strip_prefix("refs/heads/")
                    .unwrap_or(branch)
                    .to_string(),
            );
        } else if line == "bare" {
            item.bare = true;
        } else if line.starts_with("locked") {
            item.locked = true;
        } else if line == "prunable" || line.starts_with("prunable ") {
            item.prunable = true;
        }
    }
    if let Some(item) = current {
        items.push(item);
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn fixture_repo() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "ade-wt-{}-{}",
            Uuid::new_v4(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        git_output(&root, &["init"]).unwrap();
        git_output(&root, &["config", "user.email", "ade@example.com"]).unwrap();
        git_output(&root, &["config", "user.name", "ADE Test"]).unwrap();
        fs::write(root.join("README.md"), "hello\n").unwrap();
        git_output(&root, &["add", "README.md"]).unwrap();
        git_output(&root, &["commit", "-m", "init"]).unwrap();
        root
    }

    #[test]
    fn lease_compatibility_and_overlap() {
        assert!(LeaseMode::Observe.compatible_with(LeaseMode::Strong));
        assert!(!LeaseMode::Strong.compatible_with(LeaseMode::Cooperative));
        assert!(!LeaseMode::Exclusive.compatible_with(LeaseMode::Observe));
        assert!(paths_overlap("src/api", "src/api/routes.rs"));
        assert!(!paths_overlap("src/api", "src/auth"));
    }

    #[test]
    fn acquires_lists_and_conflicts_on_overlapping_strong_leases() {
        let root = fixture_repo();
        let manager = LeaseManager::new(&root);
        let agent_a = Uuid::new_v4();
        let agent_b = Uuid::new_v4();
        let lease = manager
            .acquire(agent_a, "src/api", LeaseMode::Strong, Duration::hours(1))
            .unwrap();
        assert!(manager
            .list()
            .unwrap()
            .iter()
            .any(|item| item.id == lease.id));
        let conflict = manager
            .acquire(
                agent_b,
                "src/api/handlers.rs",
                LeaseMode::Cooperative,
                Duration::hours(1),
            )
            .unwrap_err();
        assert!(conflict.to_string().contains("lease conflict"));
        assert!(manager.release(&lease.id).unwrap());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn concurrent_strong_leases_only_one_wins() {
        let root = fixture_repo();
        let path = "src/api";
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let manager = LeaseManager::new(&root);
                std::thread::spawn(move || {
                    manager.acquire(
                        Uuid::new_v4(),
                        path,
                        LeaseMode::Strong,
                        Duration::hours(1),
                    )
                })
            })
            .collect();
        let mut ok = 0usize;
        for handle in handles {
            if handle.join().unwrap().is_ok() {
                ok += 1;
            }
        }
        assert_eq!(ok, 1, "exactly one racing Strong lease should succeed");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn protected_paths_require_strong_and_serialize_writers() {
        let root = fixture_repo();
        let manager = LeaseManager::new(&root);
        let err = manager
            .acquire(
                Uuid::new_v4(),
                "Cargo.lock",
                LeaseMode::Cooperative,
                Duration::minutes(5),
            )
            .unwrap_err();
        assert!(err.to_string().contains("protected path"));
        manager
            .acquire(
                Uuid::new_v4(),
                "migrations/001.sql",
                LeaseMode::Strong,
                Duration::minutes(5),
            )
            .unwrap();
        let conflict = manager
            .acquire(
                Uuid::new_v4(),
                "migrations",
                LeaseMode::Exclusive,
                Duration::minutes(5),
            )
            .unwrap_err();
        let message = conflict.to_string();
        assert!(
            message.contains("lease conflict") || message.contains("serialized"),
            "{message}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn renews_only_active_leases_held_by_the_same_agent() {
        let root = fixture_repo();
        let manager = LeaseManager::new(&root);
        let holder = Uuid::new_v4();
        let lease = manager
            .acquire(holder, "src/api", LeaseMode::Strong, Duration::minutes(5))
            .unwrap();

        let renewed = manager
            .renew(holder, &lease.id, Duration::minutes(30))
            .unwrap();
        assert!(renewed.expires_at > lease.expires_at);

        let stranger = Uuid::new_v4();
        assert!(manager
            .renew(stranger, &lease.id, Duration::minutes(30))
            .is_err());

        // Expired leases cannot be revived via renew.
        let registry = LeaseRegistry {
            schema: LEASE_REGISTRY_SCHEMA.into(),
            leases: vec![PathLease {
                id: lease.id.clone(),
                agent_id: holder,
                path: "src/api".into(),
                mode: LeaseMode::Strong,
                created_at: Utc::now() - Duration::hours(2),
                expires_at: Utc::now() - Duration::minutes(1),
                protected: false,
            }],
        };
        manager.save_for_tests(&registry).unwrap();
        assert!(manager
            .renew(holder, &lease.id, Duration::minutes(30))
            .is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_only_writable_leases_for_agent_scope() {
        let root = fixture_repo();
        let manager = LeaseManager::new(&root);
        let agent = Uuid::new_v4();
        manager
            .acquire(
                agent,
                "src/read-only",
                LeaseMode::Observe,
                Duration::minutes(5),
            )
            .unwrap();
        manager
            .acquire(
                agent,
                "src/feature",
                LeaseMode::Strong,
                Duration::minutes(5),
            )
            .unwrap();

        assert_eq!(
            manager.resolve_owned_paths(agent, &[]).unwrap(),
            vec!["src/feature"]
        );
        assert_eq!(
            manager
                .resolve_owned_paths(agent, &["src/feature/api.rs".into()])
                .unwrap(),
            vec!["src/feature/api.rs"]
        );
        assert!(manager
            .resolve_owned_paths(agent, &["src/other.rs".into()])
            .is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn purges_stale_leases() {
        let root = fixture_repo();
        let manager = LeaseManager::new(&root);
        let registry = LeaseRegistry {
            schema: LEASE_REGISTRY_SCHEMA.into(),
            leases: vec![PathLease {
                id: Uuid::new_v4().to_string(),
                agent_id: Uuid::new_v4(),
                path: "src/old".into(),
                mode: LeaseMode::Observe,
                created_at: Utc::now() - Duration::hours(2),
                expires_at: Utc::now() - Duration::minutes(1),
                protected: false,
            }],
        };
        manager.save_for_tests(&registry).unwrap();
        assert_eq!(manager.release_stale().unwrap(), 1);
        assert!(manager.list().unwrap().is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn creates_and_lists_worktrees_and_refuses_dirty_remove() {
        let root = fixture_repo();
        let manager = WorktreeManager::new(&root);
        let wt = root
            .parent()
            .unwrap()
            .join(format!("ade-wt-add-{}", Uuid::new_v4()));
        let info = manager
            .add(&wt, &format!("feature/ade-{}", Uuid::new_v4()), None)
            .unwrap();
        assert!(info.branch.is_some());
        assert!(manager.list().unwrap().len() >= 2);
        fs::write(wt.join("dirty.txt"), "x\n").unwrap();
        let err = manager.remove(&wt, false).unwrap_err();
        assert!(err.to_string().contains("dirty"));
        manager.remove(&wt, true).unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn refuses_unsafe_paths_and_branches() {
        let root = fixture_repo();
        let leases = LeaseManager::new(&root);
        assert!(leases
            .acquire(
                Uuid::new_v4(),
                "../secrets",
                LeaseMode::Observe,
                Duration::minutes(1),
            )
            .is_err());
        let manager = WorktreeManager::new(&root);
        assert!(manager
            .add(Path::new("nested"), "bad branch", None)
            .is_err());
        let _ = fs::remove_dir_all(root);
    }
}
