//! Virtual Keys (Bifrost-inspired scoped governance & routing).
//!
//! Virtual keys decouple downstream application/team credentials from raw
//! upstream provider API keys. Each virtual key carries:
//! - Scoped tenant & application identity
//! - Model allowlists (glob/prefix/exact)
//! - Budget ceiling (USD) & optional TPM/RPM policy overrides
//! - Weighted provider routing preferences

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualKeyRecord {
    pub id: String,
    pub name: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub tenant_id: String,
    pub application_id: String,
    pub environment: String,
    pub allowed_models: Vec<String>,
    pub budget_usd: Option<f64>,
    pub spent_usd: f64,
    /// Optional provider weight overrides: provider_name -> weight (1..=100).
    #[serde(default)]
    pub provider_weights: HashMap<String, u32>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVirtualKeyRequest {
    pub name: String,
    pub tenant_id: String,
    #[serde(default = "default_app")]
    pub application_id: String,
    #[serde(default = "default_env")]
    pub environment: String,
    #[serde(default = "default_models")]
    pub allowed_models: Vec<String>,
    #[serde(default)]
    pub budget_usd: Option<f64>,
    #[serde(default)]
    pub provider_weights: HashMap<String, u32>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

fn default_app() -> String {
    "default".to_string()
}

fn default_env() -> String {
    "prod".to_string()
}

fn default_models() -> Vec<String> {
    vec!["*".to_string()]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedVirtualKeyResponse {
    pub record: VirtualKeyRecord,
    /// Raw secret returned once upon creation.
    pub raw_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VirtualKeyError {
    NotFound,
    Disabled,
    Expired,
    BudgetExceeded,
    ModelNotAllowed(String),
}

impl std::fmt::Display for VirtualKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "Virtual key not found"),
            Self::Disabled => write!(f, "Virtual key is disabled"),
            Self::Expired => write!(f, "Virtual key is expired"),
            Self::BudgetExceeded => write!(f, "Virtual key budget exceeded"),
            Self::ModelNotAllowed(m) => write!(f, "Model '{}' is not allowed by virtual key", m),
        }
    }
}

#[derive(Clone, Default)]
pub struct VirtualKeyStore {
    by_hash: Arc<RwLock<HashMap<String, VirtualKeyRecord>>>,
    by_id: Arc<RwLock<HashMap<String, String>>>,
}

impl VirtualKeyStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn hash_key(raw: &str) -> String {
        let mut h = Sha256::new();
        h.update(raw.as_bytes());
        hex::encode(h.finalize())
    }

    pub fn create(&self, req: CreateVirtualKeyRequest) -> CreatedVirtualKeyResponse {
        let id = format!("vk_{}", Uuid::new_v4().simple());
        let raw_key = format!("sb-vk-{}", Uuid::new_v4().simple());
        let key_hash = Self::hash_key(&raw_key);
        let key_prefix = raw_key[..12.min(raw_key.len())].to_string();

        let record = VirtualKeyRecord {
            id: id.clone(),
            name: req.name,
            key_hash: key_hash.clone(),
            key_prefix,
            tenant_id: req.tenant_id,
            application_id: req.application_id,
            environment: req.environment,
            allowed_models: req.allowed_models,
            budget_usd: req.budget_usd,
            spent_usd: 0.0,
            provider_weights: req.provider_weights,
            enabled: true,
            created_at: Utc::now(),
            expires_at: req.expires_at,
        };

        let mut by_hash = self.by_hash.write().unwrap_or_else(|e| e.into_inner());
        let mut by_id = self.by_id.write().unwrap_or_else(|e| e.into_inner());
        by_id.insert(id, key_hash.clone());
        by_hash.insert(key_hash, record.clone());

        CreatedVirtualKeyResponse { record, raw_key }
    }

    pub fn authorize(
        &self,
        raw_key: &str,
        model: &str,
    ) -> Result<VirtualKeyRecord, VirtualKeyError> {
        let rec = self.validate_credential(raw_key)?;
        let model_ok = rec.allowed_models.iter().any(|pat| {
            pat == "*"
                || pat == model
                || pat
                    .strip_suffix('*')
                    .map(|prefix| model.starts_with(prefix))
                    .unwrap_or(false)
        });
        if !model_ok {
            return Err(VirtualKeyError::ModelNotAllowed(model.to_string()));
        }
        Ok(rec)
    }

    pub fn validate_credential(&self, raw_key: &str) -> Result<VirtualKeyRecord, VirtualKeyError> {
        let hash = Self::hash_key(raw_key);
        let by_hash = self.by_hash.read().unwrap_or_else(|e| e.into_inner());
        let rec = by_hash.get(&hash).ok_or(VirtualKeyError::NotFound)?;
        if !rec.enabled {
            return Err(VirtualKeyError::Disabled);
        }
        if let Some(exp) = rec.expires_at {
            if Utc::now() > exp {
                return Err(VirtualKeyError::Expired);
            }
        }
        if let Some(budget) = rec.budget_usd {
            if rec.spent_usd >= budget {
                return Err(VirtualKeyError::BudgetExceeded);
            }
        }
        Ok(rec.clone())
    }

    pub fn record_spend(&self, key_id: &str, delta_usd: f64) -> bool {
        let by_id = self.by_id.read().unwrap_or_else(|e| e.into_inner());
        let Some(hash) = by_id.get(key_id) else {
            return false;
        };
        let mut by_hash = self.by_hash.write().unwrap_or_else(|e| e.into_inner());
        if let Some(rec) = by_hash.get_mut(hash) {
            rec.spent_usd += delta_usd.max(0.0);
            return true;
        }
        false
    }

    pub fn revoke(&self, key_id: &str) -> bool {
        let by_id = self.by_id.read().unwrap_or_else(|e| e.into_inner());
        let Some(hash) = by_id.get(key_id) else {
            return false;
        };
        let mut by_hash = self.by_hash.write().unwrap_or_else(|e| e.into_inner());
        if let Some(rec) = by_hash.get_mut(hash) {
            rec.enabled = false;
            return true;
        }
        false
    }

    pub fn list(&self, tenant_id: Option<&str>) -> Vec<VirtualKeyRecord> {
        let by_hash = self.by_hash.read().unwrap_or_else(|e| e.into_inner());
        by_hash
            .values()
            .filter(|r| tenant_id.map(|t| r.tenant_id == t).unwrap_or(true))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_key_model_prefix_and_budget_enforcement() {
        let store = VirtualKeyStore::new();
        let created = store.create(CreateVirtualKeyRequest {
            name: "staging-app".into(),
            tenant_id: "acme".into(),
            application_id: "chat".into(),
            environment: "staging".into(),
            allowed_models: vec!["gpt-4o*".into()],
            budget_usd: Some(1.0),
            provider_weights: HashMap::new(),
            expires_at: None,
        });

        assert!(store.authorize(&created.raw_key, "gpt-4o-mini").is_ok());
        assert!(matches!(
            store.authorize(&created.raw_key, "claude-3-5-sonnet"),
            Err(VirtualKeyError::ModelNotAllowed(_))
        ));
        assert!(store.record_spend(&created.record.id, 1.25));
        assert!(matches!(
            store.authorize(&created.raw_key, "gpt-4o"),
            Err(VirtualKeyError::BudgetExceeded)
        ));
    }
}
