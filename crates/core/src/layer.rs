use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdLayer {
    L0, // Hardware
    L1, // OS & Shell
    L2, // Canonical runtime
    L3, // ADE portfolio
    L4, // Project brain
    L5, // Context hygiene
    L6, // Tools & MCP
    L7, // Providers & models
    L8, // Quality gates
    L9, // Verification
    L10, // Continuity
    L11, // Governance
}

impl AdLayer {
    pub fn name(&self) -> &'static str {
        match self {
            Self::L0 => "Hardware",
            Self::L1 => "OS & Shell",
            Self::L2 => "Canonical runtime",
            Self::L3 => "ADE portfolio",
            Self::L4 => "Project brain",
            Self::L5 => "Context hygiene",
            Self::L6 => "Tools & MCP",
            Self::L7 => "Providers & models",
            Self::L8 => "Quality gates",
            Self::L9 => "Verification",
            Self::L10 => "Continuity",
            Self::L11 => "Governance",
        }
    }

    pub fn all() -> Vec<AdLayer> {
        vec![
            Self::L0, Self::L1, Self::L2, Self::L3, Self::L4, Self::L5,
            Self::L6, Self::L7, Self::L8, Self::L9, Self::L10, Self::L11,
        ]
    }
}
