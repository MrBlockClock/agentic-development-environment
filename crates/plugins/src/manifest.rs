use crate::sandbox::PluginPermissions;
use ade_core::error::AdeError;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

pub const PLUGIN_MANIFEST_SCHEMA: &str = "ade.plugin/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub schema: String,
    pub id: String,
    pub version: String,
    pub entry: PathBuf,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub permissions: PluginPermissions,
}

impl PluginManifest {
    pub fn load(path: &Path) -> Result<Self, AdeError> {
        let manifest: Self = serde_json::from_slice(&std::fs::read(path)?)
            .map_err(|error| AdeError::Plugin(format!("invalid {}: {error}", path.display())))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), AdeError> {
        if self.schema != PLUGIN_MANIFEST_SCHEMA {
            return Err(AdeError::Plugin(format!(
                "unsupported plugin manifest schema '{}'",
                self.schema
            )));
        }
        validate_plugin_id(&self.id)?;
        if self.version.trim().is_empty() || self.version.len() > 64 {
            return Err(AdeError::Plugin(
                "plugin version must contain 1-64 characters".into(),
            ));
        }
        validate_relative_entry(&self.entry)?;
        self.permissions.validate_for_v1()
    }

    pub fn resolve_entry(&self, manifest_path: &Path) -> Result<PathBuf, AdeError> {
        self.validate()?;
        let directory = manifest_path.parent().ok_or_else(|| {
            AdeError::Plugin(format!(
                "plugin manifest has no parent directory: {}",
                manifest_path.display()
            ))
        })?;
        let entry = directory.join(&self.entry);
        if !entry.is_file() {
            return Err(AdeError::Plugin(format!(
                "plugin '{}' entry does not exist: {}",
                self.id,
                entry.display()
            )));
        }
        Ok(entry)
    }
}

fn validate_plugin_id(id: &str) -> Result<(), AdeError> {
    let valid = !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if valid {
        Ok(())
    } else {
        Err(AdeError::Plugin(
            "plugin id must be 1-64 ASCII letters, digits, '.', '_' or '-'".into(),
        ))
    }
}

fn validate_relative_entry(entry: &Path) -> Result<(), AdeError> {
    if entry.as_os_str().is_empty()
        || entry.is_absolute()
        || entry
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || entry.extension().and_then(|value| value.to_str()) != Some("wasm")
    {
        return Err(AdeError::Plugin(
            "plugin entry must be a relative traversal-free .wasm path".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> PluginManifest {
        PluginManifest {
            schema: PLUGIN_MANIFEST_SCHEMA.into(),
            id: "com.example.echo".into(),
            version: "1.0.0".into(),
            entry: "echo.wasm".into(),
            enabled: false,
            permissions: PluginPermissions::default(),
        }
    }

    #[test]
    fn validates_deny_by_default_manifest() {
        valid().validate().unwrap();
    }

    #[test]
    fn rejects_unsafe_ids_entries_and_capabilities() {
        let mut manifest = valid();
        manifest.id = "../escape".into();
        assert!(manifest.validate().is_err());
        manifest = valid();
        manifest.entry = PathBuf::from("../escape.wasm");
        assert!(manifest.validate().is_err());
        manifest = valid();
        manifest.permissions.network = true;
        assert!(manifest.validate().is_err());
    }
}
