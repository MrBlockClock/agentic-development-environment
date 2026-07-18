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

    pub fn load(&self, _manifest: &str) -> Result<(), String> {
        // TODO: load MCP-based plugin from manifest
        Ok(())
    }
}
