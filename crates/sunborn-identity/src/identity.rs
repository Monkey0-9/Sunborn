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

        // State-Complete Dimension 1 & 2: Sampling Gate (IEEE 754 temperature & stochastic isolation)
        let effective_temp = req.temperature.unwrap_or(0.0);
        let is_deterministic = effective_temp <= 1e-6;

        let seed = req.metadata.get("seed").and_then(|v| v.as_u64());
        if is_deterministic {
            // Parameter Equivalence: temperature 0.0 and absent None both resolve to deterministic t=0
            hasher.update(b"|t=0");
            match req.top_p {
                Some(p) if (p - 1.0).abs() > 1e-6 => {
                    hasher.update(format!("|p={:.4}", p).as_bytes())
                }
                _ => hasher.update(b"|p=1"),
            }
        } else {
            // Sampling Parameter Gate: Canonicalize (temperature, top_p, seed) tuple
            // so identical stochastic parameters can share an exact cache identity key,
            // while deterministic callers (t=0) are strictly isolated and never polluted.
            let p_val = req.top_p.unwrap_or(1.0);
            match seed {
                Some(s) => hasher.update(
                    format!("|t={:.4}|p={:.4}|seed={}", effective_temp, p_val, s).as_bytes(),
                ),
                None => hasher.update(
                    format!("|t={:.4}|p={:.4}|seed=none", effective_temp, p_val).as_bytes(),
                ),
            }
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

        // State-Complete Dimension 10: Multi-Turn Conversation Messages History
        if !req.messages.is_empty() {
            let mut msgs_hasher = Sha256::new();
            for msg in &req.messages {
                msgs_hasher.update(msg.role.as_bytes());
                msgs_hasher.update(b":");
                if let Some(content) = &msg.content {
                    msgs_hasher.update(canonicalize_text(content).as_bytes());
                }
                if let Some(name) = &msg.name {
                    msgs_hasher.update(b"|name=");
                    msgs_hasher.update(name.as_bytes());
                }
                if let Some(tc) = &msg.tool_calls {
                    if let Ok(tc_json) = serde_json::to_string(tc) {
                        msgs_hasher.update(b"|tc=");
                        msgs_hasher.update(tc_json.as_bytes());
                    }
                }
                if let Some(tcid) = &msg.tool_call_id {
                    msgs_hasher.update(b"|tcid=");
                    msgs_hasher.update(tcid.as_bytes());
                }
                msgs_hasher.update(b"\n");
            }
            let msgs_digest = hex::encode(msgs_hasher.finalize());
            hasher.update(format!("|msgs={}", msgs_digest).as_bytes());
        } else {
            hasher.update(b"|msgs=none");
        }

        // State-Complete Dimension 11: Principal & Session Scope Isolation (when supplied)
        if let Some(session_id) = req.metadata.get("session_id").and_then(|v| v.as_str()) {
            hasher.update(format!("|session={}", session_id).as_bytes());
        }
        if let Some(user_id) = req.metadata.get("user_id").and_then(|v| v.as_str()) {
            hasher.update(format!("|user={}", user_id).as_bytes());
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

    /// Server-derived canonical conversation & context digest (§10).
    /// Returns `None` only for stateless/context-free requests unless `context_free_share=true`.
    pub fn compute_context_digest(req: &RequestEnvelope) -> Option<String> {
        if let Some(explicit) = req.metadata.get("context_digest").and_then(|v| v.as_str()) {
            return Some(explicit.to_string());
        }
        let context_free_share = req
            .metadata
            .get("context_free_share")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if context_free_share && req.messages.is_empty() {
            return None;
        }

        let session_id = req.metadata.get("session_id").and_then(|v| v.as_str());
        let user_id = req.metadata.get("user_id").and_then(|v| v.as_str());
        let summary_ver = req.metadata.get("summary_version").and_then(|v| v.as_str());

        let has_context = !req.messages.is_empty()
            || req.system_prompt.is_some()
            || !req.tools.is_empty()
            || !req.retrieval_context_hashes.is_empty()
            || session_id.is_some()
            || user_id.is_some()
            || summary_ver.is_some();
        if !has_context {
            return None;
        }

        let mut h = Sha256::new();
        h.update(req.tenant_id.0.as_bytes());
        h.update(b"|");
        h.update(req.application_id.0.as_bytes());
        if let Some(sid) = session_id {
            h.update(format!("|sid={sid}").as_bytes());
        }
        if let Some(uid) = user_id {
            h.update(format!("|uid={uid}").as_bytes());
        }
        if let Some(sv) = summary_ver {
            h.update(format!("|sumv={sv}").as_bytes());
        }
        if let Some(ref sys) = req.system_prompt {
            h.update(format!("|sys={}", canonicalize_text(sys)).as_bytes());
        }
        for msg in &req.messages {
            h.update(msg.role.as_bytes());
            h.update(b":");
            if let Some(ref c) = msg.content {
                h.update(canonicalize_text(c).as_bytes());
            }
            if let Some(ref tc) = msg.tool_calls {
                h.update(serde_json::to_string(tc).unwrap_or_default().as_bytes());
            }
            if let Some(ref tcid) = msg.tool_call_id {
                h.update(tcid.as_bytes());
            }
            h.update(b"\n");
        }
        if !req.tools.is_empty() {
            let mut tools = req.tools.clone();
            tools.sort_by(|a, b| a.name.cmp(&b.name));
            h.update(serde_json::to_string(&tools).unwrap_or_default().as_bytes());
        }
        if !req.retrieval_context_hashes.is_empty() {
            let mut ctx = req.retrieval_context_hashes.clone();
            ctx.sort();
            h.update(ctx.join(",").as_bytes());
        }
        Some(format!("ctxv1:{}", hex::encode(h.finalize())))
    }

    /// Canonical application config & model parameter fingerprint (§3.3 & §3.4).
    pub fn compute_config_fingerprint(req: &RequestEnvelope) -> String {
        let mut h = Sha256::new();
        h.update(req.model.as_bytes());
        if let Some(ref sys) = req.system_prompt {
            h.update(format!("|sys={}", canonicalize_text(sys)).as_bytes());
        }
        h.update(
            format!(
                "|t={:.4}|p={:.4}",
                req.temperature.unwrap_or(0.0),
                req.top_p.unwrap_or(1.0)
            )
            .as_bytes(),
        );
        if let Some(m) = req.max_tokens {
            h.update(format!("|m={m}").as_bytes());
        }
        if !req.stop_sequences.is_empty() {
            let mut stops = req.stop_sequences.clone();
            stops.sort();
            h.update(format!("|stop={}", stops.join(",")).as_bytes());
        }
        if !req.tools.is_empty() {
            let mut tools = req.tools.clone();
            tools.sort_by(|a, b| a.name.cmp(&b.name));
            h.update(serde_json::to_string(&tools).unwrap_or_default().as_bytes());
        }
        if let Some(ref rf) = req.response_format {
            h.update(format!("|rf={rf}").as_bytes());
        }
        for k in ["app_config_version", "safety_policy", "retrieval_config"] {
            if let Some(val) = req.metadata.get(k) {
                h.update(format!("|{k}={val}").as_bytes());
            }
        }
        format!("cfgv1:{}", hex::encode(h.finalize()))
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
    fn test_canonical_temperature_equivalence_and_stochastic_gate() {
        let req_none = RequestEnvelope::new("tenant-a", "app-1", "gpt-4o", "Generate code");
        let mut req_zero = req_none.clone();
        req_zero.temperature = Some(0.0);

        let id1 = IdentityEngine::compute_identity(&req_none);
        let id2 = IdentityEngine::compute_identity(&req_zero);

        // Parameter equivalence (RFC 8785): None and explicit 0.0 temperature are both deterministic t=0
        assert_eq!(
            id1.hash, id2.hash,
            "None and explicit Some(0.0) temperature must map to the same deterministic identity"
        );

        // Stochastic request (temperature 0.7) must NOT collide with deterministic t=0
        let mut req_stochastic = req_none.clone();
        req_stochastic.temperature = Some(0.7);
        let id3 = IdentityEngine::compute_identity(&req_stochastic);
        assert_ne!(id1.hash, id3.hash);

        // Seeded stochastic request is deterministic for identical seeds
        let mut req_seeded1 = req_stochastic.clone();
        req_seeded1.metadata = serde_json::json!({ "seed": 42 });
        let mut req_seeded2 = req_stochastic.clone();
        req_seeded2.metadata = serde_json::json!({ "seed": 42 });
        let id_s1 = IdentityEngine::compute_identity(&req_seeded1);
        let id_s2 = IdentityEngine::compute_identity(&req_seeded2);
        assert_eq!(id_s1.hash, id_s2.hash);
    }

    #[test]
    fn test_state_complete_multi_turn_history_isolation() {
        use sunborn_core::model::ChatMessage;

        let mut req_turn1 = RequestEnvelope::new("tenant-a", "app-1", "gpt-4o", "Yes");
        req_turn1.messages = vec![
            ChatMessage::new("system", "You are a helpful assistant"),
            ChatMessage::new("user", "Are you ready?"),
            ChatMessage::new("assistant", "Yes, how can I help?"),
            ChatMessage::new("user", "Yes"),
        ];

        let mut req_turn2 = RequestEnvelope::new("tenant-a", "app-1", "gpt-4o", "Yes");
        req_turn2.messages = vec![
            ChatMessage::new("system", "You are a helpful assistant"),
            ChatMessage::new("user", "Do you want to delete the database?"),
            ChatMessage::new("assistant", "This action is destructive. Are you sure?"),
            ChatMessage::new("user", "Yes"),
        ];

        let id1 = IdentityEngine::compute_identity(&req_turn1);
        let id2 = IdentityEngine::compute_identity(&req_turn2);

        assert_ne!(
            id1.hash, id2.hash,
            "Identical last user prompt with different prior conversation turns must not collide in cache"
        );
    }
}
