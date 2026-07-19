use crate::registry::PluginDescriptor;
use crate::sandbox::PluginPermissions;
use ade_core::error::AdeError;
use serde_json::Value;
use std::collections::HashMap;
use wasmtime::{Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};

/// Stable v1 guest ABI:
///
/// - export `memory`
/// - export `ade_alloc(len: i32) -> i32`
/// - export `ade_invoke(ptr: i32, len: i32) -> i64`
///
/// `ade_invoke` returns `(output_ptr << 32) | output_len`. Input and output are
/// UTF-8 JSON. The host provides zero imports, so guests have no ambient WASI,
/// filesystem, environment, clock, random, or network capability.
pub struct WasmPluginHost {
    engine: Engine,
    plugins: HashMap<String, LoadedPlugin>,
}

struct LoadedPlugin {
    module: Module,
    permissions: PluginPermissions,
}

struct StoreState {
    limits: StoreLimits,
}

impl WasmPluginHost {
    pub fn new() -> Result<Self, AdeError> {
        let mut config = wasmtime::Config::new();
        config.consume_fuel(true);
        // Guest traps are returned as errors; disabling WASM backtrace capture
        // avoids crossing platform unwind boundaries in the embedded host.
        config.wasm_backtrace_max_frames(None);
        let engine = Engine::new(&config).map_err(plugin_error)?;
        Ok(Self {
            engine,
            plugins: HashMap::new(),
        })
    }

    pub fn load(&mut self, descriptor: &PluginDescriptor) -> Result<String, AdeError> {
        descriptor.manifest.validate()?;
        if !descriptor.manifest.enabled {
            return Err(AdeError::Plugin(format!(
                "plugin '{}' is disabled; enable it explicitly in plugin.json",
                descriptor.manifest.id
            )));
        }
        let module = Module::from_file(&self.engine, &descriptor.module_path)
            .map_err(|error| plugin_error_with_id(&descriptor.manifest.id, error))?;
        if let Some(import) = module.imports().next() {
            return Err(AdeError::Plugin(format!(
                "plugin '{}' imports '{}::{}'; v1 plugins must be capability-free",
                descriptor.manifest.id,
                import.module(),
                import.name()
            )));
        }
        for required in ["memory", "ade_alloc", "ade_invoke"] {
            if module.get_export(required).is_none() {
                return Err(AdeError::Plugin(format!(
                    "plugin '{}' does not export required ABI item '{required}'",
                    descriptor.manifest.id
                )));
            }
        }
        let id = descriptor.manifest.id.clone();
        if self
            .plugins
            .insert(
                id.clone(),
                LoadedPlugin {
                    module,
                    permissions: descriptor.manifest.permissions.clone(),
                },
            )
            .is_some()
        {
            return Err(AdeError::Plugin(format!("plugin '{id}' is already loaded")));
        }
        Ok(id)
    }

    pub fn loaded_ids(&self) -> Vec<String> {
        let mut ids = self.plugins.keys().cloned().collect::<Vec<_>>();
        ids.sort();
        ids
    }

    pub fn invoke(&self, id: &str, input: &Value) -> Result<Value, AdeError> {
        let plugin = self
            .plugins
            .get(id)
            .ok_or_else(|| AdeError::NotFound(format!("loaded plugin '{id}'")))?;
        let input = serde_json::to_vec(input)?;
        if input.len() > plugin.permissions.max_input_bytes {
            return Err(AdeError::Plugin(format!(
                "plugin '{id}' input is {} bytes; limit is {}",
                input.len(),
                plugin.permissions.max_input_bytes
            )));
        }

        let limits = StoreLimitsBuilder::new()
            .memory_size(plugin.permissions.max_memory_bytes)
            .instances(1)
            .memories(1)
            .tables(2)
            .build();
        let mut store = Store::new(&self.engine, StoreState { limits });
        store.limiter(|state| &mut state.limits);
        store
            .set_fuel(plugin.permissions.max_fuel)
            .map_err(|error| plugin_error_with_id(id, error))?;
        let linker = Linker::new(&self.engine);
        let instance = linker
            .instantiate(&mut store, &plugin.module)
            .map_err(|error| plugin_error_with_id(id, error))?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| AdeError::Plugin(format!("plugin '{id}' memory export is invalid")))?;
        let alloc = instance
            .get_typed_func::<i32, i32>(&mut store, "ade_alloc")
            .map_err(|error| plugin_error_with_id(id, error))?;
        let invoke = instance
            .get_typed_func::<(i32, i32), i64>(&mut store, "ade_invoke")
            .map_err(|error| plugin_error_with_id(id, error))?;

        let input_len = i32::try_from(input.len())
            .map_err(|_| AdeError::Plugin(format!("plugin '{id}' input is too large")))?;
        let input_ptr = alloc
            .call(&mut store, input_len)
            .map_err(|error| plugin_error_with_id(id, error))?;
        let input_offset = nonnegative_offset(id, "input", input_ptr)?;
        memory
            .write(&mut store, input_offset, &input)
            .map_err(|error| plugin_error_with_id(id, error))?;

        let packed = invoke
            .call(&mut store, (input_ptr, input_len))
            .map_err(|error| plugin_error_with_id(id, error))? as u64;
        let output_offset = (packed >> 32) as u32 as usize;
        let output_len = (packed & u32::MAX as u64) as usize;
        if output_len > plugin.permissions.max_output_bytes {
            return Err(AdeError::Plugin(format!(
                "plugin '{id}' output is {output_len} bytes; limit is {}",
                plugin.permissions.max_output_bytes
            )));
        }
        let output_end = output_offset
            .checked_add(output_len)
            .ok_or_else(|| AdeError::Plugin(format!("plugin '{id}' output range overflowed")))?;
        if output_end > memory.data_size(&store) {
            return Err(AdeError::Plugin(format!(
                "plugin '{id}' returned output outside guest memory"
            )));
        }
        let mut output = vec![0; output_len];
        memory
            .read(&store, output_offset, &mut output)
            .map_err(|error| plugin_error_with_id(id, error))?;
        let text = std::str::from_utf8(&output).map_err(|error| {
            AdeError::Plugin(format!("plugin '{id}' returned non-UTF-8: {error}"))
        })?;
        serde_json::from_str(text).map_err(|error| {
            AdeError::Plugin(format!("plugin '{id}' returned invalid JSON: {error}"))
        })
    }
}

fn nonnegative_offset(id: &str, label: &str, pointer: i32) -> Result<usize, AdeError> {
    usize::try_from(pointer)
        .map_err(|_| AdeError::Plugin(format!("plugin '{id}' returned a negative {label} pointer")))
}

fn plugin_error(error: impl std::fmt::Display) -> AdeError {
    AdeError::Plugin(error.to_string())
}

fn plugin_error_with_id(id: &str, error: impl std::fmt::Display) -> AdeError {
    AdeError::Plugin(format!("plugin '{id}': {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{PluginManifest, PLUGIN_MANIFEST_SCHEMA};
    use crate::registry::PluginDescriptor;
    use std::path::PathBuf;
    use uuid::Uuid;

    const ECHO_WAT: &str = r#"
        (module
          (memory (export "memory") 1)
          (global $heap (mut i32) (i32.const 1024))
          (func (export "ade_alloc") (param $len i32) (result i32)
            (local $ptr i32)
            (local.set $ptr (global.get $heap))
            (global.set $heap (i32.add (global.get $heap) (local.get $len)))
            (local.get $ptr))
          (func (export "ade_invoke") (param $ptr i32) (param $len i32) (result i64)
            (i64.or
              (i64.shl (i64.extend_i32_u (local.get $ptr)) (i64.const 32))
              (i64.extend_i32_u (local.get $len)))))
    "#;

    fn fixture_descriptor(
        wat: &str,
        permissions: PluginPermissions,
    ) -> (PathBuf, PluginDescriptor) {
        let root = std::env::temp_dir().join(format!("ade-wasm-plugin-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let module_path = root.join("plugin.wasm");
        std::fs::write(&module_path, wat::parse_str(wat).unwrap()).unwrap();
        let manifest_path = root.join("plugin.json");
        let manifest = PluginManifest {
            schema: PLUGIN_MANIFEST_SCHEMA.into(),
            id: "example.echo".into(),
            version: "1.0.0".into(),
            entry: "plugin.wasm".into(),
            enabled: true,
            permissions,
        };
        (
            root,
            PluginDescriptor {
                manifest_path,
                module_path,
                manifest,
            },
        )
    }

    #[test]
    fn invokes_capability_free_json_guest() {
        let (root, descriptor) = fixture_descriptor(ECHO_WAT, PluginPermissions::default());
        let mut host = WasmPluginHost::new().unwrap();
        let id = host.load(&descriptor).unwrap();
        let input = serde_json::json!({ "hello": "world" });
        assert_eq!(host.invoke(&id, &input).unwrap(), input);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_disabled_or_importing_plugins() {
        let (root, mut descriptor) = fixture_descriptor(ECHO_WAT, PluginPermissions::default());
        descriptor.manifest.enabled = false;
        assert!(WasmPluginHost::new().unwrap().load(&descriptor).is_err());
        let _ = std::fs::remove_dir_all(root);

        let importing = r#"
            (module
              (import "wasi_snapshot_preview1" "fd_write" (func $write))
              (memory (export "memory") 1)
              (func (export "ade_alloc") (param i32) (result i32) (i32.const 0))
              (func (export "ade_invoke") (param i32 i32) (result i64) (i64.const 0)))
        "#;
        let (root, descriptor) = fixture_descriptor(importing, PluginPermissions::default());
        assert!(WasmPluginHost::new().unwrap().load(&descriptor).is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fuel_stops_non_terminating_guest() {
        let expensive = r#"
            (module
              (memory (export "memory") 1)
              (func (export "ade_alloc") (param i32) (result i32) (i32.const 0))
              (func (export "ade_invoke") (param i32 i32) (result i64)
                (local $i i32)
                (loop $work
                  (local.set $i (i32.add (local.get $i) (i32.const 1)))
                  (br_if $work (i32.lt_u (local.get $i) (i32.const 1000000))))
                (i64.const 0)))
        "#;
        let permissions = PluginPermissions {
            max_fuel: 10_000,
            ..PluginPermissions::default()
        };
        let (root, descriptor) = fixture_descriptor(expensive, permissions);
        let mut host = WasmPluginHost::new().unwrap();
        let id = host.load(&descriptor).unwrap();
        assert!(host.invoke(&id, &serde_json::json!({})).is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}
