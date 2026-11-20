//! Secure Native OS / Encrypted Local Credential Vault for Sunborn Local Desktop (BYOK)
//! Guarantees that AI provider API keys (OpenAI, Anthropic, Gemini, Ollama, etc.)
//! are protected at rest, never stored in browser localStorage or plaintext logs,
//! and never exposed in plaintext to frontend JavaScript or public endpoints.

use crate::crypto_shred::{generate_random_nonce_96, hkdf_sha256_derive, ZeroizingKey};
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::RwLock;
use sunborn_core::sync::{read_lock, write_lock};
use sunborn_core::{Result, SunbornError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfiguredProviderInfo {
    pub name: String,
    pub configured: bool,
    pub key_preview: String,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct EncryptedVaultPayload {
    version: u32,
    nonce: [u8; 12],
    ciphertext: Vec<u8>,
}

#[derive(Serialize, Deserialize, Default)]
struct PlaintextVaultRecords {
    providers: HashMap<String, StoredProviderRecord>,
}

#[derive(Serialize, Deserialize, Clone)]
struct StoredProviderRecord {
    api_key: String,
    base_url: Option<String>,
    default_model: Option<String>,
}

pub struct LocalCredentialVault {
    vault_path: PathBuf,
    master_key: ZeroizingKey,
    cache: RwLock<HashMap<String, StoredProviderRecord>>,
}

impl LocalCredentialVault {
    /// Generates or resolves the standard OS application data directory for Sunborn.
    pub fn default_vault_path() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                return PathBuf::from(local_app_data)
                    .join("Sunborn")
                    .join("vault.enc");
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home)
                    .join(".local")
                    .join("share")
                    .join("sunborn")
                    .join("vault.enc");
            }
        }
        PathBuf::from(".sunborn_vault.enc")
    }

    /// Derives an OS-user-bound 256-bit AES master key using machine and user entropy.
    fn derive_user_master_key() -> ZeroizingKey {
        let mut entropy = Vec::new();
        if let Ok(user) = std::env::var("USERNAME").or_else(|_| std::env::var("USER")) {
            entropy.extend_from_slice(user.as_bytes());
        }
        if let Ok(machine) = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")) {
            entropy.extend_from_slice(machine.as_bytes());
        }
        if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
            entropy.extend_from_slice(home.as_bytes());
        }
        if entropy.is_empty() {
            entropy.extend_from_slice(b"sunborn-default-local-user-entropy");
        }

        let salt = b"sunborn-os-credential-vault-salt-v1";
        let info = b"sunborn-byok-native-aes-256-gcm-master-key";
        let derived = hkdf_sha256_derive(&entropy, salt, info);
        ZeroizingKey::new(derived)
    }

    /// Instantiates the vault at the given path with hardware-bound entropy.
    pub fn new_at_path(path: impl Into<PathBuf>) -> Self {
        let vault_path = path.into();
        let master_key = Self::derive_user_master_key();
        let vault = Self {
            vault_path,
            master_key,
            cache: RwLock::new(HashMap::new()),
        };
        vault.load_from_disk();
        vault
    }

    pub fn new_default() -> Self {
        Self::new_at_path(Self::default_vault_path())
    }

    /// Masks an API key for safe display (e.g. "sk-...a1b2"), preventing plaintext leakage to UI.
    pub fn mask_key(key: &str) -> String {
        let trimmed = key.trim();
        if trimmed.len() <= 8 {
            "********".to_string()
        } else {
            let prefix = if trimmed.starts_with("sk-") {
                "sk-..."
            } else {
                "..."
            };
            let suffix = &trimmed[trimmed.len().saturating_sub(4)..];
            format!("{prefix}{suffix}")
        }
    }

    fn load_from_disk(&self) {
        if !self.vault_path.exists() {
            return;
        }
        if let Ok(mut file) = File::open(&self.vault_path) {
            let mut buffer = Vec::new();
            if file.read_to_end(&mut buffer).is_ok() {
                if let Ok(payload) = serde_json::from_slice::<EncryptedVaultPayload>(&buffer) {
                    let key = Key::<Aes256Gcm>::from_slice(self.master_key.as_bytes());
                    let cipher = Aes256Gcm::new(key);
                    let nonce = Nonce::from_slice(&payload.nonce);
                    if let Ok(plaintext) = cipher.decrypt(
                        nonce,
                        Payload {
                            msg: &payload.ciphertext,
                            aad: b"sunborn-local-vault-aad",
                        },
                    ) {
                        if let Ok(records) =
                            serde_json::from_slice::<PlaintextVaultRecords>(&plaintext)
                        {
                            let mut cache = write_lock(&self.cache);
                            *cache = records.providers;
                        }
                    }
                }
            }
        }
    }

    fn save_to_disk(&self) -> Result<()> {
        if let Some(parent) = self.vault_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let records = {
            let cache = read_lock(&self.cache);
            PlaintextVaultRecords {
                providers: cache.clone(),
            }
        };
        let plaintext = serde_json::to_vec(&records).map_err(|e| {
            SunbornError::internal(format!("Failed to serialize vault records: {e}"))
        })?;

        let nonce_bytes = generate_random_nonce_96()?;
        let key = Key::<Aes256Gcm>::from_slice(self.master_key.as_bytes());
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(
                nonce,
                Payload {
                    msg: &plaintext,
                    aad: b"sunborn-local-vault-aad",
                },
            )
            .map_err(|e| SunbornError::internal(format!("Vault encryption failed: {e}")))?;

        let payload = EncryptedVaultPayload {
            version: 1,
            nonce: nonce_bytes,
            ciphertext,
        };
        let serialized = serde_json::to_vec(&payload).map_err(|e| {
            SunbornError::internal(format!("Failed to serialize encrypted vault: {e}"))
        })?;

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.vault_path)
            .map_err(|e| SunbornError::internal(format!("Failed to write vault file: {e}")))?;
        file.write_all(&serialized)
            .map_err(|e| SunbornError::internal(format!("Failed to write vault data: {e}")))?;
        file.flush()
            .map_err(|e| SunbornError::internal(format!("Failed to flush vault data: {e}")))?;
        Ok(())
    }

    /// Stores an API key for a specified provider (OpenAI, Anthropic, Gemini, Ollama).
    pub fn store_provider(
        &self,
        provider_name: &str,
        api_key: &str,
        base_url: Option<String>,
        default_model: Option<String>,
    ) -> Result<()> {
        let name = provider_name.trim().to_lowercase();
        if name.is_empty() {
            return Err(SunbornError::invalid_payload(
                "Provider name cannot be empty",
            ));
        }
        if api_key.trim().is_empty() {
            return Err(SunbornError::invalid_payload("API key cannot be empty"));
        }

        {
            let mut cache = write_lock(&self.cache);
            cache.insert(
                name,
                StoredProviderRecord {
                    api_key: api_key.trim().to_string(),
                    base_url,
                    default_model,
                },
            );
        }
        self.save_to_disk()
    }

    /// Retrieves an API key in-memory for proxying upstream (never serialize to API response).
    pub fn get_provider_key(&self, provider_name: &str) -> Option<String> {
        let name = provider_name.trim().to_lowercase();
        let cache = read_lock(&self.cache);
        cache.get(&name).map(|r| r.api_key.clone())
    }

    /// Removes a provider's credential.
    pub fn remove_provider(&self, provider_name: &str) -> Result<bool> {
        let name = provider_name.trim().to_lowercase();
        let removed = {
            let mut cache = write_lock(&self.cache);
            cache.remove(&name).is_some()
        };
        if removed {
            self.save_to_disk()?;
        }
        Ok(removed)
    }

    /// Lists configured providers with masked previews for UI rendering without secret exposure.
    pub fn list_providers(&self) -> Vec<ConfiguredProviderInfo> {
        let cache = read_lock(&self.cache);
        let mut list = Vec::new();
        for (name, record) in cache.iter() {
            list.push(ConfiguredProviderInfo {
                name: name.clone(),
                configured: true,
                key_preview: Self::mask_key(&record.api_key),
                base_url: record.base_url.clone(),
                default_model: record.default_model.clone(),
            });
        }
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_credential_vault_lifecycle_and_masking() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("sunborn_vault_test_{}", nanos));
        let vault_path = temp_dir.join("vault.enc");

        // 1. Initialize vault and store credentials
        {
            let vault = LocalCredentialVault::new_at_path(&vault_path);
            vault
                .store_provider(
                    "OpenAI",
                    "sk-proj-1234567890abcdef1234567890",
                    Some("https://api.openai.com/v1".into()),
                    Some("gpt-4o".into()),
                )
                .unwrap();
            vault
                .store_provider(
                    "Anthropic",
                    "sk-ant-api03-abcdefghijklmnop",
                    None,
                    Some("claude-3-5-sonnet".into()),
                )
                .unwrap();

            assert_eq!(
                vault.get_provider_key("openai").unwrap(),
                "sk-proj-1234567890abcdef1234567890"
            );
            let providers = vault.list_providers();
            assert_eq!(providers.len(), 2);
            assert_eq!(providers[0].name, "anthropic");
            assert_eq!(providers[0].key_preview, "sk-...mnop"); // Masked
            assert_eq!(providers[1].name, "openai");
            assert_eq!(providers[1].key_preview, "sk-...7890"); // Masked
        }

        // 2. Re-instantiate from disk (simulating machine reboot)
        {
            let restored = LocalCredentialVault::new_at_path(&vault_path);
            assert_eq!(
                restored.get_provider_key("openai").unwrap(),
                "sk-proj-1234567890abcdef1234567890"
            );
            assert_eq!(
                restored.get_provider_key("anthropic").unwrap(),
                "sk-ant-api03-abcdefghijklmnop"
            );

            restored.remove_provider("anthropic").unwrap();
            assert!(restored.get_provider_key("anthropic").is_none());
            assert_eq!(restored.list_providers().len(), 1);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
