//! Handlers grouped by domain; see `super` for shared imports.

use super::*;

pub async fn trigger_rebuild(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;
    let entries = state
        .db
        .scan_cache_entries(100000, 0)
        .await
        .map_err(AppError::from)?;
    let total = entries.len();
    let mut exact_restored = 0;
    let mut vector_restored = 0;

    for record in entries {
        // 1. Repopulate exact cache
        let ttl_secs = record
            .expires_at
            .map(|exp| (exp - Utc::now()).num_seconds().max(1) as u64)
            .unwrap_or(state.config.cache.default_ttl_secs);

        // Strict Provenance Recovery (P0-03): Rebuild authentic certificate from durable store.
        // If certificate is missing, malformed, or fails cryptographic integrity, fail closed
        // and never invent fake positive evidence or empty snapshots.
        let restored_cert = match record.certificate_json.as_deref() {
            Some(cert_str) => {
                match serde_json::from_str::<sunborn_core::ReuseCertificate>(cert_str) {
                    Ok(cert) => {
                        if cert
                            .verify_integrity(&state.certificate_signing_key)
                            .is_ok()
                        {
                            Some(cert)
                        } else {
                            tracing::warn!(
                                key = %record.key,
                                "Stored certificate integrity check failed during rebuild -> skipping unverified cache admission"
                            );
                            None
                        }
                    }
                    Err(err) => {
                        tracing::warn!(
                            key = %record.key,
                            error = %err,
                            "Failed to deserialize stored certificate during rebuild -> skipping cache admission"
                        );
                        None
                    }
                }
            }
            None => {
                tracing::warn!(
                    key = %record.key,
                    "No authentic certificate found in durable record -> skipping cache admission (fail-closed)"
                );
                None
            }
        };

        let restored_cert = match restored_cert {
            Some(c) => c,
            None => continue, // Never admit cache entries without authentic cryptographic provenance
        };

        let mut entry = CacheEntry::new(&record.key, &record.content, Some(ttl_secs));
        if let Ok(cert_val) = serde_json::to_value(&restored_cert) {
            entry.metadata["certificate"] = cert_val.clone();
        }
        if let Some(env) = parse_envelope_from_stored(&record.content) {
            if let Ok(env_val) = serde_json::to_value(&env) {
                entry.metadata["encrypted_envelope"] = env_val;
            }
        }
        if state.cache.set(entry.clone()).await.is_ok() {
            exact_restored += 1;
            if let Some(ref gmc) = state.graph_memory_cache {
                let source_ids: Vec<Uuid> = restored_cert
                    .source_snapshots
                    .iter()
                    .map(|s| s.source_id)
                    .collect();
                let doc_hashes: Vec<u64> = restored_cert
                    .source_snapshots
                    .iter()
                    .map(|s| {
                        let mut bytes = Vec::with_capacity(64);
                        bytes.extend_from_slice(s.source_id.as_bytes());
                        bytes.extend_from_slice(s.content_hash.as_bytes());
                        sunborn_invalidation::deterministic_hash_64(&bytes)
                    })
                    .collect();
                let record_id = sunborn_invalidation::deterministic_hash_64(record.key.as_bytes());
                if let Err(e) = gmc
                    .set_with_lineage(entry, &source_ids, &doc_hashes, record_id)
                    .await
                {
                    tracing::warn!(key = %record.key, error = %e, "Failed to repopulate graph memory cache entry");
                }
            }
        }

        // 2. Repopulate vector store
        if let Ok(vec) = state
            .selected_embedder
            .embed(&record.prompt_canonical)
            .await
        {
            let mut meta = serde_json::json!({
                "model": record.model,
                "certificate": serde_json::to_value(&restored_cert).unwrap_or_default(),
            });
            if let Some(env) = parse_envelope_from_stored(&record.content) {
                if let Ok(env_val) = serde_json::to_value(&env) {
                    meta["encrypted_envelope"] = env_val;
                }
            }
            let ventry = VectorEntry {
                id: record.key.clone(),
                tenant_id: TenantId::new(record.tenant_id),
                application_id: sunborn_core::model::ApplicationId::new(record.application_id),
                prompt: record.prompt_canonical,
                content: record.content,
                vector: vec,
                metadata: meta,
                created_at: record.created_at,
            };
            if state.vector_store.insert(ventry).await.is_ok() {
                vector_restored += 1;
            }
        }
    }

    Ok(Json(json!({
        "status": "completed",
        "scanned_db_records": total,
        "rebuilt_exact_cache": exact_restored,
        "rebuilt_vectors": vector_restored
    })))
}

pub async fn sweep_lifecycle(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;
    let swept = state.cache.sweep_expired().await.map_err(AppError::from)?;
    Ok(Json(json!({
        "status": "completed",
        "swept_entries": swept
    })))
}

/// Requeue outbox events parked in FAILED state (the effective DLQ).
///
/// Recovery step for the `SunbornInvalidationDLQCountHigh` alert. Requeueing
/// rather than deleting means the same invalidation is retried by the worker;
/// the historical `outbox_dlq` audit rows are left untouched.
pub async fn replay_outbox_dlq(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let quorum_key = headers
        .get("x-quorum-admin-key")
        .and_then(|v| v.to_str().ok());
    sec_ctx
        .require_admin_quorum(quorum_key, &state.api_key_registry)
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    let replayed = state
        .db
        .replay_outbox_dlq(OUTBOX_REPLAY_BATCH)
        .await
        .map_err(AppError::from)?;
    let remaining = state
        .db
        .failed_outbox_depth()
        .await
        .map_err(AppError::from)?;

    if remaining > 0 {
        tracing::warn!(
            replayed,
            remaining,
            batch = OUTBOX_REPLAY_BATCH,
            "Outbox DLQ replay left events pending; call again to continue draining"
        );
    }

    Ok(Json(json!({
        "status": if remaining == 0 { "drained" } else { "partial" },
        "replayed": replayed,
        "remaining_failed": remaining,
        "batch_size": OUTBOX_REPLAY_BATCH,
    })))
}

/// Bounded per call so a large DLQ cannot turn one admin request into an
/// unbounded write burst. Operators call repeatedly, or schedule it.
const OUTBOX_REPLAY_BATCH: usize = 100;

pub async fn chaos_status(State(state): State<AppState>) -> impl IntoResponse {
    let status = state.circuit_breaker.status().await;
    Json(json!({
        "circuit_status": format!("{:?}", status)
    }))
}

pub async fn chaos_trip(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let quorum_key = headers
        .get("x-quorum-admin-key")
        .and_then(|v| v.to_str().ok());
    sec_ctx
        .require_admin_quorum(quorum_key, &state.api_key_registry)
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;
    state.circuit_breaker.trip().await;
    Ok(Json(json!({
        "status": "tripped",
        "circuit_status": "Open"
    })))
}

pub async fn chaos_reset(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let quorum_key = headers
        .get("x-quorum-admin-key")
        .and_then(|v| v.to_str().ok());
    sec_ctx
        .require_admin_quorum(quorum_key, &state.api_key_registry)
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;
    state.circuit_breaker.reset().await;
    Ok(Json(json!({
        "status": "reset",
        "circuit_status": "Closed"
    })))
}
