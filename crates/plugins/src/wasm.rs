pub struct WasmPluginHost;

impl WasmPluginHost {
    pub fn new() -> Self {
        Self
    }

    pub fn load_plugin(&self, _path: &str) -> Result<(), String> {
        // TODO: load WASM component via wasmtime
        Ok(())
    }
}
