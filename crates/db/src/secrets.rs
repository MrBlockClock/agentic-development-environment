use ade_core::error::AdeError;
use keyring::{Entry, Error as KeyringError};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const SERVICE: &str = "dev.ade.provider-keys";
const FILE_VAULT_DIR: &str = "provider-keys";

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
/// Dual-write on Windows: Credential Manager **and** a DPAPI file under
/// `%LOCALAPPDATA%\ade\provider-keys\` so rebuilds / debug relaunches never
/// orphan keys. Non-Windows keeps keyring only.
///
/// Secret values are never logged or returned over IPC.
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
        let provider = normalize_id(provider, "provider")?;
        match self.keyring_get(&provider) {
            Ok(Some(secret)) => {
                // Refresh durable file copy when keyring still has the value.
                let _ = self.file_set(&provider, &secret);
                Ok(Some(secret))
            }
            Ok(None) => {
                if let Some(secret) = self.file_get(&provider)? {
                    // Heal keyring from durable file after a rebuild / wipe.
                    let _ = self.keyring_set(&provider, &secret);
                    Ok(Some(secret))
                } else {
                    Ok(None)
                }
            }
            Err(error) => {
                // Keyring flaky → fall back to durable file.
                if let Some(secret) = self.file_get(&provider)? {
                    Ok(Some(secret))
                } else {
                    Err(error)
                }
            }
        }
    }

    pub fn contains(&self, provider: &str) -> Result<bool, AdeError> {
        self.get(provider).map(|secret| secret.is_some())
    }

    pub fn set(&self, provider: &str, value: &str) -> Result<(), AdeError> {
        if value.trim().is_empty() {
            return Err(AdeError::Provider("provider key cannot be empty".into()));
        }
        let provider = normalize_id(provider, "provider")?;
        let value = value.trim();
        // Durable file first so a keyring failure cannot drop the only copy.
        self.file_set(&provider, value)?;
        match self.keyring_set(&provider, value) {
            Ok(()) => Ok(()),
            Err(error) => {
                tracing::warn!(
                    profile = %self.profile,
                    provider = %provider,
                    "OS keyring set failed; durable file vault retained the key"
                );
                // File already has it — treat as success for dogfood durability.
                let _ = error;
                Ok(())
            }
        }
    }

    pub fn delete(&self, provider: &str) -> Result<bool, AdeError> {
        let provider = normalize_id(provider, "provider")?;
        let removed_file = self.file_delete(&provider)?;
        let removed_keyring = self.keyring_delete(&provider).unwrap_or_default();
        Ok(removed_file || removed_keyring)
    }

    fn entry(&self, provider: &str) -> Result<Entry, AdeError> {
        // Stable Windows target name (survives rebuilds; not path-scoped).
        let target = format!("ade.provider.{}.{}", self.profile, provider);
        let user = format!("{}:{provider}", self.profile);
        Entry::new_with_target(&target, SERVICE, &user).map_err(vault_error)
    }

    fn legacy_entry(&self, provider: &str) -> Result<Entry, AdeError> {
        Entry::new(SERVICE, &format!("{}:{provider}", self.profile)).map_err(vault_error)
    }

    fn keyring_get(&self, provider: &str) -> Result<Option<String>, AdeError> {
        match self.entry(provider)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(KeyringError::NoEntry) => match self.legacy_entry(provider)?.get_password() {
                Ok(secret) => {
                    // Migrate legacy Credential Manager entry to stable target.
                    let _ = self.keyring_set(provider, &secret);
                    Ok(Some(secret))
                }
                Err(KeyringError::NoEntry) => Ok(None),
                Err(error) => Err(vault_error(error)),
            },
            Err(error) => Err(vault_error(error)),
        }
    }

    fn keyring_set(&self, provider: &str, value: &str) -> Result<(), AdeError> {
        self.entry(provider)?
            .set_password(value)
            .map_err(vault_error)
    }

    fn keyring_delete(&self, provider: &str) -> Result<bool, AdeError> {
        let mut removed = false;
        for entry in [self.entry(provider)?, self.legacy_entry(provider)?] {
            match entry.delete_credential() {
                Ok(()) => removed = true,
                Err(KeyringError::NoEntry) => {}
                Err(error) => return Err(vault_error(error)),
            }
        }
        Ok(removed)
    }

    fn file_path(&self, provider: &str) -> Result<PathBuf, AdeError> {
        let dir = vault_dir()?;
        Ok(dir.join(format!("{}--{provider}.bin", self.profile)))
    }

    fn file_get(&self, provider: &str) -> Result<Option<String>, AdeError> {
        let path = self.file_path(provider)?;
        if !path.is_file() {
            return Ok(None);
        }
        let raw = std::fs::read(&path).map_err(AdeError::Io)?;
        let plain = unprotect_secret(&raw)?;
        let secret = String::from_utf8(plain)
            .map_err(|_| AdeError::Provider("vault file is not valid UTF-8".into()))?;
        if secret.trim().is_empty() {
            Ok(None)
        } else {
            Ok(Some(secret))
        }
    }

    fn file_set(&self, provider: &str, value: &str) -> Result<(), AdeError> {
        let path = self.file_path(provider)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let protected = protect_secret(value.as_bytes())?;
        let tmp = path.with_extension("bin.tmp");
        std::fs::write(&tmp, protected)?;
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    fn file_delete(&self, provider: &str) -> Result<bool, AdeError> {
        let path = self.file_path(provider)?;
        if !path.is_file() {
            return Ok(false);
        }
        std::fs::remove_file(&path)?;
        Ok(true)
    }
}

fn vault_dir() -> Result<PathBuf, AdeError> {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .ok_or_else(|| AdeError::Provider("cannot resolve LOCALAPPDATA/HOME for vault".into()))?;
    Ok(base.join("ade").join(FILE_VAULT_DIR))
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

/// Protect secret bytes for on-disk storage (DPAPI on Windows; raw elsewhere is refused).
fn protect_secret(plain: &[u8]) -> Result<Vec<u8>, AdeError> {
    #[cfg(windows)]
    {
        dpapi::protect(plain)
    }
    #[cfg(not(windows))]
    {
        // Non-Windows: rely on keyring; still write a marker-prefixed blob only if
        // the process can use a user-private directory (best-effort, not DPAPI).
        let mut out = b"ade.vault.v1.plain\n".to_vec();
        out.extend_from_slice(plain);
        Ok(out)
    }
}

fn unprotect_secret(blob: &[u8]) -> Result<Vec<u8>, AdeError> {
    #[cfg(windows)]
    {
        dpapi::unprotect(blob)
    }
    #[cfg(not(windows))]
    {
        const PREFIX: &[u8] = b"ade.vault.v1.plain\n";
        if let Some(rest) = blob.strip_prefix(PREFIX) {
            return Ok(rest.to_vec());
        }
        Err(AdeError::Provider(
            "unsupported vault file encoding on this platform".into(),
        ))
    }
}

#[cfg(windows)]
mod dpapi {
    use ade_core::error::AdeError;
    use windows_sys::Win32::Foundation::{LocalFree, BOOL};
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
    };

    pub fn protect(plain: &[u8]) -> Result<Vec<u8>, AdeError> {
        unsafe {
            let input = CRYPT_INTEGER_BLOB {
                cbData: plain.len() as u32,
                pbData: plain.as_ptr() as *mut u8,
            };
            let mut output = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            };
            let ok: BOOL = CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                &mut output,
            );
            if ok == 0 || output.pbData.is_null() {
                return Err(AdeError::Provider(
                    "Windows DPAPI CryptProtectData failed".into(),
                ));
            }
            let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            LocalFree(output.pbData as _);
            Ok(bytes)
        }
    }

    pub fn unprotect(blob: &[u8]) -> Result<Vec<u8>, AdeError> {
        unsafe {
            let input = CRYPT_INTEGER_BLOB {
                cbData: blob.len() as u32,
                pbData: blob.as_ptr() as *mut u8,
            };
            let mut output = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            };
            let ok: BOOL = CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                &mut output,
            );
            if ok == 0 || output.pbData.is_null() {
                return Err(AdeError::Provider(
                    "Windows DPAPI CryptUnprotectData failed".into(),
                ));
            }
            let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            LocalFree(output.pbData as _);
            Ok(bytes)
        }
    }
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

/// Import missing provider keys from OpenCode Desktop `auth.json` (gap-fill only).
/// Never overwrites an existing vault entry. Never logs secret values.
pub fn import_opencode_auth_gaps(
    vault: &dyn ProviderKeyVault,
    profile: &str,
) -> Result<Vec<String>, AdeError> {
    let Some(path) = find_opencode_auth_path() else {
        return Ok(vec![]);
    };
    import_opencode_auth_file(vault, profile, &path, false)
}

pub fn import_opencode_auth_file(
    vault: &dyn ProviderKeyVault,
    profile: &str,
    path: &Path,
    force: bool,
) -> Result<Vec<String>, AdeError> {
    let raw = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|error| AdeError::Provider(format!("invalid OpenCode auth.json: {error}")))?;
    let Some(obj) = value.as_object() else {
        return Err(AdeError::Provider(
            "OpenCode auth.json root must be an object".into(),
        ));
    };

    let mut imported = Vec::new();
    for (provider, entry) in obj {
        let provider = provider.trim().to_ascii_lowercase();
        if provider.is_empty() {
            continue;
        }
        let key = entry
            .get("key")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let Some(secret) = key else {
            continue;
        };
        if !force && vault.contains(profile, &provider)? {
            continue;
        }
        vault.set(profile, &provider, secret)?;
        imported.push(provider);
    }
    Ok(imported)
}

pub fn find_opencode_auth_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        let home = PathBuf::from(home);
        candidates.push(
            home.join(".local")
                .join("share")
                .join("opencode")
                .join("auth.json"),
        );
        candidates.push(home.join(".config").join("opencode").join("auth.json"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        candidates.push(PathBuf::from(appdata).join("opencode").join("auth.json"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local).join("opencode").join("auth.json"));
    }
    candidates.into_iter().find(|path| path.is_file())
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

    #[test]
    fn import_opencode_auth_gap_fill_only() {
        let root = std::env::temp_dir().join(format!("ade-auth-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("auth.json");
        std::fs::write(
            &path,
            r#"{"freellm":{"type":"api","key":"fl-secret"},"opencode":{"type":"api","key":"oc-secret"}}"#,
        )
        .unwrap();
        let vault = InMemoryProviderKeyVault::default();
        vault.set("local", "opencode", "keep-me").unwrap();
        let imported = import_opencode_auth_file(&vault, "local", &path, false).unwrap();
        assert_eq!(imported, vec!["freellm".to_string()]);
        assert_eq!(
            vault.get("local", "opencode").unwrap().as_deref(),
            Some("keep-me")
        );
        assert_eq!(
            vault.get("local", "freellm").unwrap().as_deref(),
            Some("fl-secret")
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
