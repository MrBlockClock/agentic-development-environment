use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerifyGate {
    G0, // Golden path probe
    G1, // Contract present
    G2, // Lint/types/format
    G3, // Unit tests
    G4, // Integration/health
    G5, // Browser/hardware evidence
}

impl VerifyGate {
    pub fn name(&self) -> &'static str {
        match self {
            Self::G0 => "G0: Golden path probe",
            Self::G1 => "G1: Contract present",
            Self::G2 => "G2: Lint/types/format",
            Self::G3 => "G3: Unit tests",
            Self::G4 => "G4: Integration/health",
            Self::G5 => "G5: Browser/hardware evidence",
        }
    }

    pub fn all() -> Vec<VerifyGate> {
        vec![
            Self::G0, Self::G1, Self::G2, Self::G3, Self::G4, Self::G5,
        ]
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyResult {
    pub gate: String,
    pub command: String,
    pub exit_code: Option<i32>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub passed: bool,
}
