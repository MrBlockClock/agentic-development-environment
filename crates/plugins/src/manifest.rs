use crate::sandbox::PluginPermissions;
use ade_core::error::AdeError;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

pub const PLUGIN_MANIFEST_SCHEMA: &str = "ade.plugin/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    #[default]
    Wasm,
    Mcp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct McpPluginSpec {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub schema: String,
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub kind: PluginKind,
    #[serde(default)]
    pub entry: Option<PathBuf>,
    #[serde(default)]
    pub mcp: Option<McpPluginSpec>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub permissions: PluginPermissions,
    /// Optional declared SHA-256 digest (hex) of the plugin artifact.
    #[serde(default)]
    pub digest: Option<String>,
    /// Optional Ed25519 signature (hex) over `id\\0version\\0digest`.
    #[serde(default)]
    pub signature: Option<String>,
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
        match self.kind {
            PluginKind::Wasm => {
                let entry = self.entry.as_ref().ok_or_else(|| {
                    AdeError::Plugin(format!("wasm plugin '{}' requires entry", self.id))
                })?;
                validate_relative_entry(entry)?;
                if self.mcp.is_some() {
                    return Err(AdeError::Plugin(format!(
                        "wasm plugin '{}' must not declare mcp config",
                        self.id
                    )));
                }
                self.permissions.validate_for_v1()?;
            }
            PluginKind::Mcp => {
                let mcp = self.mcp.as_ref().ok_or_else(|| {
                    AdeError::Plugin(format!("mcp plugin '{}' requires mcp.command", self.id))
                })?;
                if mcp.command.trim().is_empty() {
                    return Err(AdeError::Plugin(format!(
                        "mcp plugin '{}' command must not be empty",
                        self.id
                    )));
                }
                if self.entry.is_some() {
                    return Err(AdeError::Plugin(format!(
                        "mcp plugin '{}' must not declare a wasm entry",
                        self.id
                    )));
                }
                // MCP plugins still cannot request WASM filesystem/network rights.
                self.permissions.validate_for_v1()?;
            }
        }
        Ok(())
    }

    pub fn resolve_entry(&self, manifest_path: &Path) -> Result<PathBuf, AdeError> {
        self.validate()?;
        if self.kind != PluginKind::Wasm {
            return Err(AdeError::Plugin(format!(
                "plugin '{}' is not a wasm plugin",
                self.id
            )));
        }
        let directory = manifest_path.parent().ok_or_else(|| {
            AdeError::Plugin(format!(
                "plugin manifest has no parent directory: {}",
                manifest_path.display()
            ))
        })?;
        let entry = directory.join(self.entry.as_ref().expect("validated wasm entry"));
        if !entry.is_file() {
            return Err(AdeError::Plugin(format!(
                "plugin '{}' entry does not exist: {}",
                self.id,
                entry.display()
            )));
        }
        Ok(entry)
    }

    pub fn artifact_bytes(&self, manifest_path: &Path) -> Result<Vec<u8>, AdeError> {
        match self.kind {
            PluginKind::Wasm => Ok(std::fs::read(self.resolve_entry(manifest_path)?)?),
            PluginKind::Mcp => {
                let mcp = self.mcp.as_ref().expect("validated mcp config");
                Ok(serde_json::to_vec(&serde_json::json!({
                    "command": mcp.command,
                    "args": mcp.args,
                }))?)
            }
        }
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

    fn valid_wasm() -> PluginManifest {
        PluginManifest {
            schema: PLUGIN_MANIFEST_SCHEMA.into(),
            id: "com.example.echo".into(),
            version: "1.0.0".into(),
            kind: PluginKind::Wasm,
            entry: Some("echo.wasm".into()),
            mcp: None,
            enabled: false,
            permissions: PluginPermissions::default(),
            digest: None,
            signature: None,
        }
    }

    #[test]
    fn validates_deny_by_default_manifest() {
        valid_wasm().validate().unwrap();
    }

    #[test]
    fn rejects_unsafe_ids_entries_and_capabilities() {
        let mut manifest = valid_wasm();
        manifest.id = "../escape".into();
        assert!(manifest.validate().is_err());
        manifest = valid_wasm();
        manifest.entry = Some(PathBuf::from("../escape.wasm"));
        assert!(manifest.validate().is_err());
        manifest = valid_wasm();
        manifest.permissions.network = true;
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn validates_mcp_manifests() {
        let manifest = PluginManifest {
            schema: PLUGIN_MANIFEST_SCHEMA.into(),
            id: "com.example.mcp".into(),
            version: "1.0.0".into(),
            kind: PluginKind::Mcp,
            entry: None,
            mcp: Some(McpPluginSpec {
                command: "npx".into(),
                args: vec!["-y".into(), "@modelcontextprotocol/server-memory".into()],
            }),
            enabled: false,
            permissions: PluginPermissions::default(),
            digest: None,
            signature: None,
        };
        manifest.validate().unwrap();
    }
}
