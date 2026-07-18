pub struct McpHost;

impl Default for McpHost {
    fn default() -> Self {
        Self::new()
    }
}

impl McpHost {
    pub fn new() -> Self {
        Self
    }

    pub async fn connect_server(
        &self,
        _name: &str,
        _command: &str,
        _args: &[&str],
    ) -> Result<(), String> {
        // TODO: connect to MCP server via rmcp
        Ok(())
    }

    pub async fn list_tools(&self) -> Vec<String> {
        vec![]
    }
}
