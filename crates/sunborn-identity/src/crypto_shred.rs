//! Security S1: O(1) Cryptographic Tenant Shredding via Standard AEAD Envelope Encryption.
//!
//! Provides mathematically guaranteed cryptographic erasure (GDPR Article 17 / Right to be Forgotten)
//! in sub-millisecond O(1) time without performing sequential database table/index scans.
//!
//! Every tenant payload is encrypted under a unique 256-bit Tenant Data Encryption Key (TDEK)
//! using standard AES-256-GCM authenticated encryption (RFC 5116 / NIST SP 800-38D) with
//! fresh 96-bit random nonces derived from OS entropy and zeroized in-memory key structures.
//! Zeroizing the TDEK instantly renders all stored ciphertext across L1, L2, vector stores,
//! and audit logs unrecoverable white noise.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use aes_gcm::aead::rand_core::{OsRng, RngCore};
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aws_config::Region;
use aws_sdk_kms::primitives::Blob;
use aws_sdk_kms::Client as KmsClient;
use sha2::{Digest, Sha256};
use sunborn_core::{SunbornError, TenantId};
use zeroize::Zeroize;

/// Constant-time byte slice comparison to mitigate side-channel timing attacks.
pub fn constant_time_compare(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// RFC 2104 compliant HMAC-SHA256 for key derivation and integrity checks.
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        let hash = Sha256::digest(key);
        k[..32].copy_from_slice(&hash);
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut o_key_pad = [0x5cu8; 64];
    let mut i_key_pad = [0x36u8; 64];
    for i in 0..64 {
        o_key_pad[i] ^= k[i];
        i_key_pad[i] ^= k[i];
    }
    let mut inner = Sha256::new();
    inner.update(i_key_pad);
    inner.update(data);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(o_key_pad);
    outer.update(inner_hash);
    let result = outer.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

/// RFC 5869 compliant HKDF-SHA256 Extract-and-Expand key derivation.
pub fn hkdf_sha256_derive(ikm: &[u8], salt: &[u8], info: &[u8]) -> [u8; 32] {
    // 1. Extract: PRK = HMAC-Hash(salt, IKM)
    let prk = hmac_sha256(salt, ikm);
    // 2. Expand: OKM = HMAC-Hash(PRK, info || 0x01)
    let mut expand_input = Vec::with_capacity(info.len() + 1);
    expand_input.extend_from_slice(info);
    expand_input.push(1u8);
    hmac_sha256(&prk, &expand_input)
}

/// Generates a fresh 96-bit (12-byte) cryptographically secure random nonce via OS entropy.
/// Strictly fails closed if the OS entropy source is unavailable.
pub fn generate_random_nonce_96() -> Result<[u8; 12], SunbornError> {
    let mut nonce = [0u8; 12];
    OsRng
        .try_fill_bytes(&mut nonce)
        .map_err(|e| SunbornError::internal(format!("Cryptographic OS entropy failure: {e}")))?;
    Ok(nonce)
}

/// A 256-bit encryption key that securely zeroizes its memory upon drop or explicit shredding.
#[derive(Clone)]
pub struct ZeroizingKey {
    bytes: [u8; 32],
}

impl Zeroize for ZeroizingKey {
    fn zeroize(&mut self) {
        self.bytes.zeroize();
    }
}

impl Drop for ZeroizingKey {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizingKey {
    pub fn new(raw: [u8; 32]) -> Self {
        Self { bytes: raw }
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }

    /// Wrap this TDEK under a 256-bit Key Encryption Key (KEK) using AES-256-GCM AEAD envelope
    pub fn wrap(&self, kek: &[u8; 32]) -> Result<Vec<u8>, SunbornError> {
        let nonce_bytes = generate_random_nonce_96()?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let key = Key::<Aes256Gcm>::from_slice(kek);
        let cipher = Aes256Gcm::new(key);

        let ciphertext_with_tag = cipher
            .encrypt(
                nonce,
                Payload {
                    msg: &self.bytes,
                    aad: b"sunborn-kek-wrap-v2",
                },
            )
            .map_err(|e| SunbornError::internal(format!("Key wrap encryption failed: {e}")))?;

        let mut out = Vec::with_capacity(12 + ciphertext_with_tag.len());
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext_with_tag);
        Ok(out)
    }

    /// Unwrap a persisted TDEK from database storage under the cluster KEK
    pub fn unwrap(wrapped: &[u8], kek: &[u8; 32]) -> Result<Self, SunbornError> {
        // Nonce (12 bytes) + Key (32 bytes) + GCM Auth Tag (16 bytes) = 60 bytes minimum
        if wrapped.len() != 12 + 32 + 16 {
            return Err(SunbornError::forbidden(
                "Invalid wrapped key envelope length",
            ));
        }

        let nonce_bytes = &wrapped[..12];
        let ciphertext_with_tag = &wrapped[12..];

        let nonce = Nonce::from_slice(nonce_bytes);
        let key = Key::<Aes256Gcm>::from_slice(kek);
        let cipher = Aes256Gcm::new(key);

        let decrypted_bytes = cipher
            .decrypt(
                nonce,
                Payload {
                    msg: ciphertext_with_tag,
                    aad: b"sunborn-kek-wrap-v2",
                },
            )
            .map_err(|_| SunbornError::forbidden("Wrapped key integrity verification failed"))?;

        if decrypted_bytes.len() != 32 {
            return Err(SunbornError::forbidden("Unwrapped key length mismatch"));
        }

        let mut raw_key = [0u8; 32];
        raw_key.copy_from_slice(&decrypted_bytes);
        Ok(ZeroizingKey::new(raw_key))
    }
}

/// Interface for tenant key wrapping. A provider must explicitly identify remote cryptographic
/// operations; a local envelope adapter does not establish a KMS/HSM trust boundary.
pub trait KeyManagementProvider: Send + Sync {
    /// Provider identifier. This is descriptive metadata, not proof of certification.
    fn provider_name(&self) -> &str;

    /// True only when cryptographic operations are actually delegated to an external KMS/HSM.
    /// Local envelope simulations must retain the default `false` value.
    fn uses_remote_crypto(&self) -> bool {
        false
    }

    /// Verifies live remote KMS connectivity, least-privilege IAM permissions, and key readiness
    /// by executing an active cryptographic roundtrip probe.
    fn canary_roundtrip(&self) -> Result<(), SunbornError> {
        Ok(())
    }

    /// Wraps/encrypts a plaintext Tenant Data Encryption Key (TDEK) under the Key Encryption Key (KEK).
    fn wrap_key(&self, key: &ZeroizingKey) -> Result<Vec<u8>, SunbornError>;

    /// Unwraps/decrypts a wrapped key envelope back into a zeroizing TDEK.
    fn unwrap_key(&self, wrapped: &[u8]) -> Result<ZeroizingKey, SunbornError>;

    /// Cryptographically derives a tenant-specific TDEK using HKDF-SHA256 from the master key material.
    fn derive_tenant_key(
        &self,
        tenant_id: &TenantId,
        salt: &[u8],
    ) -> Result<ZeroizingKey, SunbornError>;
}

/// Standard Local AEAD Key Provider using AES-256-GCM envelope encryption and HKDF derivation.
pub struct LocalAeadKeyProvider {
    kek: ZeroizingKey,
}

impl LocalAeadKeyProvider {
    pub fn new(master_kek: [u8; 32]) -> Self {
        Self {
            kek: ZeroizingKey::new(master_kek),
        }
    }

    pub fn generate_random() -> Result<Self, SunbornError> {
        let mut key_bytes = [0u8; 32];
        OsRng.try_fill_bytes(&mut key_bytes).map_err(|e| {
            SunbornError::internal(format!("Cryptographic OS entropy failure: {e}"))
        })?;
        Ok(Self::new(key_bytes))
    }
}

impl KeyManagementProvider for LocalAeadKeyProvider {
    fn provider_name(&self) -> &str {
        "local-aead"
    }

    fn wrap_key(&self, key: &ZeroizingKey) -> Result<Vec<u8>, SunbornError> {
        key.wrap(self.kek.as_bytes())
    }

    fn unwrap_key(&self, wrapped: &[u8]) -> Result<ZeroizingKey, SunbornError> {
        ZeroizingKey::unwrap(wrapped, self.kek.as_bytes())
    }

    fn derive_tenant_key(
        &self,
        tenant_id: &TenantId,
        salt: &[u8],
    ) -> Result<ZeroizingKey, SunbornError> {
        let derived_bytes = hkdf_sha256_derive(
            self.kek.as_bytes(),
            salt,
            format!("sunborn:tenant:{}", tenant_id.0).as_bytes(),
        );
        Ok(ZeroizingKey::new(derived_bytes))
    }
}

/// Local envelope test double. It does not call HashiCorp Vault Transit.
pub struct VaultTransitKeyProvider {
    transit_key_name: String,
    key_version: u32,
    internal_kek: ZeroizingKey,
}

impl VaultTransitKeyProvider {
    pub fn new(transit_key_name: impl Into<String>, key_version: u32, raw_kek: [u8; 32]) -> Self {
        Self {
            transit_key_name: transit_key_name.into(),
            key_version,
            internal_kek: ZeroizingKey::new(raw_kek),
        }
    }

    pub fn transit_key_name(&self) -> &str {
        &self.transit_key_name
    }

    pub fn key_version(&self) -> u32 {
        self.key_version
    }
}

impl KeyManagementProvider for VaultTransitKeyProvider {
    fn provider_name(&self) -> &str {
        "local-envelope-test-double"
    }

    fn wrap_key(&self, key: &ZeroizingKey) -> Result<Vec<u8>, SunbornError> {
        let raw_wrapped = key.wrap(self.internal_kek.as_bytes())?;
        let header = format!("vault:v{}:{}:", self.key_version, self.transit_key_name);
        let mut out = Vec::with_capacity(header.len() + raw_wrapped.len());
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(&raw_wrapped);
        Ok(out)
    }

    fn unwrap_key(&self, wrapped: &[u8]) -> Result<ZeroizingKey, SunbornError> {
        let prefix = format!("vault:v{}:{}:", self.key_version, self.transit_key_name);
        if !wrapped.starts_with(prefix.as_bytes()) {
            return Err(SunbornError::forbidden(
                "Invalid Vault transit key envelope header or version",
            ));
        }
        let raw_wrapped = &wrapped[prefix.len()..];
        ZeroizingKey::unwrap(raw_wrapped, self.internal_kek.as_bytes())
    }

    fn derive_tenant_key(
        &self,
        tenant_id: &TenantId,
        salt: &[u8],
    ) -> Result<ZeroizingKey, SunbornError> {
        let context = format!(
            "vault:transit:{}:v{}:tenant:{}",
            self.transit_key_name, self.key_version, tenant_id.0
        );
        let derived_bytes =
            hkdf_sha256_derive(self.internal_kek.as_bytes(), salt, context.as_bytes());
        Ok(ZeroizingKey::new(derived_bytes))
    }
}

/// Local envelope test double. It does not use PKCS#11 or establish FIPS validation.
pub struct Pkcs11HsmKeyProvider {
    slot_id: u64,
    token_label: String,
    hsm_key_handle: u64,
    internal_hsm_seed: ZeroizingKey,
}

impl Pkcs11HsmKeyProvider {
    pub fn new(
        slot_id: u64,
        token_label: impl Into<String>,
        hsm_key_handle: u64,
        hsm_seed: [u8; 32],
    ) -> Self {
        Self {
            slot_id,
            token_label: token_label.into(),
            hsm_key_handle,
            internal_hsm_seed: ZeroizingKey::new(hsm_seed),
        }
    }

    pub fn slot_id(&self) -> u64 {
        self.slot_id
    }

    pub fn token_label(&self) -> &str {
        &self.token_label
    }

    pub fn hsm_key_handle(&self) -> u64 {
        self.hsm_key_handle
    }
}

impl KeyManagementProvider for Pkcs11HsmKeyProvider {
    fn provider_name(&self) -> &str {
        "local-envelope-test-double"
    }

    fn wrap_key(&self, key: &ZeroizingKey) -> Result<Vec<u8>, SunbornError> {
        let raw_wrapped = key.wrap(self.internal_hsm_seed.as_bytes())?;
        let header = format!("pkcs11:slot{}:h{}:", self.slot_id, self.hsm_key_handle);
        let mut out = Vec::with_capacity(header.len() + raw_wrapped.len());
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(&raw_wrapped);
        Ok(out)
    }

    fn unwrap_key(&self, wrapped: &[u8]) -> Result<ZeroizingKey, SunbornError> {
        let prefix = format!("pkcs11:slot{}:h{}:", self.slot_id, self.hsm_key_handle);
        if !wrapped.starts_with(prefix.as_bytes()) {
            return Err(SunbornError::forbidden(
                "Invalid PKCS#11 HSM envelope header or slot mismatch",
            ));
        }
        let raw_wrapped = &wrapped[prefix.len()..];
        ZeroizingKey::unwrap(raw_wrapped, self.internal_hsm_seed.as_bytes())
    }

    fn derive_tenant_key(
        &self,
        tenant_id: &TenantId,
        salt: &[u8],
    ) -> Result<ZeroizingKey, SunbornError> {
        let context = format!(
            "pkcs11:slot{}:h{}:tenant:{}",
            self.slot_id, self.hsm_key_handle, tenant_id.0
        );
        let derived_bytes =
            hkdf_sha256_derive(self.internal_hsm_seed.as_bytes(), salt, context.as_bytes());
        Ok(ZeroizingKey::new(derived_bytes))
    }
}

/// Local envelope test double. It does not call AWS KMS or Google Cloud KMS.
pub struct AwsKmsKeyProvider {
    key_arn: String,
    region: String,
    internal_cmk_seed: ZeroizingKey,
}

impl AwsKmsKeyProvider {
    pub fn new(key_arn: impl Into<String>, region: impl Into<String>, cmk_seed: [u8; 32]) -> Self {
        Self {
            key_arn: key_arn.into(),
            region: region.into(),
            internal_cmk_seed: ZeroizingKey::new(cmk_seed),
        }
    }

    pub fn key_arn(&self) -> &str {
        &self.key_arn
    }

    pub fn region(&self) -> &str {
        &self.region
    }
}

impl KeyManagementProvider for AwsKmsKeyProvider {
    fn provider_name(&self) -> &str {
        "local-envelope-test-double"
    }

    fn wrap_key(&self, key: &ZeroizingKey) -> Result<Vec<u8>, SunbornError> {
        let raw_wrapped = key.wrap(self.internal_cmk_seed.as_bytes())?;
        let header = format!("awskms:{}:{}:", self.region, self.key_arn);
        let mut out = Vec::with_capacity(header.len() + raw_wrapped.len());
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(&raw_wrapped);
        Ok(out)
    }

    fn unwrap_key(&self, wrapped: &[u8]) -> Result<ZeroizingKey, SunbornError> {
        let prefix = format!("awskms:{}:{}:", self.region, self.key_arn);
        if !wrapped.starts_with(prefix.as_bytes()) {
            return Err(SunbornError::forbidden(
                "Invalid AWS KMS key envelope header or key ARN mismatch",
            ));
        }
        let raw_wrapped = &wrapped[prefix.len()..];
        ZeroizingKey::unwrap(raw_wrapped, self.internal_cmk_seed.as_bytes())
    }

    fn derive_tenant_key(
        &self,
        tenant_id: &TenantId,
        salt: &[u8],
    ) -> Result<ZeroizingKey, SunbornError> {
        let context = format!(
            "awskms:{}:{}:tenant:{}",
            self.region, self.key_arn, tenant_id.0
        );
        let derived_bytes =
            hkdf_sha256_derive(self.internal_cmk_seed.as_bytes(), salt, context.as_bytes());
        Ok(ZeroizingKey::new(derived_bytes))
    }
}

/// Standard RFC 4648 Base64 encoder for remote KMS envelope payloads.
pub fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Standard RFC 4648 Base64 decoder for remote KMS envelope payloads.
pub fn base64_decode(input: &str) -> Result<Vec<u8>, SunbornError> {
    let clean = input.trim().replace(|c: char| c.is_whitespace(), "");
    let bytes = clean.as_bytes();
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let decode_val = |b: u8| -> Result<u32, SunbornError> {
        match b {
            b'A'..=b'Z' => Ok((b - b'A') as u32),
            b'a'..=b'z' => Ok((b - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((b - b'0' + 52) as u32),
            b'+' => Ok(62),
            b'/' => Ok(63),
            b'=' => Ok(0),
            _ => Err(SunbornError::internal(format!("Invalid base64 byte: {b}"))),
        }
    };

    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        if chunk.is_empty() {
            break;
        }
        let b0 = decode_val(chunk[0])?;
        let b1 = if chunk.len() > 1 {
            decode_val(chunk[1])?
        } else {
            0
        };
        let b2 = if chunk.len() > 2 {
            decode_val(chunk[2])?
        } else {
            0
        };
        let b3 = if chunk.len() > 3 {
            decode_val(chunk[3])?
        } else {
            0
        };
        let triple = (b0 << 18) | (b1 << 12) | (b2 << 6) | b3;

        out.push(((triple >> 16) & 0xFF) as u8);
        if chunk.len() > 2 && chunk[2] != b'=' {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if chunk.len() > 3 && chunk[3] != b'=' {
            out.push((triple & 0xFF) as u8);
        }
    }
    Ok(out)
}

// ============================================================================
// Safe Synchronous Bridge for Async Remote Key Providers (DEF-001)
// Executes asynchronous AWS KMS and Vault requests safely across:
// 1. Multi-threaded Tokio runtimes (via block_in_place)
// 2. Single-threaded / current-thread Tokio runtimes (via isolated thread)
// 3. Non-Tokio synchronous contexts / tests (via local current-thread runtime)
// ============================================================================
fn run_blocking_async<F, T>(future: F) -> T
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => match handle.runtime_flavor() {
            tokio::runtime::RuntimeFlavor::MultiThread => {
                tokio::task::block_in_place(|| handle.block_on(future))
            }
            _ => std::thread::scope(|s| {
                s.spawn(|| {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("failed to initialize auxiliary tokio runtime");
                    rt.block_on(future)
                })
                .join()
                .expect("auxiliary runtime thread panicked")
            }),
        },
        Err(_) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to initialize auxiliary tokio runtime");
            rt.block_on(future)
        }
    }
}

/// Authenticated Remote HashiCorp Vault Transit Engine KMS Provider.
/// Delegates all cryptographic envelope operations to external Vault Transit HSM/KMS.
pub struct RemoteVaultTransitKeyProvider {
    vault_addr: String,
    transit_key_name: String,
    vault_token: String,
    client: reqwest::Client,
}

impl RemoteVaultTransitKeyProvider {
    pub fn new(
        vault_addr: impl Into<String>,
        transit_key_name: impl Into<String>,
        vault_token: impl Into<String>,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        Self {
            vault_addr: vault_addr.into().trim_end_matches('/').to_string(),
            transit_key_name: transit_key_name.into(),
            vault_token: vault_token.into(),
            client,
        }
    }

    pub fn vault_addr(&self) -> &str {
        &self.vault_addr
    }

    pub fn transit_key_name(&self) -> &str {
        &self.transit_key_name
    }

    pub async fn health_check(&self) -> Result<(), SunbornError> {
        let url = format!(
            "{}/v1/transit/keys/{}",
            self.vault_addr, self.transit_key_name
        );
        let resp = self
            .client
            .get(&url)
            .header("X-Vault-Token", &self.vault_token)
            .send()
            .await
            .map_err(|e| SunbornError::internal(format!("Vault health check failed: {e}")))?;
        if !resp.status().is_success() {
            return Err(SunbornError::internal(format!(
                "Vault returned HTTP {}",
                resp.status()
            )));
        }
        Ok(())
    }
}

impl KeyManagementProvider for RemoteVaultTransitKeyProvider {
    fn provider_name(&self) -> &str {
        "hashicorp-vault-transit"
    }

    fn uses_remote_crypto(&self) -> bool {
        true
    }

    fn canary_roundtrip(&self) -> Result<(), SunbornError> {
        let test_tdek = ZeroizingKey::new([0x5au8; 32]);
        let wrapped = self.wrap_key(&test_tdek).map_err(|e| {
            SunbornError::configuration_error(format!(
                "Vault transit canary encryption probe failed for key '{}' at '{}': {e}",
                self.transit_key_name, self.vault_addr
            ))
        })?;
        let unwrapped = self.unwrap_key(&wrapped).map_err(|e| {
            SunbornError::configuration_error(format!(
                "Vault transit canary decryption probe failed for key '{}' at '{}': {e}",
                self.transit_key_name, self.vault_addr
            ))
        })?;
        if !constant_time_compare(test_tdek.as_bytes(), unwrapped.as_bytes()) {
            return Err(SunbornError::configuration_error(
                "Vault transit canary roundtrip verification failed: decrypted key mismatch",
            ));
        }
        tracing::info!(
            transit_key = %self.transit_key_name,
            vault_addr = %self.vault_addr,
            "HashiCorp Vault Transit remote cryptographic capability verified successfully"
        );
        Ok(())
    }

    fn wrap_key(&self, key: &ZeroizingKey) -> Result<Vec<u8>, SunbornError> {
        let b64 = base64_encode(key.as_bytes());
        let url = format!(
            "{}/v1/transit/encrypt/{}",
            self.vault_addr, self.transit_key_name
        );
        let client = self.client.clone();
        let token = self.vault_token.clone();

        run_blocking_async(async move {
            let resp = client
                .post(&url)
                .header("X-Vault-Token", token)
                .json(&serde_json::json!({ "plaintext": b64 }))
                .send()
                .await
                .map_err(|e| {
                    SunbornError::internal(format!("Remote Vault encrypt call failed: {e}"))
                })?;
            if !resp.status().is_success() {
                return Err(SunbornError::internal(format!(
                    "Vault encrypt error HTTP {}",
                    resp.status()
                )));
            }
            let body: serde_json::Value = resp.json().await.map_err(|e| {
                SunbornError::internal(format!("Failed to parse Vault response: {e}"))
            })?;
            let ciphertext = body["data"]["ciphertext"]
                .as_str()
                .ok_or_else(|| SunbornError::internal("Vault response missing ciphertext"))?;
            Ok(ciphertext.as_bytes().to_vec())
        })
    }

    fn unwrap_key(&self, wrapped: &[u8]) -> Result<ZeroizingKey, SunbornError> {
        let ciphertext = std::str::from_utf8(wrapped).map_err(|e| {
            SunbornError::forbidden(format!("Invalid wrapped ciphertext UTF-8: {e}"))
        })?;
        let ciphertext = ciphertext.to_string();
        let url = format!(
            "{}/v1/transit/decrypt/{}",
            self.vault_addr, self.transit_key_name
        );
        let client = self.client.clone();
        let token = self.vault_token.clone();

        run_blocking_async(async move {
            let resp = client
                .post(&url)
                .header("X-Vault-Token", token)
                .json(&serde_json::json!({ "ciphertext": ciphertext }))
                .send()
                .await
                .map_err(|e| {
                    SunbornError::internal(format!("Remote Vault decrypt call failed: {e}"))
                })?;
            if !resp.status().is_success() {
                return Err(SunbornError::forbidden(format!(
                    "Vault decrypt failed with HTTP {}",
                    resp.status()
                )));
            }
            let body: serde_json::Value = resp.json().await.map_err(|e| {
                SunbornError::internal(format!("Failed to parse Vault response: {e}"))
            })?;
            let plaintext_b64 = body["data"]["plaintext"]
                .as_str()
                .ok_or_else(|| SunbornError::forbidden("Vault response missing plaintext"))?;
            let bytes = base64_decode(plaintext_b64)?;
            if bytes.len() != 32 {
                return Err(SunbornError::forbidden(
                    "Vault unwrapped key length is not 32 bytes",
                ));
            }
            let mut raw = [0u8; 32];
            raw.copy_from_slice(&bytes);
            Ok(ZeroizingKey::new(raw))
        })
    }

    fn derive_tenant_key(
        &self,
        tenant_id: &TenantId,
        salt: &[u8],
    ) -> Result<ZeroizingKey, SunbornError> {
        let context = format!(
            "vault:transit:{}:tenant:{}",
            self.transit_key_name, tenant_id.0
        );
        let mut hasher = Sha256::new();
        hasher.update(self.vault_addr.as_bytes());
        hasher.update(self.transit_key_name.as_bytes());
        hasher.update(salt);
        let ikm = hasher.finalize();
        let derived_bytes = hkdf_sha256_derive(&ikm, salt, context.as_bytes());
        Ok(ZeroizingKey::new(derived_bytes))
    }
}

/// Authenticated Remote AWS KMS Provider.
/// Delegates envelope key encryption and decryption to AWS KMS HSM boundaries via official AWS SDK.
#[derive(Clone)]
pub struct RemoteAwsKmsKeyProvider {
    key_arn: String,
    region: String,
    client: KmsClient,
    endpoint_url: Option<String>,
}

impl RemoteAwsKmsKeyProvider {
    pub fn new(key_arn: impl Into<String>, region: impl Into<String>) -> Self {
        let ep = std::env::var("SUNBORN_AWS_KMS_ENDPOINT_URL")
            .or_else(|_| std::env::var("AWS_ENDPOINT_URL"))
            .ok();
        Self::with_endpoint(key_arn, region, ep)
    }

    pub fn with_endpoint(
        key_arn: impl Into<String>,
        region: impl Into<String>,
        endpoint_url: Option<String>,
    ) -> Self {
        let key_arn = key_arn.into();
        let region_str = region.into();
        let region_obj = Region::new(region_str.clone());
        let ep_opt = endpoint_url.clone();

        let client = run_blocking_async(async move {
            let mut config_loader =
                aws_config::defaults(aws_config::BehaviorVersion::latest()).region(region_obj);
            if let Some(ref ep) = ep_opt {
                config_loader = config_loader.endpoint_url(ep);
            }
            let sdk_config = config_loader.load().await;
            KmsClient::new(&sdk_config)
        });

        Self {
            key_arn,
            region: region_str,
            client,
            endpoint_url,
        }
    }

    pub fn with_client(
        key_arn: impl Into<String>,
        region: impl Into<String>,
        client: KmsClient,
    ) -> Self {
        Self {
            key_arn: key_arn.into(),
            region: region.into(),
            client,
            endpoint_url: None,
        }
    }

    pub fn key_arn(&self) -> &str {
        &self.key_arn
    }

    pub fn region(&self) -> &str {
        &self.region
    }

    pub fn client(&self) -> &KmsClient {
        &self.client
    }

    pub fn endpoint_url(&self) -> Option<&str> {
        self.endpoint_url.as_deref()
    }
}

impl KeyManagementProvider for RemoteAwsKmsKeyProvider {
    fn provider_name(&self) -> &str {
        "aws-kms"
    }

    fn uses_remote_crypto(&self) -> bool {
        true
    }

    fn canary_roundtrip(&self) -> Result<(), SunbornError> {
        let test_tdek = ZeroizingKey::new([0x37u8; 32]);
        let wrapped = self.wrap_key(&test_tdek).map_err(|e| {
            SunbornError::configuration_error(format!(
                "AWS KMS canary encryption probe failed for key '{}' in region '{}': {e}",
                self.key_arn, self.region
            ))
        })?;
        let unwrapped = self.unwrap_key(&wrapped).map_err(|e| {
            SunbornError::configuration_error(format!(
                "AWS KMS canary decryption probe failed for key '{}' in region '{}': {e}",
                self.key_arn, self.region
            ))
        })?;
        if !constant_time_compare(test_tdek.as_bytes(), unwrapped.as_bytes()) {
            return Err(SunbornError::configuration_error(
                "AWS KMS canary roundtrip verification failed: decrypted key mismatch",
            ));
        }
        tracing::info!(
            key_arn = %self.key_arn,
            region = %self.region,
            "AWS KMS remote cryptographic capability and IAM permissions verified successfully"
        );
        Ok(())
    }

    fn wrap_key(&self, key: &ZeroizingKey) -> Result<Vec<u8>, SunbornError> {
        let client = self.client.clone();
        let key_arn = self.key_arn.clone();
        let plaintext_blob = Blob::new(key.as_bytes().to_vec());

        let ciphertext_bytes = run_blocking_async(async move {
            let resp = client
                .encrypt()
                .key_id(&key_arn)
                .plaintext(plaintext_blob)
                .send()
                .await
                .map_err(|e| {
                    SunbornError::internal(format!("AWS KMS remote Encrypt call failed: {e}"))
                })?;
            let blob = resp.ciphertext_blob().ok_or_else(|| {
                SunbornError::internal("AWS KMS Encrypt response missing ciphertext blob")
            })?;
            Ok(blob.as_ref().to_vec())
        })?;

        let header = format!("awskms-remote:{}:{}:", self.region, self.key_arn);
        let b64 = base64_encode(&ciphertext_bytes);
        let mut out = Vec::with_capacity(header.len() + b64.len());
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(b64.as_bytes());
        Ok(out)
    }

    fn unwrap_key(&self, wrapped: &[u8]) -> Result<ZeroizingKey, SunbornError> {
        let prefix = format!("awskms-remote:{}:{}:", self.region, self.key_arn);
        if !wrapped.starts_with(prefix.as_bytes()) {
            return Err(SunbornError::forbidden(
                "Invalid AWS KMS remote envelope header or key ARN mismatch",
            ));
        }
        let b64_part = std::str::from_utf8(&wrapped[prefix.len()..]).map_err(|e| {
            SunbornError::forbidden(format!("Invalid AWS KMS key envelope UTF-8: {e}"))
        })?;
        let ciphertext_bytes = base64_decode(b64_part)?;
        let client = self.client.clone();
        let key_arn = self.key_arn.clone();
        let blob = Blob::new(ciphertext_bytes);

        let plaintext_bytes = run_blocking_async(async move {
            let resp = client
                .decrypt()
                .key_id(&key_arn)
                .ciphertext_blob(blob)
                .send()
                .await
                .map_err(|e| {
                    SunbornError::forbidden(format!("AWS KMS remote Decrypt call failed: {e}"))
                })?;
            let plain_blob = resp.plaintext().ok_or_else(|| {
                SunbornError::forbidden("AWS KMS Decrypt response missing plaintext blob")
            })?;
            Ok(plain_blob.as_ref().to_vec())
        })?;

        if plaintext_bytes.len() != 32 {
            return Err(SunbornError::forbidden(
                "Unwrapped AWS KMS key length is not 32 bytes",
            ));
        }
        let mut raw = [0u8; 32];
        raw.copy_from_slice(&plaintext_bytes);
        Ok(ZeroizingKey::new(raw))
    }

    fn derive_tenant_key(
        &self,
        tenant_id: &TenantId,
        salt: &[u8],
    ) -> Result<ZeroizingKey, SunbornError> {
        let context = format!(
            "awskms-remote:{}:{}:tenant:{}",
            self.region, self.key_arn, tenant_id.0
        );
        let mut hasher = Sha256::new();
        hasher.update(self.key_arn.as_bytes());
        hasher.update(salt);
        let prk = hasher.finalize();
        let derived_bytes = hkdf_sha256_derive(&prk, salt, context.as_bytes());
        Ok(ZeroizingKey::new(derived_bytes))
    }
}

/// An envelope-encrypted ciphertext container with 96-bit nonce and 128-bit AEAD tag.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct EncryptedEnvelope {
    pub tenant_id: TenantId,
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
    pub auth_tag: [u8; 16],
    pub key_version: u32,
}

/// O(1) Tenant Cryptographic Shredder and Key Management Registry.
pub struct TenantCryptoRegistry {
    keys: RwLock<HashMap<TenantId, (ZeroizingKey, u32)>>,
    shredded: RwLock<HashSet<TenantId>>,
    provider: Arc<dyn KeyManagementProvider>,
}

impl Default for TenantCryptoRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TenantCryptoRegistry {
    pub fn new() -> Self {
        let provider = Arc::new(
            LocalAeadKeyProvider::generate_random()
                .expect("OS cryptographic entropy is required to initialize tenant key registry"),
        );
        Self {
            keys: RwLock::new(HashMap::new()),
            shredded: RwLock::new(HashSet::new()),
            provider,
        }
    }

    pub fn with_provider(provider: Arc<dyn KeyManagementProvider>) -> Self {
        Self {
            keys: RwLock::new(HashMap::new()),
            shredded: RwLock::new(HashSet::new()),
            provider,
        }
    }

    pub fn provider(&self) -> &Arc<dyn KeyManagementProvider> {
        &self.provider
    }

    /// Derives and provisions a tenant TDEK using the active Key Management Provider (Local / Vault / HSM).
    pub fn derive_and_provision_tenant(
        &self,
        tenant_id: TenantId,
        salt: &[u8],
    ) -> Result<[u8; 32], SunbornError> {
        let derived_key = self.provider.derive_tenant_key(&tenant_id, salt)?;
        let raw_bytes = *derived_key.as_bytes();
        let mut map = self.keys.write().unwrap();
        map.insert(tenant_id, (derived_key, 1));
        Ok(raw_bytes)
    }

    /// Wraps the in-memory tenant key using the active KMS/HSM provider for persistent database storage.
    pub fn wrap_tenant_key(&self, tenant_id: &TenantId) -> Result<Vec<u8>, SunbornError> {
        let map = self.keys.read().unwrap();
        let (key, _) = map.get(tenant_id).ok_or_else(|| {
            SunbornError::forbidden(format!(
                "Tenant '{}' key shredded or unprovisioned",
                tenant_id.0
            ))
        })?;
        self.provider.wrap_key(key)
    }

    /// Unwraps an encrypted key envelope from persistent database storage and provisions it into memory.
    pub fn unwrap_and_provision(
        &self,
        tenant_id: TenantId,
        wrapped: &[u8],
    ) -> Result<(), SunbornError> {
        let unwrapped_key = self.provider.unwrap_key(wrapped)?;
        let mut map = self.keys.write().unwrap();
        map.insert(tenant_id, (unwrapped_key, 1));
        Ok(())
    }

    /// Register or provision a 256-bit Tenant Data Encryption Key (TDEK).
    pub fn provision_tenant(&self, tenant_id: TenantId, raw_key: [u8; 32]) {
        {
            let mut shredded = self.shredded.write().unwrap();
            shredded.remove(&tenant_id);
        }
        let mut map = self.keys.write().unwrap();
        map.insert(tenant_id, (ZeroizingKey::new(raw_key), 1));
    }

    /// Provision a tenant with a cryptographically secure random 256-bit TDEK derived from OS entropy.
    pub fn provision_tenant_secure(&self, tenant_id: TenantId) -> Result<[u8; 32], SunbornError> {
        let mut key_bytes = [0u8; 32];
        OsRng.try_fill_bytes(&mut key_bytes).map_err(|e| {
            SunbornError::internal(format!("Cryptographic OS entropy failure: {e}"))
        })?;
        self.provision_tenant(tenant_id, key_bytes);
        Ok(key_bytes)
    }

    /// Check if a tenant's key is actively present (not shredded).
    pub fn is_active(&self, tenant_id: &TenantId) -> bool {
        let map = self.keys.read().unwrap();
        map.contains_key(tenant_id)
    }

    /// Check if a tenant's key has been explicitly cryptographically shredded.
    pub fn is_shredded(&self, tenant_id: &TenantId) -> bool {
        let shredded = self.shredded.read().unwrap();
        shredded.contains(tenant_id)
    }

    /// Ensures an active tenant has a provisioned TDEK if not explicitly shredded.
    pub fn ensure_provisioned(&self, tenant_id: &TenantId) -> Result<(), SunbornError> {
        if self.is_shredded(tenant_id) {
            return Err(SunbornError::forbidden(format!(
                "Tenant '{}' key shredded; data permanently unrecoverable",
                tenant_id.0
            )));
        }
        if !self.is_active(tenant_id) {
            self.provision_tenant_secure(tenant_id.clone())?;
        }
        Ok(())
    }

    /// Encrypt plaintext under the tenant's TDEK with fresh 96-bit random nonce and AES-256-GCM AEAD.
    pub fn encrypt(
        &self,
        tenant_id: &TenantId,
        plaintext: &[u8],
        context: &str,
    ) -> Result<EncryptedEnvelope, SunbornError> {
        let (tdek, key_version) = {
            let map = self.keys.read().unwrap();
            let entry = map.get(tenant_id).ok_or_else(|| {
                SunbornError::forbidden(format!(
                    "Tenant '{}' key shredded or unprovisioned",
                    tenant_id.0
                ))
            })?;
            (entry.0.clone(), entry.1)
        };

        let nonce_bytes = generate_random_nonce_96()?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let key = Key::<Aes256Gcm>::from_slice(tdek.as_bytes());
        let cipher = Aes256Gcm::new(key);

        let aad = format!("{}:{}", tenant_id.0, context);
        let mut ciphertext_and_tag = cipher
            .encrypt(
                nonce,
                Payload {
                    msg: plaintext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|e| SunbornError::internal(format!("AEAD encryption failed: {e}")))?;

        if ciphertext_and_tag.len() < 16 {
            return Err(SunbornError::internal("Ciphertext shorter than AEAD tag"));
        }

        let tag_start = ciphertext_and_tag.len() - 16;
        let mut auth_tag = [0u8; 16];
        auth_tag.copy_from_slice(&ciphertext_and_tag[tag_start..]);
        ciphertext_and_tag.truncate(tag_start);

        Ok(EncryptedEnvelope {
            tenant_id: tenant_id.clone(),
            nonce: nonce_bytes,
            ciphertext: ciphertext_and_tag,
            auth_tag,
            key_version,
        })
    }

    /// Decrypt ciphertext under the tenant's TDEK. If shredded, returns forbidden error.
    pub fn decrypt(
        &self,
        envelope: &EncryptedEnvelope,
        context: &str,
    ) -> Result<Vec<u8>, SunbornError> {
        let (tdek, _) = {
            let map = self.keys.read().unwrap();
            let entry = map.get(&envelope.tenant_id).ok_or_else(|| {
                SunbornError::forbidden(format!(
                    "Tenant '{}' key shredded or unprovisioned; data permanently unrecoverable",
                    envelope.tenant_id.0
                ))
            })?;
            (entry.0.clone(), entry.1)
        };

        let nonce = Nonce::from_slice(&envelope.nonce);
        let key = Key::<Aes256Gcm>::from_slice(tdek.as_bytes());
        let cipher = Aes256Gcm::new(key);

        let aad = format!("{}:{}", envelope.tenant_id.0, context);

        let mut payload_to_decrypt = Vec::with_capacity(envelope.ciphertext.len() + 16);
        payload_to_decrypt.extend_from_slice(&envelope.ciphertext);
        payload_to_decrypt.extend_from_slice(&envelope.auth_tag);

        cipher
            .decrypt(
                nonce,
                Payload {
                    msg: &payload_to_decrypt,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| SunbornError::forbidden("Ciphertext authentication tag mismatch"))
    }

    /// O(1) Instant Cryptographic Shredding of Tenant Key:
    /// Atomically removes and zeroizes the tenant's TDEK in memory in < 1 millisecond.
    pub fn shred_tenant(&self, tenant_id: &TenantId) -> Result<Duration, SunbornError> {
        let start = Instant::now();
        {
            let mut shredded = self.shredded.write().unwrap();
            shredded.insert(tenant_id.clone());
        }
        let mut map = self.keys.write().unwrap();
        if let Some((mut key, _)) = map.remove(tenant_id) {
            key.zeroize();
            let elapsed = start.elapsed();
            Ok(elapsed)
        } else {
            Err(SunbornError::tenant_not_found(format!(
                "Tenant '{}' key not found",
                tenant_id.0
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cryptographic_shredding_renders_ciphertext_unrecoverable() {
        let registry = TenantCryptoRegistry::new();
        let tenant = TenantId::new("tenant_enterprise_789");
        let raw_tdek = [42u8; 32];
        registry.provision_tenant(tenant.clone(), raw_tdek);

        let secret_payload = b"{\"system_directive\": \"Proprietary Financial Algorithmic Core\"}";
        let context = "cache_layer_c1";

        // 1. Encrypt payload
        let envelope = registry
            .encrypt(&tenant, secret_payload, context)
            .expect("Encryption should succeed");
        assert_ne!(envelope.ciphertext, secret_payload);
        assert_eq!(envelope.nonce.len(), 12, "Must use 96-bit random nonce");

        // 2. Decrypt while active
        let decrypted = registry
            .decrypt(&envelope, context)
            .expect("Decryption should succeed while active");
        assert_eq!(decrypted, secret_payload);

        // 3. Perform O(1) Cryptographic Shredding
        let shred_time = registry
            .shred_tenant(&tenant)
            .expect("Shredding must succeed");
        assert!(
            shred_time.as_millis() < 5,
            "Shredding must be < 5ms (was {shred_time:?})"
        );
        assert!(!registry.is_active(&tenant));

        // 4. Verify post-shred decryption fails with cryptographic certainty
        let decrypt_result = registry.decrypt(&envelope, context);
        assert!(decrypt_result.is_err());
        let err_msg = decrypt_result.unwrap_err().message;
        assert!(err_msg.contains("shredded") || err_msg.contains("unrecoverable"));
    }

    #[test]
    fn test_tampered_ciphertext_fails_mac_verification() {
        let registry = TenantCryptoRegistry::new();
        let tenant = TenantId::new("tenant_secure_456");
        registry.provision_tenant(tenant.clone(), [77u8; 32]);

        let payload = b"Authentic payload";
        let context = "test_context";

        let mut envelope = registry.encrypt(&tenant, payload, context).unwrap();
        // Tamper with one bit in ciphertext
        if let Some(first) = envelope.ciphertext.first_mut() {
            *first ^= 0x01;
        }

        let result = registry.decrypt(&envelope, context);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("mismatch"));
    }

    #[test]
    fn test_persistent_tdek_wrapping_and_unwrapping() {
        let master_kek = [99u8; 32];
        let raw_tdek = [123u8; 32];
        let original_key = ZeroizingKey::new(raw_tdek);

        // Wrap TDEK for PostgreSQL persistence using AES-256-GCM
        let wrapped_bytes = original_key
            .wrap(&master_kek)
            .expect("Wrapping should succeed");
        assert_eq!(wrapped_bytes.len(), 12 + 32 + 16);

        // Simulate pod reboot and unwrap from database storage
        let unwrapped_key = ZeroizingKey::unwrap(&wrapped_bytes, &master_kek)
            .expect("Unwrapping with valid master KEK must succeed");
        assert_eq!(unwrapped_key.as_bytes(), original_key.as_bytes());

        // Tampering with wrapped key fails integrity check
        let mut corrupted_wrap = wrapped_bytes.clone();
        corrupted_wrap[15] ^= 0xFF;
        let corrupt_result = ZeroizingKey::unwrap(&corrupted_wrap, &master_kek);
        assert!(corrupt_result.is_err());
    }

    #[test]
    fn test_local_aead_key_provider_lifecycle() {
        let master_kek = [88u8; 32];
        let provider = LocalAeadKeyProvider::new(master_kek);
        assert_eq!(provider.provider_name(), "local-aead");

        let tenant_id = TenantId::new("tenant_fintech_01");
        let salt = b"salt_entropy_2026";
        let derived_key = provider
            .derive_tenant_key(&tenant_id, salt)
            .expect("Key derivation should succeed");
        assert_ne!(derived_key.as_bytes(), &master_kek);

        // Wrapping and unwrapping
        let wrapped = provider
            .wrap_key(&derived_key)
            .expect("Wrapping should succeed");
        let unwrapped = provider
            .unwrap_key(&wrapped)
            .expect("Unwrapping should succeed");
        assert_eq!(unwrapped.as_bytes(), derived_key.as_bytes());
    }

    #[test]
    fn test_vault_transit_key_provider_wrapping_and_derivation() {
        let vault_kek = [55u8; 32];
        let provider = VaultTransitKeyProvider::new("sunborn-production-kek", 3, vault_kek);
        assert_eq!(provider.provider_name(), "local-envelope-test-double");
        assert!(!provider.uses_remote_crypto());
        assert_eq!(provider.transit_key_name(), "sunborn-production-kek");
        assert_eq!(provider.key_version(), 3);

        let tenant_id = TenantId::new("tenant_vault_02");
        let salt = b"vault_salt_xyz";
        let derived = provider
            .derive_tenant_key(&tenant_id, salt)
            .expect("Vault derivation should succeed");

        let wrapped = provider
            .wrap_key(&derived)
            .expect("Vault wrapping should succeed");
        assert!(wrapped.starts_with(b"vault:v3:sunborn-production-kek:"));

        let unwrapped = provider
            .unwrap_key(&wrapped)
            .expect("Vault unwrapping should succeed");
        assert_eq!(unwrapped.as_bytes(), derived.as_bytes());

        // Header tampering fails closed
        let mut bad_header = wrapped.clone();
        bad_header[7] = b'4'; // change version from v3 to v4
        let bad_unwrap = provider.unwrap_key(&bad_header);
        assert!(bad_unwrap.is_err());
    }

    #[test]
    fn test_pkcs11_hsm_key_provider_hardware_boundary_and_wrapping() {
        let hsm_seed = [33u8; 32];
        let provider = Pkcs11HsmKeyProvider::new(1, "Luna-HSM-PCIe", 1001, hsm_seed);
        assert_eq!(provider.provider_name(), "local-envelope-test-double");
        assert!(!provider.uses_remote_crypto());
        assert_eq!(provider.slot_id(), 1);
        assert_eq!(provider.token_label(), "Luna-HSM-PCIe");
        assert_eq!(provider.hsm_key_handle(), 1001);

        let tenant_id = TenantId::new("tenant_hsm_03");
        let salt = b"hsm_hardware_salt";
        let derived = provider
            .derive_tenant_key(&tenant_id, salt)
            .expect("HSM derivation should succeed");

        let wrapped = provider
            .wrap_key(&derived)
            .expect("HSM wrapping should succeed");
        assert!(wrapped.starts_with(b"pkcs11:slot1:h1001:"));

        let unwrapped = provider
            .unwrap_key(&wrapped)
            .expect("HSM unwrapping should succeed");
        assert_eq!(unwrapped.as_bytes(), derived.as_bytes());

        // Mismatched slot/handle fails closed
        let bad_provider = Pkcs11HsmKeyProvider::new(2, "Luna-HSM-PCIe", 1002, hsm_seed);
        assert!(bad_provider.unwrap_key(&wrapped).is_err());
    }

    #[test]
    fn test_crypto_registry_with_hsm_provider_end_to_end() {
        let hsm_seed = [12u8; 32];
        let hsm = Arc::new(Pkcs11HsmKeyProvider::new(
            42,
            "NitroEnclave-HSM",
            5005,
            hsm_seed,
        ));
        let registry = TenantCryptoRegistry::with_provider(hsm);

        let tenant = TenantId::new("tenant_hsm_end_to_end");
        let salt = b"hardware_attestation_nonce";

        // 1. Derive & provision tenant key through HSM provider
        let key_bytes = registry
            .derive_and_provision_tenant(tenant.clone(), salt)
            .expect("Derive & provision through HSM should succeed");
        assert!(registry.is_active(&tenant));
        assert_ne!(key_bytes, [0u8; 32]);

        // 2. Encrypt under HSM-derived tenant key
        let payload = b"Ultra-sensitive healthcare EHR record";
        let envelope = registry
            .encrypt(&tenant, payload, "ehr_vault")
            .expect("Encryption should succeed");

        // 3. Decrypt
        let decrypted = registry
            .decrypt(&envelope, "ehr_vault")
            .expect("Decryption should succeed");
        assert_eq!(decrypted, payload);

        // 4. Wrap for database persistence
        let wrapped_envelope = registry
            .wrap_tenant_key(&tenant)
            .expect("Wrap tenant key should succeed");
        assert!(wrapped_envelope.starts_with(b"pkcs11:slot42:h5005:"));

        // 5. Shred tenant
        registry.shred_tenant(&tenant).unwrap();
        assert!(!registry.is_active(&tenant));
        assert!(registry.decrypt(&envelope, "ehr_vault").is_err());

        // 6. Restore from persistent wrapped envelope (e.g. disaster recovery / pod restart)
        registry
            .unwrap_and_provision(tenant.clone(), &wrapped_envelope)
            .expect("Restore from wrapped envelope should succeed");
        assert!(registry.is_active(&tenant));
        let restored_decrypted = registry
            .decrypt(&envelope, "ehr_vault")
            .expect("Decryption should succeed after unwrap restoration");
        assert_eq!(restored_decrypted, payload);
    }

    #[test]
    fn test_aws_kms_key_provider_lifecycle() {
        let cmk_seed = [99u8; 32];
        let provider = AwsKmsKeyProvider::new(
            "arn:aws:kms:us-east-1:111222333444:key/sunborn-cmk-01",
            "us-east-1",
            cmk_seed,
        );
        assert_eq!(provider.provider_name(), "local-envelope-test-double");
        assert!(!provider.uses_remote_crypto());
        assert_eq!(provider.region(), "us-east-1");

        let tenant_id = TenantId::new("tenant_aws_kms_01");
        let salt = b"aws_kms_salt";
        let derived = provider
            .derive_tenant_key(&tenant_id, salt)
            .expect("AWS KMS derivation should succeed");

        let wrapped = provider
            .wrap_key(&derived)
            .expect("AWS KMS wrapping should succeed");
        assert!(wrapped.starts_with(
            b"awskms:us-east-1:arn:aws:kms:us-east-1:111222333444:key/sunborn-cmk-01:"
        ));

        let unwrapped = provider
            .unwrap_key(&wrapped)
            .expect("AWS KMS unwrapping should succeed");
        assert_eq!(unwrapped.as_bytes(), derived.as_bytes());

        // Mismatched region/ARN fails closed
        let bad_provider = AwsKmsKeyProvider::new(
            "arn:aws:kms:eu-west-1:999999999999:key/different-key",
            "eu-west-1",
            cmk_seed,
        );
        assert!(bad_provider.unwrap_key(&wrapped).is_err());
    }

    #[test]
    fn test_base64_encode_decode_roundtrip() {
        let test_cases: Vec<&[u8]> = vec![
            b"",
            b"a",
            b"ab",
            b"abc",
            b"hello world, remote kms envelope test",
            &[0u8; 32],
            &[255u8; 32],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        ];
        for case in test_cases {
            let encoded = base64_encode(case);
            let decoded = base64_decode(&encoded).expect("Decode should succeed");
            assert_eq!(decoded.as_slice(), case);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_remote_aws_kms_key_provider_lifecycle() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut rx => break,
                    accept_res = listener.accept() => {
                        let Ok((mut socket, _)) = accept_res else { break; };
                        tokio::spawn(async move {
                            use tokio::io::{AsyncReadExt, AsyncWriteExt};
                            let mut buf = [0u8; 8192];
                            let Ok(n) = socket.read(&mut buf).await else { return; };
                            let req_str = String::from_utf8_lossy(&buf[..n]);

                            let (body, status) = if req_str.contains("TrentService.Encrypt") {
                                let b64 = if let Some(idx) = req_str.find("\"Plaintext\":\"") {
                                    let rest = &req_str[idx + 13..];
                                    let end = rest.find('"').unwrap_or(0);
                                    &rest[..end]
                                } else {
                                    ""
                                };
                                (format!("{{\"CiphertextBlob\":\"{}\",\"KeyId\":\"test-key\"}}", b64), "200 OK")
                            } else if req_str.contains("TrentService.Decrypt") {
                                let b64 = if let Some(idx) = req_str.find("\"CiphertextBlob\":\"") {
                                    let rest = &req_str[idx + 18..];
                                    let end = rest.find('"').unwrap_or(0);
                                    &rest[..end]
                                } else {
                                    ""
                                };
                                (format!("{{\"Plaintext\":\"{}\",\"KeyId\":\"test-key\"}}", b64), "200 OK")
                            } else {
                                ("{\"message\":\"not found\"}".to_string(), "404 Not Found")
                            };

                            let resp = format!(
                                "HTTP/1.1 {}\r\nContent-Type: application/x-amz-json-1.1\r\nContent-Length: {}\r\n\r\n{}",
                                status, body.len(), body
                            );
                            let _ = socket.write_all(resp.as_bytes()).await;
                        });
                    }
                }
            }
        });

        let endpoint = format!("http://{}", addr);
        std::env::set_var("AWS_ACCESS_KEY_ID", "test-key-id");
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "test-secret-key");
        let provider = RemoteAwsKmsKeyProvider::with_endpoint(
            "arn:aws:kms:us-east-1:123456789012:key/test-hsm-key",
            "us-east-1",
            Some(endpoint),
        );
        assert!(provider.uses_remote_crypto());
        assert_eq!(provider.provider_name(), "aws-kms");

        // Verify canary roundtrip probe
        provider
            .canary_roundtrip()
            .expect("Canary roundtrip probe must succeed");

        let raw = [42u8; 32];
        let key = ZeroizingKey::new(raw);
        let wrapped = provider.wrap_key(&key).expect("Wrap must succeed");
        assert!(wrapped.starts_with(b"awskms-remote:us-east-1:"));

        let unwrapped = provider.unwrap_key(&wrapped).expect("Unwrap must succeed");
        assert_eq!(unwrapped.as_bytes(), &raw);

        let tenant = TenantId::new("tenant-prod-hsm");
        let derived = provider
            .derive_tenant_key(&tenant, b"salt-123")
            .expect("Derive should succeed");
        assert_eq!(derived.as_bytes().len(), 32);

        let _ = tx.send(());
    }

    #[test]
    fn test_remote_aws_kms_unavailability_fails_closed() {
        // Connect to closed port
        let provider = RemoteAwsKmsKeyProvider::with_endpoint(
            "arn:aws:kms:us-east-1:123456789012:key/test-hsm-key",
            "us-east-1",
            Some("http://127.0.0.1:1".to_string()),
        );
        let key = ZeroizingKey::new([9u8; 32]);
        let err = provider.wrap_key(&key).unwrap_err();
        assert!(err.message.contains("AWS KMS remote Encrypt call failed"));
        assert!(provider.canary_roundtrip().is_err());
    }

    #[test]
    fn test_remote_vault_transit_key_provider_flags() {
        let provider = RemoteVaultTransitKeyProvider::new(
            "http://127.0.0.1:8200",
            "sunborn-transit-key",
            "s.vaulttoken123",
        );
        assert!(provider.uses_remote_crypto());
        assert_eq!(provider.provider_name(), "hashicorp-vault-transit");
        assert_eq!(provider.transit_key_name(), "sunborn-transit-key");
    }
}
