use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionProfile {
    Daily,
    Plan,
    Ops,
    Review,
    Research,
    Incident,
    Offline,
}

impl SessionProfile {
    pub fn mcp_count(&self) -> usize {
        match self {
            Self::Daily => 2,
            Self::Plan => 0,
            Self::Ops => 5,
            Self::Review => 0,
            Self::Research => 0,
            Self::Incident => 5,
            Self::Offline => 0,
        }
    }

    pub fn model_tier(&self) -> &'static str {
        match self {
            Self::Daily => "fast",
            Self::Plan => "strong",
            Self::Ops => "strong+confirm",
            Self::Review => "independent",
            Self::Research => "cheap",
            Self::Incident => "strong+human",
            Self::Offline => "local",
        }
    }
}
