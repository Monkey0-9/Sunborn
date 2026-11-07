//! # Sunborn Identity
//!
//! `sunborn-identity` provides enterprise-grade cryptographic identity management,
//! deterministic RFC 8785 JSON canonicalization, inline zero-leakage PII tokenization,
//! and O(1) AES-256-GCM authenticated envelope encryption for tenant data isolation
//! and cryptographic erasure (GDPR Article 17 "Right to be Forgotten").
//!
//! ## Core Modules
//!
//! - [`canonical`]: RFC 8785 JSON Canonicalization Scheme (JCS) and Unicode NFC normalization.
//! - [`crypto_shred`]: O(1) AEAD envelope encryption with hardware security modules (PKCS#11 HSM, AWS KMS, HashiCorp Vault Transit, Local AES-GCM).
//! - [`identity`]: State-complete 10-dimensional SHA-256 cache identity hash computation.
//! - [`privacy`]: Inline PII tokenization (`<US_SSN_1>`, `<EMAIL_1>`, etc.) with request-scoped ephemeral hydration.
//!
//! ## Quick Start
//!
//! ```rust
//! use sunborn_identity::{canonicalize_text, normalize_unicode_nfc, PiiTokenizer};
//!
//! // Unicode NFC normalization and whitespace collapse
//! let text = canonicalize_text("  Hello   world!  \r\n");
//! assert_eq!(text, "Hello world!");
//!
//! // Zero-leakage inline PII tokenization
//! let raw = "Contact alice@enterprise.org regarding SSN 000-12-3456.";
//! let (canonical, session) = PiiTokenizer::tokenize(raw);
//! assert!(canonical.contains("<EMAIL_1>"));
//! assert!(canonical.contains("<US_SSN_1>"));
//!
//! // Ephemeral client response hydration
//! let cached_response = "Processed request for <EMAIL_1>.";
//! let hydrated = session.hydrate(cached_response);
//! assert_eq!(hydrated, "Processed request for alice@enterprise.org.");
//! ```

pub mod canonical;
pub mod crypto_shred;
pub mod identity;
pub mod privacy;

pub use canonical::{
    canonicalize_json, canonicalize_json_rfc8785, canonicalize_text, normalize_unicode_nfc,
};
pub use crypto_shred::{
    base64_decode, base64_encode, constant_time_compare, generate_random_nonce_96,
    hkdf_sha256_derive, hmac_sha256, AwsKmsKeyProvider, EncryptedEnvelope, KeyManagementProvider,
    LocalAeadKeyProvider, Pkcs11HsmKeyProvider, RemoteAwsKmsKeyProvider,
    RemoteVaultTransitKeyProvider, TenantCryptoRegistry, VaultTransitKeyProvider, ZeroizingKey,
};
pub use identity::{CacheIdentity, IdentityEngine};
pub use privacy::{PiiTokenizer, PiiVaultSession};
