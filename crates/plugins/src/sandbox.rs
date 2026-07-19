use ade_core::error::AdeError;
use serde::{Deserialize, Serialize};

fn default_fuel() -> u64 {
    10_000_000
}

fn default_memory() -> usize {
    16 * 1024 * 1024
}

fn default_payload() -> usize {
    1024 * 1024
}

/// Capabilities and resource ceilings requested by one plugin.
///
/// The v1 host intentionally supports no filesystem or network capability.
/// Manifests requesting either are rejected rather than silently weakened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PluginPermissions {
    pub filesystem: Vec<String>,
    pub network: bool,
    #[serde(default = "default_fuel")]
    pub max_fuel: u64,
    #[serde(default = "default_memory")]
    pub max_memory_bytes: usize,
    #[serde(default = "default_payload")]
    pub max_input_bytes: usize,
    #[serde(default = "default_payload")]
    pub max_output_bytes: usize,
}

impl Default for PluginPermissions {
    fn default() -> Self {
        Self {
            filesystem: vec![],
            network: false,
            max_fuel: default_fuel(),
            max_memory_bytes: default_memory(),
            max_input_bytes: default_payload(),
            max_output_bytes: default_payload(),
        }
    }
}

impl PluginPermissions {
    pub fn validate_for_v1(&self) -> Result<(), AdeError> {
        if !self.filesystem.is_empty() {
            return Err(AdeError::Plugin(
                "filesystem capability is not supported by the v1 plugin host".into(),
            ));
        }
        if self.network {
            return Err(AdeError::Plugin(
                "network capability is not supported by the v1 plugin host".into(),
            ));
        }
        if self.max_fuel == 0 {
            return Err(AdeError::Plugin("max_fuel must be positive".into()));
        }
        if self.max_memory_bytes < 64 * 1024 {
            return Err(AdeError::Plugin(
                "max_memory_bytes must allow at least one WASM page (65536 bytes)".into(),
            ));
        }
        if self.max_input_bytes == 0 || self.max_output_bytes == 0 {
            return Err(AdeError::Plugin(
                "plugin payload limits must be positive".into(),
            ));
        }
        Ok(())
    }
}
