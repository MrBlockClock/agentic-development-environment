use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub const VERIFY_SCHEMA: &str = "ade.verify.results/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerifyGate {
    G0, // Golden path probe
    G1, // Contract present
    G2, // Lint/types/format
    G3, // Unit tests
    G4, // Integration/health
    G5, // Browser/hardware evidence
}

impl VerifyGate {
    pub fn id(self) -> &'static str {
        match self {
            Self::G0 => "G0",
            Self::G1 => "G1",
            Self::G2 => "G2",
            Self::G3 => "G3",
            Self::G4 => "G4",
            Self::G5 => "G5",
        }
    }

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
        vec![Self::G0, Self::G1, Self::G2, Self::G3, Self::G4, Self::G5]
    }
}

impl FromStr for VerifyGate {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_uppercase().as_str() {
            "G0" | "0" => Ok(Self::G0),
            "G1" | "1" => Ok(Self::G1),
            "G2" | "2" => Ok(Self::G2),
            "G3" | "3" => Ok(Self::G3),
            "G4" | "4" => Ok(Self::G4),
            "G5" | "5" => Ok(Self::G5),
            other => Err(format!(
                "unknown verify gate '{other}' (expected G0, G1, G2, G3, G4, or G5)"
            )),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gate_ids_case_insensitively() {
        assert_eq!("g2".parse::<VerifyGate>().unwrap(), VerifyGate::G2);
        assert_eq!("5".parse::<VerifyGate>().unwrap(), VerifyGate::G5);
        assert!("G9".parse::<VerifyGate>().is_err());
    }
}
