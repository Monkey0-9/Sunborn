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
/// Counts semantic evaluations that returned an error and degraded to origin.
/// Without this an embedding/vector outage is invisible: requests still succeed,
/// so neither latency nor error rate reveals that the cache tier is gone.
static SEMANTIC_EVAL_FAILURES: AtomicU64 = AtomicU64::new(0);
static CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
static RATE_LIMITED_TOTAL: AtomicU64 = AtomicU64::new(0);
static SPEND_VELOCITY_REJECTIONS_TOTAL: AtomicU64 = AtomicU64::new(0);
static QUOTA_REJECTIONS_TOTAL: AtomicU64 = AtomicU64::new(0);
static REQUEST_DURATION_MS_SUM: AtomicU64 = AtomicU64::new(0);

/// Prometheus histogram buckets for end-to-end request latency, in seconds.
/// "+Inf" is implied by the total observation count (`REQUEST_OBSERVATIONS`).
pub(crate) const LATENCY_BUCKETS_SECS: [f64; 12] = [
    0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
];

/// One cumulative counter per bucket, plus the observation count and sum.
/// Buckets are stored as cumulative counts at render time: each observation
/// increments every bucket whose upper bound it falls at or below.
static LATENCY_BUCKET_COUNTS: [AtomicU64; LATENCY_BUCKETS_SECS.len()] = [
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
];
static REQUEST_OBSERVATIONS: AtomicU64 = AtomicU64::new(0);

/// Process start time, used to derive `sunborn_uptime_seconds`.
static PROCESS_START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Records one completed request in the latency histogram.
pub(crate) fn observe_request_latency(duration_ms: u64) {
    let seconds = duration_ms as f64 / 1000.0;
    REQUEST_OBSERVATIONS.fetch_add(1, Ordering::Relaxed);
    for (idx, bound) in LATENCY_BUCKETS_SECS.iter().enumerate() {
        if seconds <= *bound {
            LATENCY_BUCKET_COUNTS[idx].fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Pins the process start instant. Call once during startup so
/// `sunborn_uptime_seconds` reflects real process age rather than the first
/// scrape.
/// Counts invalidation requests that left at least one cache key or edge
/// un-evicted. A stale cache entry is a correctness bug, so this is a signal to
/// page on rather than a debug aid.
static INVALIDATION_PARTIAL_FAILURES: AtomicU64 = AtomicU64::new(0);

pub fn invalidation_partial_failures() -> u64 {
    INVALIDATION_PARTIAL_FAILURES.load(Ordering::Relaxed)
}

pub(crate) fn record_invalidation_partial_failure() {
    INVALIDATION_PARTIAL_FAILURES.fetch_add(1, Ordering::Relaxed);
}

/// Counts spend-ledger reservation failures. The request is rejected
/// (fail-closed) whenever this increments, so it is an availability signal, not
/// a warning: the tenant cannot spend until the ledger recovers.
static SPEND_LEDGER_ERRORS: AtomicU64 = AtomicU64::new(0);

pub fn spend_ledger_errors() -> u64 {
    SPEND_LEDGER_ERRORS.load(Ordering::Relaxed)
}

pub(crate) fn record_spend_ledger_error() {
    SPEND_LEDGER_ERRORS.fetch_add(1, Ordering::Relaxed);
}

/// Number of semantic evaluations that failed and fell through to origin.
pub fn semantic_eval_failures() -> u64 {
    SEMANTIC_EVAL_FAILURES.load(Ordering::Relaxed)
}

pub(crate) fn record_semantic_eval_failure() {
    SEMANTIC_EVAL_FAILURES.fetch_add(1, Ordering::Relaxed);
}

pub fn init_metrics() {
    PROCESS_START.get_or_init(Instant::now);
}

/// Returns process uptime in seconds.
pub(crate) fn process_uptime_seconds() -> f64 {
    PROCESS_START
        .get_or_init(Instant::now)
        .elapsed()
        .as_secs_f64()
}

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
mod gateway_ext;
mod governance;
mod graph;
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
pub use gateway_ext::*;
pub use governance::*;
pub use graph::*;
pub use health::*;
pub use invalidation::*;
pub use lifecycle::*;
pub use llm::*;
pub use local::*;
pub use platform::*;
pub use security::*;
pub use tenants::*;

#[cfg(test)]
mod metrics_tests {
    use super::*;
    use std::sync::atomic::Ordering;

    fn snapshot() -> (Vec<u64>, u64) {
        let buckets = (0..LATENCY_BUCKETS_SECS.len())
            .map(|idx| LATENCY_BUCKET_COUNTS[idx].load(Ordering::Relaxed))
            .collect();
        (buckets, REQUEST_OBSERVATIONS.load(Ordering::Relaxed))
    }

    /// Prometheus requires cumulative buckets: an observation must increment
    /// every bucket whose upper bound it falls at or below. A non-cumulative
    /// implementation still renders, but every rate() calculation over it is
    /// silently wrong and `le="+Inf"` stops matching `_count`.
    #[test]
    fn latency_buckets_are_cumulative_and_agree_with_the_observation_count() {
        let (before, observations_before) = snapshot();

        observe_request_latency(4); // 4ms: falls in le=0.005 and every larger bound
        observe_request_latency(2_000); // 2s: falls in le=2.5 and le=5.0 only

        let (after, observations_after) = snapshot();
        assert_eq!(observations_after - observations_before, 2);

        let delta: Vec<u64> = before
            .iter()
            .zip(after.iter())
            .map(|(b, a)| a - b)
            .collect();

        // le=0.001 and le=0.0025 are below 0.004, so neither observation counts.
        assert_eq!(delta[0], 0, "le=0.001 must not count a 4ms observation");
        assert_eq!(delta[1], 0, "le=0.0025 must not count a 4ms observation");
        // le=0.005 .. le=2.0 include the 4ms request but not the 2s one.
        for (idx, count) in delta.iter().enumerate().take(10).skip(2) {
            assert_eq!(
                *count, 1,
                "le={} should count only the 4ms observation",
                LATENCY_BUCKETS_SECS[idx]
            );
        }
        // le=2.5 and le=5.0 include both observations.
        assert_eq!(delta[10], 2, "le=2.5 must count both observations");
        assert_eq!(delta[11], 2, "le=5.0 must count both observations");

        // Cumulative order: every bucket is >= the one below it.
        for window in after.windows(2) {
            assert!(
                window[1] >= window[0],
                "bucket counts must be non-decreasing as the bound grows"
            );
        }
    }
}
