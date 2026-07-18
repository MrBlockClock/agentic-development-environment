pub struct ContextBudget {
    pub always_on_tokens: u32,
    pub rules_tokens: u32,
    pub skills_tokens: u32,
    pub mcp_servers: u32,
}

impl ContextBudget {
    pub fn default_daily() -> Self {
        Self {
            always_on_tokens: 200,
            rules_tokens: 800,
            skills_tokens: 6_000,
            mcp_servers: 2,
        }
    }

    pub fn check(&self) -> ContextStatus {
        // TODO: measure actual usage and compare
        ContextStatus::Green
    }
}

pub enum ContextStatus {
    Green,
    Warning,
    Critical,
}
