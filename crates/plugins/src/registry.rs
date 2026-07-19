use crate::manifest::{PluginKind, PluginManifest};
use ade_core::error::AdeError;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct PluginDescriptor {
    pub manifest_path: PathBuf,
    pub module_path: Option<PathBuf>,
    pub manifest: PluginManifest,
}

pub struct PluginRegistry {
    pub dirs: Vec<PathBuf>,
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self { dirs: vec![] }
    }

    pub fn from_workspace(root: impl AsRef<Path>) -> Self {
        Self {
            dirs: vec![root.as_ref().join(".ade").join("plugins")],
        }
    }

    pub fn with_dir(mut self, directory: impl Into<PathBuf>) -> Self {
        self.dirs.push(directory.into());
        self
    }

    /// Discover strict `plugin.json` manifests one directory below each
    /// registry root. Duplicate ids are rejected to prevent shadowing.
    pub fn discover(&self) -> Result<Vec<PluginDescriptor>, AdeError> {
        let mut plugins = Vec::new();
        let mut ids = HashSet::new();
        for directory in &self.dirs {
            if !directory.is_dir() {
                continue;
            }
            let mut entries = std::fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                if !entry.file_type()?.is_dir() {
                    continue;
                }
                let manifest_path = entry.path().join("plugin.json");
                if !manifest_path.is_file() {
                    continue;
                }
                let manifest = PluginManifest::load(&manifest_path)?;
                if !ids.insert(manifest.id.clone()) {
                    return Err(AdeError::Plugin(format!(
                        "duplicate plugin id '{}'",
                        manifest.id
                    )));
                }
                let module_path = match manifest.kind {
                    PluginKind::Wasm => Some(manifest.resolve_entry(&manifest_path)?),
                    PluginKind::Mcp => None,
                };
                plugins.push(PluginDescriptor {
                    manifest_path,
                    module_path,
                    manifest,
                });
            }
        }
        plugins.sort_by(|left, right| left.manifest.id.cmp(&right.manifest.id));
        Ok(plugins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::PLUGIN_MANIFEST_SCHEMA;
    use uuid::Uuid;

    #[test]
    fn discovers_manifests_and_refuses_duplicate_ids() {
        let root = std::env::temp_dir().join(format!("ade-plugin-registry-{}", Uuid::new_v4()));
        let first = root.join("first");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::write(first.join("plugin.wasm"), b"\0asm\x01\0\0\0").unwrap();
        std::fs::write(
            first.join("plugin.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": PLUGIN_MANIFEST_SCHEMA,
                "id": "example.echo",
                "version": "1.0.0",
                "entry": "plugin.wasm"
            }))
            .unwrap(),
        )
        .unwrap();
        let registry = PluginRegistry::new().with_dir(&root);
        let plugins = registry.discover().unwrap();
        assert_eq!(plugins.len(), 1);
        assert!(!plugins[0].manifest.enabled);

        let second = root.join("second");
        std::fs::create_dir_all(&second).unwrap();
        std::fs::copy(first.join("plugin.wasm"), second.join("plugin.wasm")).unwrap();
        std::fs::copy(first.join("plugin.json"), second.join("plugin.json")).unwrap();
        assert!(registry.discover().is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}
