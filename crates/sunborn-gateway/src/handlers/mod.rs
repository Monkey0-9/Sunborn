//! HTTP handlers, grouped by domain.
//!
//! This module used to be a single 3,755-line file that was excluded from
//! `rustfmt`. It is now split along the existing domain seams (health probes,
//! request execution, LLM-compatible APIs, spend controls, invalidation,
//! execution planning, erasure, lifecycle, tenant administration, platform
//! primitives, credentials, errors, security, and local BYOK). Each child gets
//! the shared imports and counters through `use super::*`, and the glob
//! re-exports below keep `crate::handlers::<handler>` paths stable for the
//! router.

use crate::middleware::auth::SecurityContext;
use crate::quota::QuotaError;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use sunborn_cache::CacheEntry;
use sunborn_core::sync::read_lock;
use sunborn_core::{
    finops::{
        format_inr_currency, format_lakhs_crores, FinOpsEngine, GstBreakdown, MicroPaise,
        ModelPricing,
    },
    CachePath, DecisionAction, DecisionRecord, RequestEnvelope, ResponseEnvelope, SunbornError,
    TenantId,
};
use sunborn_identity::{EncryptedEnvelope, IdentityEngine};
use sunborn_semantic::{EmbeddingProvider, VectorEntry, VectorStore};
use uuid::Uuid;

static REQUESTS_TOTAL: AtomicU64 = AtomicU64::new(0);
static EXACT_HITS: AtomicU64 = AtomicU64::new(0);
static SEMANTIC_HITS: AtomicU64 = AtomicU64::new(0);
static CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
static RATE_LIMITED_TOTAL: AtomicU64 = AtomicU64::new(0);
static SPEND_VELOCITY_REJECTIONS_TOTAL: AtomicU64 = AtomicU64::new(0);
static QUOTA_REJECTIONS_TOTAL: AtomicU64 = AtomicU64::new(0);
static REQUEST_DURATION_MS_SUM: AtomicU64 = AtomicU64::new(0);

/// Measures safety evidence by comparing the certificate's bound reference
/// fingerprint against the candidate prompt. Returns `None` for legacy
/// certificates with no reference fingerprint.
fn measured_safety_evidence(
    cert: &sunborn_core::ReuseCertificate,
    candidate_prompt: &str,
    calibrated_risk_bound: f32,
) -> Option<sunborn_core::SafetyEvidenceRecord> {
    if cert.reference_fingerprint.is_empty() {
        return None;
    }

    let reference = serde_json::from_str::<sunborn_semantic::StructuralFingerprint>(
        &cert.reference_fingerprint,
    )
    .ok()?;
    let candidate = sunborn_semantic::StructuralFingerprint::extract(candidate_prompt);

    let negation_match = reference.negation_polarity_bit == candidate.negation_polarity_bit
        && reference.negation_antonym_tokens == candidate.negation_antonym_tokens
        && reference.quantifier_scope_tokens == candidate.quantifier_scope_tokens;
    let numeric_match = reference.sorted_numeric_literals == candidate.sorted_numeric_literals
        && (reference.role_bound_numeric_tokens.is_empty()
            || candidate.role_bound_numeric_tokens.is_empty()
            || reference.role_bound_numeric_tokens == candidate.role_bound_numeric_tokens)
        && (reference.ordered_numeric_sequence.is_empty()
            || candidate.ordered_numeric_sequence.is_empty()
            || reference.ordered_numeric_sequence == candidate.ordered_numeric_sequence);
    let entity_drift = reference.entity_tokens != candidate.entity_tokens;
    let matched = reference.matches(&candidate);

    Some(sunborn_core::SafetyEvidenceRecord {
        structural_hash_match: matched,
        negation_polarity_match: negation_match,
        numeric_literal_match: numeric_match,
        entity_drift_detected: entity_drift,
        non_conformity_score: if matched { 0.0 } else { 1.0 },
        calibrated_risk_bound,
        heuristic_risk_score: sunborn_semantic::DecisionEngine::<
            std::sync::Arc<dyn sunborn_semantic::EmbeddingProvider>,
            std::sync::Arc<dyn sunborn_semantic::VectorStore>,
        >::assess_risk(candidate_prompt),
    })
}

mod authz;
mod controls;
mod erasure;
mod errors;
mod execute;
mod execution;
mod health;
mod invalidation;
mod lifecycle;
mod llm;
mod local;
mod platform;
mod security;
mod tenants;

pub use authz::*;
pub use controls::*;
pub use erasure::*;
pub use errors::*;
pub use execute::*;
pub use execution::*;
pub use health::*;
pub use invalidation::*;
pub use lifecycle::*;
pub use llm::*;
pub use local::*;
pub use platform::*;
pub use security::*;
pub use tenants::*;
