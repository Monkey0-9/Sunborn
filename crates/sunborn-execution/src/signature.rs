use sha2::{Digest, Sha256};
use sunborn_core::error::SunbornError;
use sunborn_core::model::{ApplicationId, TenantId};
use sunborn_identity::canonical::canonicalize_json;

pub struct ToolSignatureEngine;

impl ToolSignatureEngine {
    /// Computes a cryptographically deterministic tool signature.
    /// Incorporates tenant ID, app ID, tool name, tool version, canonicalized argument payload, and context hash.
    pub fn compute_signature(
        tenant_id: &TenantId,
        app_id: &ApplicationId,
        tool_name: &str,
        tool_version: &str,
        arguments: &serde_json::Value,
        context_hash: Option<&str>,
    ) -> Result<String, SunbornError> {
        let canonical_args = canonicalize_json(arguments);
        let args_str = serde_json::to_string(&canonical_args)
            .map_err(|e| SunbornError::invalid_payload(e.to_string()))?;

        let mut hasher = Sha256::new();
        hasher.update(b"sunborn:v1:tool:");
        hasher.update(tenant_id.0.as_bytes());
        hasher.update(b":");
        hasher.update(app_id.0.as_bytes());
        hasher.update(b":");
        hasher.update(tool_name.as_bytes());
        hasher.update(b":");
        hasher.update(tool_version.as_bytes());
        hasher.update(b":");
        hasher.update(args_str.as_bytes());
        hasher.update(b":");
        if let Some(ctx) = context_hash {
            hasher.update(ctx.as_bytes());
        }

        let result = hasher.finalize();
        Ok(hex::encode(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_determinism_with_key_reordering() {
        let tenant = TenantId::new("tenant-1");
        let app = ApplicationId::new("app-1");

        let args1 = serde_json::json!({
            "query": "SELECT * FROM users",
            "limit": 100,
            "filter": {
                "active": true,
                "role": "admin"
            }
        });

        let args2 = serde_json::json!({
            "filter": {
                "role": "admin",
                "active": true
            },
            "limit": 100,
            "query": "SELECT * FROM users"
        });

        let sig1 = ToolSignatureEngine::compute_signature(
            &tenant, &app, "db_query", "1.0.0", &args1, None,
        )
        .unwrap();
        let sig2 = ToolSignatureEngine::compute_signature(
            &tenant, &app, "db_query", "1.0.0", &args2, None,
        )
        .unwrap();

        assert_eq!(
            sig1, sig2,
            "Tool signatures must match regardless of JSON key ordering"
        );
    }

    #[test]
    fn test_signature_tenant_isolation() {
        let tenant1 = TenantId::new("tenant-alpha");
        let tenant2 = TenantId::new("tenant-beta");
        let app = ApplicationId::new("app-1");
        let args = serde_json::json!({"action": "calculate", "x": 42});

        let sig1 =
            ToolSignatureEngine::compute_signature(&tenant1, &app, "calc", "1.0", &args, None)
                .unwrap();
        let sig2 =
            ToolSignatureEngine::compute_signature(&tenant2, &app, "calc", "1.0", &args, None)
                .unwrap();

        assert_ne!(
            sig1, sig2,
            "Different tenants must produce distinct tool signatures"
        );
    }
}
