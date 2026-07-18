use ade_core::authority::AuthorityLevel;

pub struct AuthorityEnforcer;

impl Default for AuthorityEnforcer {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthorityEnforcer {
    pub fn new() -> Self {
        Self
    }

    pub fn check_conflict(&self, existing: &AuthorityLevel, incoming: &AuthorityLevel) -> bool {
        incoming.priority() >= existing.priority()
    }

    pub fn resolve_and_report(&self, conflict: (&AuthorityLevel, &AuthorityLevel)) -> String {
        let (existing, incoming) = conflict;
        let winner = AuthorityLevel::resolve(existing, incoming);
        format!(
            "Authority conflict: {:?} vs {:?} — {:?} wins",
            existing, incoming, winner
        )
    }
}
