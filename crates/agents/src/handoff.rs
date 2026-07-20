use ade_core::error::AdeError;
use ade_core::handoff::HandoffCapsule;
use ade_core::ignore::SensitivePathPolicy;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct HandoffMetrics {
    pub capsule_count: u64,
    pub invalid_capsule_count: u64,
    pub total_bytes: u64,
    pub latest_bytes: u64,
    pub latest_summary_chars: u64,
    pub latest_compaction_percent: u32,
    pub latest_score_before: Option<u32>,
    pub latest_score_after: Option<u32>,
    pub latest_score_max: Option<u32>,
    pub latest_score_delta: Option<i64>,
    pub latest_status: Option<String>,
    pub latest_created_at: Option<String>,
    pub latest_context_status: Option<String>,
    pub latest_context_tokens: Option<u32>,
    pub latest_context_sections: Vec<ade_core::handoff::HandoffPromptSection>,
    pub recent: Vec<HandoffHistoryItem>,
}

/// Safe continuity history entry — no capsule goal/summary text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HandoffHistoryItem {
    pub id: String,
    pub created_at: Option<String>,
    pub turn_status: Option<String>,
    pub score_before: Option<u32>,
    pub score_after: Option<u32>,
    pub score_max: Option<u32>,
    pub score_delta: Option<i64>,
    pub context_status: Option<String>,
    pub context_tokens: Option<u32>,
}

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

    /// Returns newest immutable capsules first (skips `latest.json`).
    pub fn list_recent(&self, limit: usize) -> Result<Vec<(String, HandoffCapsule)>, AdeError> {
        let directory = self.root.join(".ade").join("handoff");
        if !directory.is_dir() || limit == 0 {
            return Ok(Vec::new());
        }

        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            let Some(id) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            if path.extension().and_then(|value| value.to_str()) != Some("json") || id == "latest" {
                continue;
            }
            let modified = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .ok()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            entries.push((modified, id.to_string(), path));
        }
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.0));

        let mut capsules = Vec::new();
        for (_, id, path) in entries.into_iter().take(limit) {
            let payload = std::fs::read(&path)?;
            let capsule: HandoffCapsule = serde_json::from_slice(&payload)?;
            validate_capsule(&capsule)?;
            capsules.push((id, capsule));
        }
        Ok(capsules)
    }

    /// Returns aggregate continuity health without exposing capsule text.
    pub fn metrics(&self) -> Result<HandoffMetrics, AdeError> {
        let directory = self.root.join(".ade").join("handoff");
        if !directory.is_dir() {
            return Ok(HandoffMetrics::default());
        }

        let mut metrics = HandoffMetrics::default();
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            let is_capsule = path.extension().and_then(|value| value.to_str()) == Some("json")
                && path.file_stem().and_then(|value| value.to_str()) != Some("latest");
            if !is_capsule {
                continue;
            }

            let payload = std::fs::read(&path)?;
            metrics.total_bytes = metrics.total_bytes.saturating_add(payload.len() as u64);
            match serde_json::from_slice::<HandoffCapsule>(&payload) {
                Ok(capsule) if validate_capsule(&capsule).is_ok() => {
                    metrics.capsule_count = metrics.capsule_count.saturating_add(1);
                }
                _ => {
                    metrics.invalid_capsule_count = metrics.invalid_capsule_count.saturating_add(1);
                }
            }
        }

        let latest_path = directory.join("latest.json");
        if latest_path.is_file() {
            let payload = std::fs::read(&latest_path)?;
            match serde_json::from_slice::<HandoffCapsule>(&payload) {
                Ok(capsule) if validate_capsule(&capsule).is_ok() => {
                    let summary_chars = capsule
                        .compact_summary
                        .as_deref()
                        .map(|summary| summary.chars().count() as u64)
                        .unwrap_or(0);
                    let latest_bytes = payload.len() as u64;
                    metrics.latest_bytes = latest_bytes;
                    metrics.latest_summary_chars = summary_chars;
                    let retained = summary_chars
                        .saturating_mul(100)
                        .checked_div(latest_bytes)
                        .unwrap_or(0);
                    metrics.latest_compaction_percent =
                        100_u32.saturating_sub(retained.min(100) as u32);
                    metrics.latest_score_before = capsule.score_before;
                    metrics.latest_score_after = capsule.score_after;
                    metrics.latest_score_max = capsule.score_max;
                    metrics.latest_score_delta = capsule
                        .score_before
                        .zip(capsule.score_after)
                        .map(|(before, after)| i64::from(after) - i64::from(before));
                    metrics.latest_status = capsule.turn_status.clone();
                    metrics.latest_created_at = capsule.created_at.clone();
                    if let Some(context) = &capsule.context_compaction {
                        metrics.latest_context_status = Some(context.status.clone());
                        metrics.latest_context_tokens = Some(context.tokens_estimated);
                        metrics.latest_context_sections = context.sections.clone();
                    }
                }
                _ => {
                    metrics.invalid_capsule_count = metrics.invalid_capsule_count.saturating_add(1);
                }
            }
        }

        metrics.recent = self
            .list_recent(8)?
            .into_iter()
            .map(|(id, capsule)| HandoffHistoryItem {
                id,
                created_at: capsule.created_at,
                turn_status: capsule.turn_status,
                score_before: capsule.score_before,
                score_after: capsule.score_after,
                score_max: capsule.score_max,
                score_delta: capsule
                    .score_before
                    .zip(capsule.score_after)
                    .map(|(before, after)| i64::from(after) - i64::from(before)),
                context_status: capsule
                    .context_compaction
                    .as_ref()
                    .map(|item| item.status.clone()),
                context_tokens: capsule
                    .context_compaction
                    .as_ref()
                    .map(|item| item.tokens_estimated),
            })
            .collect();

        Ok(metrics)
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

    #[test]
    fn reports_compaction_and_score_delta_without_capsule_text() {
        let root = fixture();
        let manager = HandoffManager::new(&root);
        let mut capsule = HandoffCapsule::new("continue private implementation details", "execute");
        capsule.score_before = Some(60);
        capsule.score_after = Some(75);
        capsule.score_max = Some(100);
        capsule.turn_status = Some("completed".into());
        capsule.context_compaction = Some(ade_core::handoff::HandoffContextCompaction {
            tokens_estimated: 420,
            status: "green".into(),
            sections: vec![ade_core::handoff::HandoffPromptSection {
                name: "authority".into(),
                tokens: 200,
                truncated: false,
            }],
        });
        capsule.compact_summary = Some(capsule.prompt_summary(120));
        manager.save_capsule(&capsule).unwrap();

        let metrics = manager.metrics().unwrap();
        assert_eq!(metrics.capsule_count, 1);
        assert_eq!(metrics.invalid_capsule_count, 0);
        assert!(metrics.total_bytes > 0);
        assert!(metrics.latest_summary_chars > 0);
        assert_eq!(metrics.latest_score_delta, Some(15));
        assert_eq!(metrics.latest_score_max, Some(100));
        assert_eq!(metrics.latest_context_status.as_deref(), Some("green"));
        assert_eq!(metrics.latest_status.as_deref(), Some("completed"));
        assert_eq!(metrics.recent.len(), 1);
        assert!(!serde_json::to_string(&metrics)
            .unwrap()
            .contains("private implementation details"));

        let recent = manager.list_recent(3).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].1.score_max, Some(100));
        let _ = std::fs::remove_dir_all(root);
    }
}
