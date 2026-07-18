pub struct McpPluginLoader;

impl McpPluginLoader {
    pub fn new() -> Self {
        Self
    }

    pub fn load(&self, _manifest: &str) -> Result<(), String> {
        // TODO: load MCP-based plugin from manifest
        Ok(())
    }
}
