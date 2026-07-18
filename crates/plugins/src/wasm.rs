pub struct WasmPluginHost;

impl Default for WasmPluginHost {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmPluginHost {
    pub fn new() -> Self {
        Self
    }

    pub fn load_plugin(&self, _path: &str) -> Result<(), String> {
        // TODO: load WASM component via wasmtime
        Ok(())
    }
}
