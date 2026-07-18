use ade_core::handoff::HandoffCapsule;

pub struct HandoffManager;

impl Default for HandoffManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HandoffManager {
    pub fn new() -> Self {
        Self
    }

    pub fn save_capsule(&self, _capsule: &HandoffCapsule) -> Result<(), String> {
        // TODO: write to .ade/handoff/
        Ok(())
    }

    pub fn load_capsule(&self, _id: &str) -> Option<HandoffCapsule> {
        None
    }
}
