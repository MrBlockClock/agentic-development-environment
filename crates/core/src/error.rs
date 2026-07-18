use thiserror::Error;

#[derive(Error, Debug)]
pub enum AdeError {
    #[error("Phase routing error: {0}")]
    PhaseRouting(String),

    #[error("Audit failed: {0}")]
    Audit(String),

    #[error("Plan validation: {0}")]
    PlanValidation(String),

    #[error("Execution error: {0}")]
    Execution(String),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Spend cap error: {0}")]
    Spend(String),

    #[error("Cancelled: {0}")]
    Cancelled(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("MCP error: {0}")]
    Mcp(String),

    #[error("Plugin error: {0}")]
    Plugin(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Authorization error: {0}")]
    Authorization(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}
