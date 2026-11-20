//! Handlers grouped by domain; see `super` for shared imports.

use super::*;

#[derive(Deserialize)]
pub struct InvalidationRequest {
    pub source_id: Uuid,
    pub tenant_id: String,
    pub version_tag: String,
}

pub async fn trigger_invalidation(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<InvalidationRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    // Durable-first ordering: persist a PENDING intent before mutating state so a
    // crash or partial failure is recoverable and visible. The event is only
    // marked PROCESSED after every store and the cluster broadcast succeed.
    let tenant_id = TenantId::new(payload.tenant_id.clone());
    let outbox_record = sunborn_db::models::OutboxRecord {
        id: 0,
        tenant_id: payload.tenant_id.clone(),
        entity_type: "knowledge_source".to_string(),
        entity_id: payload.source_id.to_string(),
        action: "invalidate".to_string(),
        payload: json!({
            "source_id": payload.source_id,
            "version_tag": payload.version_tag,
        }),
        status: "PENDING".to_string(),
        created_at: Utc::now(),
    };
    let outbox_id = state
        .db
        .insert_outbox_event(outbox_record)
        .await
        .map_err(AppError::from)?;

    let purge_report = match state
        .orchestrator
        .purge_sources(&tenant_id, &[payload.source_id])
        .await
    {
        Ok(report) => report,
        Err(err) => {
            let _ = state
                .db
                .mark_outbox_failed(outbox_id, &err.to_string())
                .await;
            return Err(AppError::from(err));
        }
    };

    // Deterministic crash-injection point for the live acceptance harness: the
    // purge has committed but the ACK has not. Never armed in production.
    if std::env::var("SUNBORN_FAULT_INJECT_OUTBOX_CRASH").as_deref() == Ok("after_purge")
        && !sunborn_core::is_production_mode()
    {
        tracing::error!(outbox_id, "FAULT INJECTION: aborting between purge and ACK");
        std::process::abort();
    }

    if let Err(err) = state.db.mark_outbox_processed(outbox_id).await {
        // The purge completed, but we cannot confirm the durable acknowledgement.
        // Return an error so the caller knows the event may be retried (deletes
        // are idempotent). Do not report a clean success.
        return Err(AppError::from(err));
    }

    Ok(Json(json!({
        "status": "processed",
        "outbox_id": outbox_id,
        "source_id": payload.source_id,
        "bumped_epoch": purge_report.bumped_epoch,
        "bumped_watermark": purge_report.watermark_advanced,
        "evicted_cache_count": purge_report.purged_cache_keys.len(),
        "purged_cache_keys": purge_report.purged_cache_keys,
        "failed_cache_keys": purge_report.failed_cache_keys,
        "cluster_broadcast_sent": purge_report.cluster_broadcast_sent,
    })))
}

#[derive(Deserialize)]
pub struct DependencyRecordRequest {
    pub source_id: Uuid,
    pub cache_key: String,
}

pub async fn record_dependency(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<DependencyRecordRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    state
        .dependency_graph
        .record_dependency(payload.source_id, &payload.cache_key)
        .await
        .map_err(AppError::from)?;

    Ok(Json(json!({
        "status": "recorded",
        "source_id": payload.source_id,
        "cache_key": payload.cache_key
    })))
}

#[derive(Deserialize)]
pub struct GraphRtcInvalidateRequest {
    #[serde(default)]
    pub source_ids: Vec<Uuid>,
    #[serde(default)]
    pub doc_hashes: Vec<u64>,
}

pub async fn get_graph_stats(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    let (total_sources, total_edges) = state.dependency_graph.get_graph_stats().await;
    let (doc_count, total_assocs) = if let Some(ref gmc) = state.graph_memory_cache {
        (
            gmc.lineage_graph().document_count(),
            gmc.lineage_graph().total_associations(),
        )
    } else {
        (0, 0)
    };

    Ok(Json(json!({
        "status": "healthy",
        "graph_dependency_cache_enabled": state.graph_memory_cache.is_some(),
        "total_sources": total_sources,
        "total_edges": total_edges,
        "total_doc_hashes": doc_count,
        "total_doc_associations": total_assocs,
        "node_count": total_sources,
        "edge_count": total_edges,
        "cached_entries": total_assocs,
        "max_capacity": 65536,
        "invalidated_records": 0,
        "hits": 0,
        "misses": 0
    })))
}

pub async fn get_graph_lineage(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    let upstream_sources = state.dependency_graph.get_upstream_sources(&key).await;
    let is_cached = state.cache.exists(&key).await.unwrap_or(false);

    Ok(Json(json!({
        "cache_key": key,
        "key": key,
        "is_cached": is_cached,
        "upstream_sources": upstream_sources,
        "upstream_source_ids": upstream_sources,
        "upstream_source_count": upstream_sources.len()
    })))
}

pub async fn invalidate_graph_rtc(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<GraphRtcInvalidateRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    let mut purged_keys = Vec::new();
    let mut failed_keys = Vec::new();
    let mut affected_doc_records_count = 0;

    for src in &payload.source_ids {
        // Fence before deletion: bump the authoritative epoch and write-back watermark
        // first so a concurrent completion cannot repopulate the entry mid-purge.
        state
            .epoch_lease_manager
            .bump_epoch(*src)
            .await
            .map_err(AppError::from)?;
        state.watermark_fencer.advance_watermark(&src.to_string());

        // A dependency-graph outage must fail the invalidation rather than look
        // like "this source has no dependants". Reporting success here would
        // return a stale-serving cache and clear the lineage needed to retry.
        let keys = state
            .dependency_graph
            .get_affected_cache_keys(src)
            .await
            .map_err(AppError::from)?;
        let mut successfully_deleted_for_src = Vec::new();

        for k in &keys {
            let mut key_failed = false;
            // Tier 1: Delete from exact cache
            if let Err(e) = state.cache.delete(k).await {
                tracing::error!(source_id = %src, key = %k, error = %e, "Exact cache delete failed during RTC");
                failed_keys.push((k.clone(), format!("exact_cache: {e}")));
                key_failed = true;
            }

            // Tier 2: Delete from vector store
            if let Err(e) = state.vector_store.delete(k).await {
                tracing::error!(source_id = %src, key = %k, error = %e, "Vector store delete failed during RTC");
                failed_keys.push((k.clone(), format!("vector_store: {e}")));
                key_failed = true;
            }

            if !key_failed {
                successfully_deleted_for_src.push(k.clone());
                purged_keys.push(k.clone());
                // Remove individual dependency edge upon confirmed multi-tier deletion
                if let Err(e) = state.dependency_graph.remove_dependency(src, k).await {
                    tracing::error!(source_id = %src, key = %k, error = %e, "Graph edge removal failed");
                    failed_keys.push((k.clone(), format!("dependency_graph: {e}")));
                }
            }
        }

        // Tier 3: Invalidate graph memory cache if configured
        if let Some(ref gmc) = state.graph_memory_cache {
            match gmc.invalidate_source_rtc(src).await {
                Ok(gmc_keys) => {
                    purged_keys.extend(gmc_keys);
                }
                Err(e) => {
                    tracing::error!(source_id = %src, error = %e, "Graph memory cache purge failed during RTC invalidation");
                    failed_keys.push((format!("graph_memory:{src}"), e.to_string()));
                }
            }
        }

        // If all keys for this source were successfully deleted, clear remaining source dependencies
        if successfully_deleted_for_src.len() == keys.len() {
            state
                .dependency_graph
                .clear_dependencies(src)
                .await
                .map_err(AppError::from)?;
        }
    }

    if !payload.doc_hashes.is_empty() {
        if let Some(ref gmc) = state.graph_memory_cache {
            match gmc.invalidate_docs_rtc_purge(&payload.doc_hashes).await {
                Ok((affected, doc_keys)) => {
                    affected_doc_records_count = affected.len() as usize;
                    // Also purge corresponding vectors for doc keys. A failed vector
                    // delete is a failed purge, never a silent success.
                    for k in &doc_keys {
                        if let Err(e) = state.vector_store.delete(k).await {
                            tracing::error!(key = %k, error = %e, "Vector delete failed during document RTC purge");
                            failed_keys.push((k.clone(), format!("vector_store: {e}")));
                        } else {
                            purged_keys.push(k.clone());
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Document hash RTC purge failed");
                    failed_keys.push(("doc_hashes".to_string(), e.to_string()));
                }
            }
        }
    }

    purged_keys.sort();
    purged_keys.dedup();

    let count = purged_keys.len();
    let status = if failed_keys.is_empty() {
        "purged"
    } else if count > 0 {
        "partially_purged"
    } else {
        "failed"
    };

    Ok(Json(json!({
        "status": status,
        "purged_cache_keys": purged_keys,
        "purged_count": count,
        "evicted_records": count,
        "affected_doc_records_count": affected_doc_records_count,
        "failed_keys": failed_keys,
    })))
}

pub async fn get_cache(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Path(key): Path<String>,
) -> Result<Json<CacheEntry>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    match state.cache.get(&key).await {
        Ok(Some(entry)) => Ok(Json(entry)),
        Ok(None) => Err(SunbornError::new(
            sunborn_core::ErrorCode::CacheMiss,
            format!("Cache key '{}' not found", key),
        )
        .into()),
        Err(err) => Err(err.into()),
    }
}

pub async fn delete_cache(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    let deleted = state.cache.delete(&key).await.map_err(AppError::from)?;
    let vector_deleted = match state.vector_store.delete(&key).await {
        Ok(v) => v,
        Err(err) => {
            tracing::error!(key = %key, error = %err, "Vector store delete failed during cache deletion");
            if sunborn_core::is_production_mode() {
                return Err(AppError::from(err));
            }
            false
        }
    };
    Ok(Json(
        json!({ "key": key, "deleted": deleted, "vector_deleted": vector_deleted }),
    ))
}
