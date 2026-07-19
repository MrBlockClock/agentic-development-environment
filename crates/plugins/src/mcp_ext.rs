use crate::manifest::{PluginKind, PluginManifest};
use crate::registry::PluginDescriptor;
use ade_core::error::AdeError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// MCP server configuration produced from a trusted MCP plugin manifest.
///
/// This intentionally mirrors the agents MCP host shape without depending on
/// `ade-agents`, so the CLI can approve+connect after trust checks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustedMcpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub approved: bool,
}

pub struct McpPluginLoader;

impl Default for McpPluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl McpPluginLoader {
    pub fn new() -> Self {
        Self
    }

    /// Convert a discovered MCP plugin into an approved MCP server config.
    ///
    /// Callers must have already verified trust and obtained human approval for
    /// connecting the resulting process.
    pub fn load(&self, descriptor: &PluginDescriptor) -> Result<TrustedMcpServerConfig, AdeError> {
        descriptor.manifest.validate()?;
        if descriptor.manifest.kind != PluginKind::Mcp {
            return Err(AdeError::Plugin(format!(
                "plugin '{}' is not an MCP plugin",
                descriptor.manifest.id
            )));
        }
        if !descriptor.manifest.enabled {
            return Err(AdeError::Plugin(format!(
                "plugin '{}' is disabled; enable it explicitly in plugin.json",
                descriptor.manifest.id
            )));
        }
        let mcp = descriptor
            .manifest
            .mcp
            .as_ref()
            .ok_or_else(|| AdeError::Plugin("mcp plugin is missing command config".into()))?;
        Ok(TrustedMcpServerConfig {
            name: descriptor.manifest.id.clone(),
            command: mcp.command.clone(),
            args: mcp.args.clone(),
            approved: true,
        })
    }

    pub fn from_manifest(
        &self,
        manifest: &PluginManifest,
    ) -> Result<TrustedMcpServerConfig, AdeError> {
        self.load(&PluginDescriptor {
            manifest_path: PathBuf::from("plugin.json"),
            module_path: None,
            manifest: manifest.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{McpPluginSpec, PLUGIN_MANIFEST_SCHEMA};
    use crate::sandbox::PluginPermissions;

    #[test]
    fn loads_enabled_mcp_plugin_as_approved_config() {
        let manifest = PluginManifest {
            schema: PLUGIN_MANIFEST_SCHEMA.into(),
            id: "example.memory".into(),
            version: "1.0.0".into(),
            kind: PluginKind::Mcp,
            entry: None,
            mcp: Some(McpPluginSpec {
                command: "npx".into(),
                args: vec!["-y".into(), "demo".into()],
            }),
            enabled: true,
            permissions: PluginPermissions::default(),
            digest: None,
            signature: None,
        };
        let config = McpPluginLoader::new().from_manifest(&manifest).unwrap();
        assert!(config.approved);
        assert_eq!(config.name, "example.memory");
        assert_eq!(config.command, "npx");
    }

    #[test]
    fn refuses_disabled_mcp_plugin() {
        let mut manifest = PluginManifest {
            schema: PLUGIN_MANIFEST_SCHEMA.into(),
            id: "example.memory".into(),
            version: "1.0.0".into(),
            kind: PluginKind::Mcp,
            entry: None,
            mcp: Some(McpPluginSpec {
                command: "npx".into(),
                args: vec![],
            }),
            enabled: false,
            permissions: PluginPermissions::default(),
            digest: None,
            signature: None,
        };
        assert!(McpPluginLoader::new().from_manifest(&manifest).is_err());
        manifest.enabled = true;
        manifest.kind = PluginKind::Wasm;
        manifest.entry = Some("x.wasm".into());
        manifest.mcp = None;
        assert!(McpPluginLoader::new().from_manifest(&manifest).is_err());
    }
}
