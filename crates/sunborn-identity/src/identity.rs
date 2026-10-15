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

        // State-Complete Dimension 1: Temperature (distinguish explicit 0.0 vs absent None)
        match req.temperature {
            Some(t) => hasher.update(format!("|t={:.4}", t).as_bytes()),
            None => hasher.update(b"|t=none"),
        }

        // State-Complete Dimension 2: Top-P
        match req.top_p {
            Some(p) => hasher.update(format!("|p={:.4}", p).as_bytes()),
            None => hasher.update(b"|p=none"),
        }

        // State-Complete Dimension 3: Max Tokens
        match req.max_tokens {
            Some(m) => hasher.update(format!("|m={}", m).as_bytes()),
            None => hasher.update(b"|m=none"),
        }

        // State-Complete Dimension 4: Stop Sequences (deterministic sort)
        if !req.stop_sequences.is_empty() {
            let mut stops = req.stop_sequences.clone();
            stops.sort();
            hasher.update(format!("|stop={}", stops.join(",")).as_bytes());
        } else {
            hasher.update(b"|stop=none");
        }

        // State-Complete Dimension 5: Tool Configuration & Schemas
        if !req.tools.is_empty() {
            let mut tools = req.tools.clone();
            tools.sort_by(|a, b| a.name.cmp(&b.name));
            let tools_json = serde_json::to_string(&tools).unwrap_or_default();
            hasher.update(format!("|tools={}", tools_json).as_bytes());
        } else {
            hasher.update(b"|tools=none");
        }

        // State-Complete Dimension 6: Tool Choice
        match &req.tool_choice {
            Some(tc) => hasher.update(format!("|tc={}", tc).as_bytes()),
            None => hasher.update(b"|tc=none"),
        }

        // State-Complete Dimension 7: Response Format (e.g. JSON schema / Text)
        match &req.response_format {
            Some(rf) => hasher.update(format!("|rf={}", rf).as_bytes()),
            None => hasher.update(b"|rf=none"),
        }

        // State-Complete Dimension 8: Knowledge Epoch
        match req.knowledge_epoch {
            Some(ep) => hasher.update(format!("|ep={}", ep).as_bytes()),
            None => hasher.update(b"|ep=none"),
        }

        // State-Complete Dimension 9: Retrieval Context Hashes
        if !req.retrieval_context_hashes.is_empty() {
            let mut ctx = req.retrieval_context_hashes.clone();
            ctx.sort();
            hasher.update(format!("|ctx={}", ctx.join(",")).as_bytes());
        } else {
            hasher.update(b"|ctx=none");
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
    use sunborn_core::ToolDefinition;

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

    #[test]
    fn test_state_complete_tool_configuration_isolation() {
        let req_no_tools =
            RequestEnvelope::new("tenant-a", "app-1", "gpt-4o", "What is the weather?");
        let mut req_with_tools = req_no_tools.clone();
        req_with_tools.tools.push(ToolDefinition {
            name: "get_weather".to_string(),
            description: Some("Fetch current weather for city".to_string()),
            parameters: serde_json::json!({ "type": "object", "properties": { "city": { "type": "string" } } }),
        });

        let id1 = IdentityEngine::compute_identity(&req_no_tools);
        let id2 = IdentityEngine::compute_identity(&req_with_tools);

        assert_ne!(
            id1.hash, id2.hash,
            "Adding tools must alter execution cache identity"
        );
        assert_ne!(id1.key, id2.key);
    }

    #[test]
    fn test_state_complete_response_format_isolation() {
        let req_text = RequestEnvelope::new("tenant-a", "app-1", "gpt-4o", "List users");
        let mut req_json = req_text.clone();
        req_json.response_format = Some("json_object".to_string());

        let id1 = IdentityEngine::compute_identity(&req_text);
        let id2 = IdentityEngine::compute_identity(&req_json);

        assert_ne!(
            id1.hash, id2.hash,
            "Different response formats must have distinct cache keys"
        );
    }

    #[test]
    fn test_explicit_vs_default_temperature_isolation() {
        let req_none = RequestEnvelope::new("tenant-a", "app-1", "gpt-4o", "Generate code");
        let mut req_zero = req_none.clone();
        req_zero.temperature = Some(0.0);

        let id1 = IdentityEngine::compute_identity(&req_none);
        let id2 = IdentityEngine::compute_identity(&req_zero);

        assert_ne!(
            id1.hash, id2.hash,
            "None vs explicit Some(0.0) temperature must not collide"
        );
    }
}
