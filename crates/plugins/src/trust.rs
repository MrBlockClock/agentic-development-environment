use ade_core::error::AdeError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const TRUST_SCHEMA: &str = "ade.plugin.trust/v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustEntry {
    pub plugin_id: String,
    /// Optional pinned SHA-256 digest (hex). When set, the artifact must match.
    #[serde(default)]
    pub digest: Option<String>,
    /// Optional Ed25519 verifying key (hex). Required when manifests carry signatures.
    #[serde(default)]
    pub pubkey: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrustRegistry {
    schema: String,
    entries: Vec<TrustEntry>,
}

pub struct PluginTrustStore {
    path: PathBuf,
}

impl PluginTrustStore {
    pub fn from_workspace(root: impl AsRef<Path>) -> Self {
        Self {
            path: root
                .as_ref()
                .join(".ade")
                .join("plugins")
                .join("trust.json"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn list(&self) -> Result<Vec<TrustEntry>, AdeError> {
        Ok(self.load()?.entries)
    }

    pub fn get(&self, plugin_id: &str) -> Result<Option<TrustEntry>, AdeError> {
        Ok(self
            .load()?
            .entries
            .into_iter()
            .find(|entry| entry.plugin_id == plugin_id))
    }

    pub fn trust(&self, entry: TrustEntry) -> Result<TrustEntry, AdeError> {
        if entry.plugin_id.trim().is_empty() {
            return Err(AdeError::Plugin("trust entry requires plugin_id".into()));
        }
        if let Some(digest) = &entry.digest {
            validate_hex(digest, 32, "digest")?;
        }
        if let Some(pubkey) = &entry.pubkey {
            validate_hex(pubkey, 32, "pubkey")?;
        }
        let mut registry = self.load()?;
        if let Some(existing) = registry
            .entries
            .iter_mut()
            .find(|candidate| candidate.plugin_id == entry.plugin_id)
        {
            *existing = entry.clone();
        } else {
            registry.entries.push(entry.clone());
        }
        registry
            .entries
            .sort_by(|left, right| left.plugin_id.cmp(&right.plugin_id));
        self.save(&registry)?;
        Ok(entry)
    }

    pub fn revoke(&self, plugin_id: &str) -> Result<bool, AdeError> {
        let mut registry = self.load()?;
        let before = registry.entries.len();
        registry
            .entries
            .retain(|entry| entry.plugin_id != plugin_id);
        let removed = registry.entries.len() != before;
        if removed {
            self.save(&registry)?;
        }
        Ok(removed)
    }

    fn load(&self) -> Result<TrustRegistry, AdeError> {
        if !self.path.is_file() {
            return Ok(TrustRegistry {
                schema: TRUST_SCHEMA.into(),
                entries: vec![],
            });
        }
        let registry: TrustRegistry = serde_json::from_slice(&std::fs::read(&self.path)?)?;
        if registry.schema != TRUST_SCHEMA {
            return Err(AdeError::Plugin("unsupported plugin trust schema".into()));
        }
        Ok(registry)
    }

    fn save(&self, registry: &TrustRegistry) -> Result<(), AdeError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = self.path.with_extension("tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(registry)?)?;
        #[cfg(windows)]
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
        }
        std::fs::rename(temporary, &self.path)?;
        Ok(())
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

pub fn verify_artifact(
    plugin_id: &str,
    version: &str,
    artifact_bytes: &[u8],
    declared_digest: Option<&str>,
    signature_hex: Option<&str>,
    trust: &TrustEntry,
) -> Result<String, AdeError> {
    if trust.plugin_id != plugin_id {
        return Err(AdeError::Plugin(format!(
            "trust entry is for '{}', not '{plugin_id}'",
            trust.plugin_id
        )));
    }
    let digest = sha256_hex(artifact_bytes);
    if let Some(expected) = trust.digest.as_deref().or(declared_digest) {
        if !constant_time_eq_hex(expected, &digest) {
            return Err(AdeError::Plugin(format!(
                "plugin '{plugin_id}' digest mismatch"
            )));
        }
    }
    if let Some(signature_hex) = signature_hex {
        let Some(pubkey_hex) = trust.pubkey.as_deref() else {
            return Err(AdeError::Plugin(format!(
                "plugin '{plugin_id}' is signed but no trusted pubkey is configured"
            )));
        };
        verify_signature(plugin_id, version, &digest, signature_hex, pubkey_hex)?;
    }
    Ok(digest)
}

fn verify_signature(
    plugin_id: &str,
    version: &str,
    digest: &str,
    signature_hex: &str,
    pubkey_hex: &str,
) -> Result<(), AdeError> {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

    let pubkey_bytes = decode_hex(pubkey_hex, 32, "pubkey")?;
    let signature_bytes = decode_hex(signature_hex, 64, "signature")?;
    let verifying_key = VerifyingKey::from_bytes(
        pubkey_bytes
            .as_slice()
            .try_into()
            .map_err(|_| AdeError::Plugin("invalid ed25519 pubkey length".into()))?,
    )
    .map_err(|error| AdeError::Plugin(format!("invalid ed25519 pubkey: {error}")))?;
    let signature = Signature::from_bytes(
        signature_bytes
            .as_slice()
            .try_into()
            .map_err(|_| AdeError::Plugin("invalid ed25519 signature length".into()))?,
    );
    let message = signing_message(plugin_id, version, digest);
    verifying_key.verify(&message, &signature).map_err(|_| {
        AdeError::Plugin(format!(
            "plugin '{plugin_id}' signature verification failed"
        ))
    })
}

pub fn signing_message(plugin_id: &str, version: &str, digest: &str) -> Vec<u8> {
    format!("{plugin_id}\0{version}\0{digest}").into_bytes()
}

fn validate_hex(value: &str, expected_bytes: usize, label: &str) -> Result<(), AdeError> {
    decode_hex(value, expected_bytes, label).map(|_| ())
}

fn decode_hex(value: &str, expected_bytes: usize, label: &str) -> Result<Vec<u8>, AdeError> {
    let bytes = hex::decode(value.trim())
        .map_err(|error| AdeError::Plugin(format!("invalid {label} hex: {error}")))?;
    if bytes.len() != expected_bytes {
        return Err(AdeError::Plugin(format!(
            "{label} must be {} hex bytes",
            expected_bytes
        )));
    }
    Ok(bytes)
}

fn constant_time_eq_hex(left: &str, right: &str) -> bool {
    let left = left.trim().as_bytes();
    let right = right.trim().as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (&left, &right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use uuid::Uuid;

    #[test]
    fn trusts_and_verifies_signed_digest() {
        let root = std::env::temp_dir().join(format!("ade-trust-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join(".ade").join("plugins")).unwrap();
        let store = PluginTrustStore::from_workspace(&root);
        let signing = SigningKey::from_bytes(&[7_u8; 32]);
        let verifying = signing.verifying_key();
        let artifact = b"wasm-bytes";
        let digest = sha256_hex(artifact);
        let message = signing_message("example.echo", "1.0.0", &digest);
        let signature = signing.sign(&message);

        store
            .trust(TrustEntry {
                plugin_id: "example.echo".into(),
                digest: Some(digest.clone()),
                pubkey: Some(hex::encode(verifying.as_bytes())),
            })
            .unwrap();
        let entry = store.get("example.echo").unwrap().unwrap();
        verify_artifact(
            "example.echo",
            "1.0.0",
            artifact,
            Some(&digest),
            Some(&hex::encode(signature.to_bytes())),
            &entry,
        )
        .unwrap();
        assert!(verify_artifact(
            "example.echo",
            "1.0.0",
            b"tampered",
            Some(&digest),
            None,
            &entry,
        )
        .is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}
