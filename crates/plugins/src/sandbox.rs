pub struct PluginSandbox;

impl Default for PluginSandbox {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginSandbox {
    pub fn new() -> Self {
        Self
    }

    pub fn restrict_filesystem(&self, _allow_paths: &[&str]) {
        // TODO: WASM sandbox filesystem restrictions
    }

    pub fn restrict_network(&self, _allow: bool) {
        // TODO: WASM sandbox network restrictions
    }
}
