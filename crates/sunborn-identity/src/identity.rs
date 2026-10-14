use crate::canonical::canonicalize_text;
use sha2::{Digest, Sha256};
use sunborn_core::RequestEnvelope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheIdentity {
    pub key: String,
    pub hash: String,
    pub policy_version: String,
}

pub struct IdentityEngine;

impl IdentityEngine {
    pub const DEFAULT_POLICY_VERSION: &'static str = "v1";

    pub fn compute_identity(req: &RequestEnvelope) -> CacheIdentity {
        Self::compute_identity_with_version(req, Self::DEFAULT_POLICY_VERSION)
    }

    pub fn compute_identity_with_version(
        req: &RequestEnvelope,
        policy_version: &str,
    ) -> CacheIdentity {
        let canonical_prompt = canonicalize_text(&req.prompt);
        let canonical_system = req
            .system_prompt
            .as_deref()
            .map(canonicalize_text)
            .unwrap_or_default();

        let mut hasher = Sha256::new();
        hasher.update(req.tenant_id.0.as_bytes());
        hasher.update(b"|");
        hasher.update(req.application_id.0.as_bytes());
        hasher.update(b"|");
        hasher.update(req.model.as_bytes());
        hasher.update(b"|");
        hasher.update(policy_version.as_bytes());
        hasher.update(b"|");
        hasher.update(canonical_system.as_bytes());
        hasher.update(b"|");
        hasher.update(canonical_prompt.as_bytes());

        // Include temperature if specified
        if let Some(t) = req.temperature {
            hasher.update(format!("|t={:.2}", t).as_bytes());
        }

        let hash_bytes = hasher.finalize();
        let hash = hex::encode(hash_bytes);
        let key = format!(
            "sunborn:{}:{}:{}:{}:{}",
            req.tenant_id.0, req.application_id.0, req.model, policy_version, hash
        );

        CacheIdentity {
            key,
            hash,
            policy_version: policy_version.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_determinism() {
        let req1 = RequestEnvelope::new("tenant-a", "app-1", "gpt-4o", "  Hello   world!  ");
        let req2 = RequestEnvelope::new("tenant-a", "app-1", "gpt-4o", "Hello world!");

        let id1 = IdentityEngine::compute_identity(&req1);
        let id2 = IdentityEngine::compute_identity(&req2);

        assert_eq!(id1.hash, id2.hash);
        assert_eq!(id1.key, id2.key);
    }

    #[test]
    fn test_tenant_isolation_in_identity() {
        let req1 = RequestEnvelope::new("tenant-a", "app-1", "gpt-4o", "Hello world!");
        let req2 = RequestEnvelope::new("tenant-b", "app-1", "gpt-4o", "Hello world!");

        let id1 = IdentityEngine::compute_identity(&req1);
        let id2 = IdentityEngine::compute_identity(&req2);

        assert_ne!(id1.hash, id2.hash);
        assert_ne!(id1.key, id2.key);
    }
}
