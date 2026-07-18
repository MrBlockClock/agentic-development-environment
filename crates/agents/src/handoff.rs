use ade_core::error::AdeError;
use ade_core::handoff::HandoffCapsule;
use ade_core::ignore::SensitivePathPolicy;
use std::path::{Path, PathBuf};

pub struct HandoffManager {
    root: PathBuf,
}

impl Default for HandoffManager {
    fn default() -> Self {
        Self::new(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }
}

impl HandoffManager {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Atomically writes an immutable capsule and refreshes `latest.json`.
    /// Returns the generated capsule id.
    pub fn save_capsule(&self, capsule: &HandoffCapsule) -> Result<String, AdeError> {
        validate_capsule(capsule)?;
        let directory = self.root.join(".ade").join("handoff");
        std::fs::create_dir_all(&directory)?;
        let id = uuid::Uuid::new_v4().to_string();
        let payload = serde_json::to_vec_pretty(capsule)?;
        let filename = id.clone() + ".json";
        write_atomic(&directory.join(filename), &payload)?;
        write_atomic(&directory.join("latest.json"), &payload)?;
        Ok(id)
    }

    pub fn load_capsule(&self, id: &str) -> Result<HandoffCapsule, AdeError> {
        if id != "latest"
            && (id.len() > 64
                || id.is_empty()
                || !id
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-'))
        {
            return Err(AdeError::NotFound("invalid handoff capsule id".into()));
        }
        let path = self
            .root
            .join(".ade")
            .join("handoff")
            .join(id)
            .with_extension("json");
        let payload = std::fs::read(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                AdeError::NotFound("handoff capsule does not exist".into())
            } else {
                AdeError::Io(error)
            }
        })?;
        let capsule: HandoffCapsule = serde_json::from_slice(&payload)?;
        validate_capsule(&capsule)?;
        Ok(capsule)
    }

    pub fn load_latest(&self) -> Result<HandoffCapsule, AdeError> {
        self.load_capsule("latest")
    }
}

fn write_atomic(path: &Path, payload: &[u8]) -> Result<(), AdeError> {
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, payload)?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    std::fs::rename(temporary, path)?;
    Ok(())
}

fn validate_capsule(capsule: &HandoffCapsule) -> Result<(), AdeError> {
    if capsule.schema != ade_core::handoff::HANDOFF_SCHEMA {
        return Err(AdeError::Other("unsupported handoff schema".into()));
    }
    let text_fields = [
        capsule.goal.as_str(),
        capsule.mode.as_str(),
        capsule.orchestrating_ade.as_str(),
        capsule.branch.as_deref().unwrap_or(""),
        capsule.next_safe_command.as_deref().unwrap_or(""),
        capsule.compact_summary.as_deref().unwrap_or(""),
        capsule.provider.as_deref().unwrap_or(""),
        capsule.model.as_deref().unwrap_or(""),
        capsule.turn_status.as_deref().unwrap_or(""),
    ];
    for field in text_fields {
        if field
            .split_whitespace()
            .any(SensitivePathPolicy::is_secret_path)
            || SensitivePathPolicy::is_secret_path(field)
        {
            return Err(AdeError::Authorization(
                "handoff capsules may not contain secret paths in text fields".into(),
            ));
        }
    }
    if capsule
        .blockers
        .iter()
        .any(|item| SensitivePathPolicy::path_is_blocked(item))
    {
        return Err(AdeError::Authorization(
            "handoff capsules may not contain secret paths in blockers".into(),
        ));
    }
    if capsule
        .changed_paths
        .iter()
        .any(|path| SensitivePathPolicy::path_is_blocked(path))
    {
        return Err(AdeError::Authorization(
            "handoff capsules may not contain secret or always-ignored paths".into(),
        ));
    }
    if capsule
        .decisions_touched
        .iter()
        .any(|path| SensitivePathPolicy::path_is_blocked(path))
    {
        return Err(AdeError::Authorization(
            "handoff capsules may not contain secret paths in decisions".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!("ade-handoff-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn saves_immutable_and_latest_capsules() {
        let root = fixture();
        let manager = HandoffManager::new(&root);
        let capsule = HandoffCapsule::new("continue work", "evaluate_existing");
        let id = manager.save_capsule(&capsule).unwrap();
        assert_eq!(manager.load_capsule(&id).unwrap().goal, "continue work");
        assert_eq!(manager.load_latest().unwrap().schema, "ade.handoff/v1");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn refuses_secret_paths_and_traversal_ids() {
        let root = fixture();
        let manager = HandoffManager::new(&root);
        let mut capsule = HandoffCapsule::new("continue work", "execute");
        capsule.changed_paths.push(".env".into());
        assert!(manager.save_capsule(&capsule).is_err());
        assert!(manager.load_capsule("../secret").is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}
