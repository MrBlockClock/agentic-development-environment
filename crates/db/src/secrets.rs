use ade_core::error::AdeError;
use keyring::{Entry, Error as KeyringError};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

const SERVICE: &str = "dev.ade.provider-keys";

/// Injectable provider-key source. Frontends may query status and mutate keys,
/// but must never expose `get` over IPC.
pub trait ProviderKeyVault: Send + Sync {
    fn get(&self, profile: &str, provider: &str) -> Result<Option<String>, AdeError>;
    fn contains(&self, profile: &str, provider: &str) -> Result<bool, AdeError> {
        self.get(profile, provider).map(|secret| secret.is_some())
    }
    fn set(&self, profile: &str, provider: &str, value: &str) -> Result<(), AdeError>;
    fn delete(&self, profile: &str, provider: &str) -> Result<bool, AdeError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NativeProviderKeyVault;

impl ProviderKeyVault for NativeProviderKeyVault {
    fn get(&self, profile: &str, provider: &str) -> Result<Option<String>, AdeError> {
        SecretsVault::for_profile(profile)?.get(provider)
    }

    fn set(&self, profile: &str, provider: &str, value: &str) -> Result<(), AdeError> {
        SecretsVault::for_profile(profile)?.set(provider, value)
    }

    fn delete(&self, profile: &str, provider: &str) -> Result<bool, AdeError> {
        SecretsVault::for_profile(profile)?.delete(provider)
    }
}

/// Process-local fake for deterministic tests. Never used by production setup.
#[derive(Debug, Default, Clone)]
pub struct InMemoryProviderKeyVault {
    values: Arc<Mutex<BTreeMap<String, String>>>,
}

impl ProviderKeyVault for InMemoryProviderKeyVault {
    fn get(&self, profile: &str, provider: &str) -> Result<Option<String>, AdeError> {
        let account = account_id(profile, provider)?;
        Ok(self
            .values
            .lock()
            .map_err(|_| AdeError::Provider("in-memory vault lock poisoned".into()))?
            .get(&account)
            .cloned())
    }

    fn set(&self, profile: &str, provider: &str, value: &str) -> Result<(), AdeError> {
        if value.trim().is_empty() {
            return Err(AdeError::Provider("provider key cannot be empty".into()));
        }
        let account = account_id(profile, provider)?;
        self.values
            .lock()
            .map_err(|_| AdeError::Provider("in-memory vault lock poisoned".into()))?
            .insert(account, value.to_string());
        Ok(())
    }

    fn delete(&self, profile: &str, provider: &str) -> Result<bool, AdeError> {
        let account = account_id(profile, provider)?;
        Ok(self
            .values
            .lock()
            .map_err(|_| AdeError::Provider("in-memory vault lock poisoned".into()))?
            .remove(&account)
            .is_some())
    }
}

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

fn account_id(profile: &str, provider: &str) -> Result<String, AdeError> {
    Ok(format!(
        "{}:{}",
        normalize_id(profile, "profile")?,
        normalize_id(provider, "provider")?
    ))
}

fn vault_error(error: KeyringError) -> AdeError {
    AdeError::Provider(format!("OS credential vault error: {error}"))
}

/// When `ADE_IMPORT_ENV_KEYS=1` (or `true`), copy common provider env vars into the
/// OS keychain for the given profile. Never logs secret values.
///
/// Supported:
/// - `OPENAI_API_KEY` → provider `openai`
/// - `ANTHROPIC_API_KEY` → provider `anthropic`
/// - `ADE_<PROVIDER>_API_KEY` → provider `<provider>` (lowercased)
///
/// Existing vault entries are left unchanged unless `ADE_IMPORT_ENV_KEYS=force`.
pub fn import_env_provider_keys(
    vault: &dyn ProviderKeyVault,
    profile: &str,
) -> Result<Vec<String>, AdeError> {
    let mode = std::env::var("ADE_IMPORT_ENV_KEYS")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if mode.is_empty() || mode == "0" || mode == "false" || mode == "no" {
        return Ok(vec![]);
    }
    let force = mode == "force";

    let mut imported = Vec::new();
    let mut candidates: Vec<(String, String)> = Vec::new();

    if let Ok(value) = std::env::var("OPENAI_API_KEY") {
        if !value.trim().is_empty() {
            candidates.push(("openai".into(), value));
        }
    }
    if let Ok(value) = std::env::var("ANTHROPIC_API_KEY") {
        if !value.trim().is_empty() {
            candidates.push(("anthropic".into(), value));
        }
    }
    for (key, value) in std::env::vars() {
        let Some(rest) = key.strip_prefix("ADE_") else {
            continue;
        };
        let Some(provider) = rest.strip_suffix("_API_KEY") else {
            continue;
        };
        if provider.is_empty() || value.trim().is_empty() {
            continue;
        }
        candidates.push((provider.to_ascii_lowercase(), value));
    }

    for (provider, value) in candidates {
        if !force && vault.contains(profile, &provider)? {
            continue;
        }
        vault.set(profile, &provider, value.trim())?;
        imported.push(provider);
    }
    Ok(imported)
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

    #[test]
    fn in_memory_vault_supports_profile_isolation() {
        let vault = InMemoryProviderKeyVault::default();
        vault.set("local", "openai", "secret-a").unwrap();
        assert!(vault.contains("local", "openai").unwrap());
        assert!(!vault.contains("staging", "openai").unwrap());
        assert_eq!(
            vault.get("local", "openai").unwrap().as_deref(),
            Some("secret-a")
        );
        assert!(vault.delete("local", "openai").unwrap());
        assert!(!vault.contains("local", "openai").unwrap());
    }

    #[test]
    fn import_env_keys_respects_flag_and_skips_existing() {
        std::env::remove_var("ADE_IMPORT_ENV_KEYS");
        std::env::set_var("OPENAI_API_KEY", "sk-test-openai");
        let vault = InMemoryProviderKeyVault::default();
        assert!(import_env_provider_keys(&vault, "local")
            .unwrap()
            .is_empty());

        std::env::set_var("ADE_IMPORT_ENV_KEYS", "1");
        let imported = import_env_provider_keys(&vault, "local").unwrap();
        assert_eq!(imported, vec!["openai".to_string()]);
        assert_eq!(
            vault.get("local", "openai").unwrap().as_deref(),
            Some("sk-test-openai")
        );

        std::env::set_var("OPENAI_API_KEY", "sk-other");
        let again = import_env_provider_keys(&vault, "local").unwrap();
        assert!(again.is_empty());
        assert_eq!(
            vault.get("local", "openai").unwrap().as_deref(),
            Some("sk-test-openai")
        );

        std::env::set_var("ADE_IMPORT_ENV_KEYS", "force");
        let forced = import_env_provider_keys(&vault, "local").unwrap();
        assert_eq!(forced, vec!["openai".to_string()]);
        assert_eq!(
            vault.get("local", "openai").unwrap().as_deref(),
            Some("sk-other")
        );

        std::env::remove_var("ADE_IMPORT_ENV_KEYS");
        std::env::remove_var("OPENAI_API_KEY");
    }
}
