//! Separate RAG Retrieval-Result Cache Tier (§9 / SB-09).
//!
//! Stores and validates retrieved document chunks independently from generated-answer
//! cache entries. Enforces index version, filter hash, embedding model fingerprint,
//! requester ACL scope, and live dependency freshness checks (`EpochLeaseManager` &
//! `TemporalValidityEngine`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::RwLock;
use sunborn_core::sync::{read_lock, write_lock};
use sunborn_core::{ApplicationId, TenantId};
use sunborn_freshness::TemporalValidityEngine;
use sunborn_invalidation::EpochLeaseManager;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetrievedChunk {
    pub source_id: Uuid,
    pub chunk_id: String,
    pub content_hash: String,
    pub version: u64,
    pub score: f32,
    #[serde(default)]
    pub excerpt: String,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalQuerySpec {
    pub tenant_id: TenantId,
    pub application_id: ApplicationId,
    pub query: String,
    pub index_version: String,
    pub embedding_fingerprint: String,
    pub filters: serde_json::Value,
    #[serde(default)]
    pub acl_scope: Option<String>,
    #[serde(default)]
    pub expected_permission_version: Option<u64>,
    #[serde(default)]
    pub current_source_hashes: HashMap<Uuid, String>,
    #[serde(default)]
    pub deleted_source_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalCacheEntry {
    pub retrieval_key: String,
    pub tenant_id: TenantId,
    pub application_id: ApplicationId,
    pub query: String,
    pub index_version: String,
    pub embedding_fingerprint: String,
    pub filters_hash: String,
    pub acl_scope: Option<String>,
    pub permission_version: u64,
    pub chunks: Vec<RetrievedChunk>,
    pub source_epochs: HashMap<Uuid, u64>,
    pub retrieved_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RetrievalRejectReason {
    CacheMiss,
    IndexVersionMismatch,
    EmbeddingFingerprintMismatch,
    FilterMismatch,
    AclScopeMismatch,
    PermissionRevoked,
    SourceChunkChanged,
    SourceDeleted,
    SourceEpochStale,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalLookupOutcome {
    pub hit: bool,
    pub retrieval_key: String,
    pub reject_reason: Option<RetrievalRejectReason>,
    pub chunks: Vec<RetrievedChunk>,
}

pub struct RagRetrievalCache {
    entries: RwLock<HashMap<String, RetrievalCacheEntry>>,
}

impl Default for RagRetrievalCache {
    fn default() -> Self {
        Self::new()
    }
}

impl RagRetrievalCache {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    pub fn hash_filters(filters: &serde_json::Value) -> String {
        let mut h = Sha256::new();
        h.update(
            serde_json::to_string(filters)
                .unwrap_or_default()
                .as_bytes(),
        );
        hex::encode(h.finalize())
    }

    pub fn compute_key(spec: &RetrievalQuerySpec) -> String {
        let mut h = Sha256::new();
        h.update(spec.tenant_id.0.as_bytes());
        h.update(b"|");
        h.update(spec.application_id.0.as_bytes());
        h.update(b"|");
        h.update(spec.query.trim().to_lowercase().as_bytes());
        h.update(b"|");
        h.update(spec.index_version.as_bytes());
        h.update(b"|");
        h.update(spec.embedding_fingerprint.as_bytes());
        h.update(b"|");
        h.update(Self::hash_filters(&spec.filters).as_bytes());
        if let Some(ref acl) = spec.acl_scope {
            h.update(format!("|acl={acl}").as_bytes());
        }
        format!("retv1:{}", hex::encode(h.finalize()))
    }

    pub async fn admit(
        &self,
        spec: &RetrievalQuerySpec,
        chunks: Vec<RetrievedChunk>,
        epoch_mgr: &EpochLeaseManager,
        temporal_engine: &TemporalValidityEngine,
    ) -> RetrievalCacheEntry {
        let mut source_epochs = HashMap::new();
        for c in &chunks {
            let ep = epoch_mgr.acquire_lease(c.source_id).await;
            source_epochs.insert(c.source_id, ep);
        }
        let perm_ver = temporal_engine
            .get_or_init_state(crate::AppState::tenant_permission_scope_uuid(
                &spec.tenant_id.0,
            ))
            .permission_version;

        let entry = RetrievalCacheEntry {
            retrieval_key: Self::compute_key(spec),
            tenant_id: spec.tenant_id.clone(),
            application_id: spec.application_id.clone(),
            query: spec.query.clone(),
            index_version: spec.index_version.clone(),
            embedding_fingerprint: spec.embedding_fingerprint.clone(),
            filters_hash: Self::hash_filters(&spec.filters),
            acl_scope: spec.acl_scope.clone(),
            permission_version: perm_ver,
            chunks,
            source_epochs,
            retrieved_at: Utc::now(),
        };
        write_lock(&self.entries).insert(entry.retrieval_key.clone(), entry.clone());
        entry
    }

    pub async fn lookup(
        &self,
        spec: &RetrievalQuerySpec,
        epoch_mgr: &EpochLeaseManager,
        temporal_engine: &TemporalValidityEngine,
    ) -> RetrievalLookupOutcome {
        let key = Self::compute_key(spec);
        let entry_opt = read_lock(&self.entries).get(&key).cloned();
        let Some(entry) = entry_opt else {
            return RetrievalLookupOutcome {
                hit: false,
                retrieval_key: key,
                reject_reason: Some(RetrievalRejectReason::CacheMiss),
                chunks: Vec::new(),
            };
        };

        if entry.index_version != spec.index_version {
            return Self::reject(key, RetrievalRejectReason::IndexVersionMismatch);
        }
        if entry.embedding_fingerprint != spec.embedding_fingerprint {
            return Self::reject(key, RetrievalRejectReason::EmbeddingFingerprintMismatch);
        }
        if entry.filters_hash != Self::hash_filters(&spec.filters) {
            return Self::reject(key, RetrievalRejectReason::FilterMismatch);
        }
        if entry.acl_scope != spec.acl_scope {
            return Self::reject(key, RetrievalRejectReason::AclScopeMismatch);
        }

        let current_perm_ver = temporal_engine
            .get_or_init_state(crate::AppState::tenant_permission_scope_uuid(
                &spec.tenant_id.0,
            ))
            .permission_version;
        let required_perm = spec
            .expected_permission_version
            .unwrap_or(entry.permission_version);
        if current_perm_ver > entry.permission_version || current_perm_ver > required_perm {
            return Self::reject(key, RetrievalRejectReason::PermissionRevoked);
        }

        for chunk in &entry.chunks {
            if chunk.deleted || spec.deleted_source_ids.contains(&chunk.source_id) {
                return Self::reject(key, RetrievalRejectReason::SourceDeleted);
            }
            if let Some(current_hash) = spec.current_source_hashes.get(&chunk.source_id) {
                if current_hash != &chunk.content_hash {
                    return Self::reject(key, RetrievalRejectReason::SourceChunkChanged);
                }
            }
            let current_ep = epoch_mgr.acquire_lease(chunk.source_id).await;
            let cached_ep = *entry.source_epochs.get(&chunk.source_id).unwrap_or(&0);
            if current_ep > cached_ep {
                return Self::reject(key, RetrievalRejectReason::SourceEpochStale);
            }
        }

        RetrievalLookupOutcome {
            hit: true,
            retrieval_key: key,
            reject_reason: None,
            chunks: entry.chunks,
        }
    }

    pub fn invalidate_by_source(&self, source_id: Uuid) -> usize {
        let mut entries = write_lock(&self.entries);
        let before = entries.len();
        entries.retain(|_, e| !e.chunks.iter().any(|c| c.source_id == source_id));
        before.saturating_sub(entries.len())
    }

    pub fn entry_count(&self) -> usize {
        read_lock(&self.entries).len()
    }

    fn reject(key: String, reason: RetrievalRejectReason) -> RetrievalLookupOutcome {
        RetrievalLookupOutcome {
            hit: false,
            retrieval_key: key,
            reject_reason: Some(reason),
            chunks: Vec::new(),
        }
    }
}
