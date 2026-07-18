use ade_core::ignore::{IgnoreAlignment, IgnoreStatus, IgnoreSurface};

pub struct IgnoreEnforcer;

impl IgnoreEnforcer {
    pub fn new() -> Self {
        Self
    }

    pub fn check_alignment(&self) -> Vec<IgnoreAlignment> {
        IgnoreSurface::all()
            .iter()
            .map(|s| IgnoreAlignment {
                surface: s.name().to_string(),
                status: IgnoreStatus::Missing,
                missing_patterns: vec![],
            })
            .collect()
    }

    pub fn path_is_blocked(&self, _path: &str) -> bool {
        // TODO: check against secret patterns
        false
    }
}
