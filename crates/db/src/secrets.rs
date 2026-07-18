pub struct SecretsVault;

impl SecretsVault {
    pub fn new() -> Self {
        Self
    }

    pub fn get(&self, _key: &str) -> Result<Option<String>, String> {
        // TODO: implement OS keychain-backed encrypted vault
        Ok(None)
    }

    pub fn set(&self, _key: &str, _value: &str) -> Result<(), String> {
        // TODO: implement
        Ok(())
    }

    pub fn delete(&self, _key: &str) -> Result<(), String> {
        // TODO: implement
        Ok(())
    }
}
