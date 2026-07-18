use ade_core::error::AdeError;
use keyring::{Entry, Error as KeyringError};

const SERVICE: &str = "dev.ade.provider-keys";

/// OS-native credential vault for BYOK provider keys.
///
/// Secret values are never serialized, logged, or persisted in the ADE
/// database. The profile and provider id form the keychain account name.
pub struct SecretsVault {
    profile: String,
}

impl Default for SecretsVault {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretsVault {
    pub fn new() -> Self {
        Self {
            profile: "default".into(),
        }
    }

    pub fn for_profile(profile: impl Into<String>) -> Result<Self, AdeError> {
        let profile = normalize_id(&profile.into(), "profile")?;
        Ok(Self { profile })
    }

    pub fn get(&self, provider: &str) -> Result<Option<String>, AdeError> {
        let entry = self.entry(provider)?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(vault_error(error)),
        }
    }

    pub fn contains(&self, provider: &str) -> Result<bool, AdeError> {
        self.get(provider).map(|secret| secret.is_some())
    }

    pub fn set(&self, provider: &str, value: &str) -> Result<(), AdeError> {
        if value.trim().is_empty() {
            return Err(AdeError::Provider("provider key cannot be empty".into()));
        }
        self.entry(provider)?
            .set_password(value)
            .map_err(vault_error)
    }

    pub fn delete(&self, provider: &str) -> Result<bool, AdeError> {
        match self.entry(provider)?.delete_credential() {
            Ok(()) => Ok(true),
            Err(KeyringError::NoEntry) => Ok(false),
            Err(error) => Err(vault_error(error)),
        }
    }

    fn entry(&self, provider: &str) -> Result<Entry, AdeError> {
        let provider = normalize_id(provider, "provider")?;
        Entry::new(SERVICE, &format!("{}:{provider}", self.profile)).map_err(vault_error)
    }
}

fn normalize_id(value: &str, label: &str) -> Result<String, AdeError> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || normalized.len() > 64
        || !normalized
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err(AdeError::Provider(format!(
            "invalid {label} id '{value}' (use 1-64 letters, digits, '.', '-', or '_')"
        )));
    }
    Ok(normalized)
}

fn vault_error(error: KeyringError) -> AdeError {
    AdeError::Provider(format!("OS credential vault error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_safe_identifiers() {
        assert_eq!(normalize_id(" OpenAI ", "provider").unwrap(), "openai");
        assert_eq!(
            normalize_id("azure.openai-1", "provider").unwrap(),
            "azure.openai-1"
        );
    }

    #[test]
    fn rejects_unsafe_identifiers() {
        assert!(normalize_id("", "provider").is_err());
        assert!(normalize_id("open ai", "provider").is_err());
        assert!(normalize_id("provider/key", "provider").is_err());
    }

    #[test]
    fn rejects_empty_secret_before_keychain_access() {
        let vault = SecretsVault::new();
        assert!(vault.set("openai", "   ").is_err());
    }
}
