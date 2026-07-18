pub struct AuthHandler;

impl AuthHandler {
    pub fn new() -> Self {
        Self
    }

    pub fn validate_session(&self, _token: &str) -> Option<String> {
        // TODO: validate JWT/session token
        None
    }
}
