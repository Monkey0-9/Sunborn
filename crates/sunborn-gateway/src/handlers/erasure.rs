//! Handlers grouped by domain; see `super` for shared imports.

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErasureResidualCounts {
    pub db: usize,
    pub exact_cache: usize,
    pub vector_store: usize,
    pub execution_store: usize,
    pub learning_store: usize,
}

pub async fn verify_multi_store_erasure_integrity(
    state: &AppState,
    tenant_id: &str,
) -> (bool, ErasureResidualCounts) {
    let tenant = TenantId::new(tenant_id);
    let mut verification_failed = false;

    let db_res = match state.db.count_tenant_entries(tenant_id).await {
        Ok(count) => count,
        Err(err) => {
            tracing::error!(tenant = %tenant_id, error = %err, "Database count check failed during erasure verification");
            verification_failed = true;
            1
        }
    };

    let prefix_sunborn = format!("sunborn:{}:", tenant_id);
    let prefix_legacy = format!("tenant:{}:", tenant_id);
    let exact_res_1 = match state.cache.count_by_prefix(&prefix_sunborn).await {
        Ok(count) => count,
        Err(err) => {
            tracing::error!(tenant = %tenant_id, error = %err, "Cache prefix check failed during erasure verification");
            verification_failed = true;
            1
        }
    };
    let exact_res_2 = match state.cache.count_by_prefix(&prefix_legacy).await {
        Ok(count) => count,
        Err(err) => {
            tracing::error!(tenant = %tenant_id, error = %err, "Legacy cache prefix check failed during erasure verification");
            verification_failed = true;
            1
        }
    };
    let exact_res = exact_res_1 + exact_res_2;

    let vector_res = match state.vector_store.count(&tenant).await {
        Ok(count) => count,
        Err(err) => {
            tracing::error!(tenant = %tenant_id, error = %err, "Vector store count check failed during erasure verification");
            verification_failed = true;
            1
        }
    };

    let execution_res = match state.execution_store.count_by_tenant(&tenant).await {
        Ok(count) => count,
        Err(err) => {
            tracing::error!(tenant = %tenant_id, error = %err, "Execution store count check failed during erasure verification");
            verification_failed = true;
            1
        }
    };

    let learning_res = state.learning_store.tenant_event_count(&tenant);

    let residuals = ErasureResidualCounts {
        db: db_res,
        exact_cache: exact_res,
        vector_store: vector_res,
        execution_store: execution_res,
        learning_store: learning_res,
    };

    let all_zero = !verification_failed
        && db_res == 0
        && exact_res == 0
        && vector_res == 0
        && execution_res == 0
        && learning_res == 0;

    if !all_zero {
        tracing::warn!(
            tenant = %tenant_id,
            ?residuals,
            verification_failed,
            "Multi-store erasure integrity verification FAILED: residual records or check errors found"
        );
    } else {
        tracing::info!(
            tenant = %tenant_id,
            "Multi-store erasure integrity verification SUCCEEDED: 0 residual records across all 5 tiers"
        );
    }

    (all_zero, residuals)
}

pub async fn delete_tenant(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    headers: axum::http::HeaderMap,
    Path(tenant_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let quorum_key = headers
        .get("x-quorum-admin-key")
        .and_then(|v| v.to_str().ok());
    sec_ctx
        .require_admin_quorum(quorum_key, &state.api_key_registry)
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    let tenant = TenantId::new(&tenant_id);

    // 0. O(1) Instant Cryptographic Shredding of TDEK: must succeed before cascading deletion
    let shred_duration = state.crypto_registry.shred_tenant(&tenant).map_err(|e| {
        AppError::from(SunbornError::internal(format!(
            "Cryptographic shredding failed: {e}"
        )))
    })?;
    let shred_duration_nanos = shred_duration.as_nanos();

    let db_deleted = state
        .db
        .delete_tenant_data(&tenant_id)
        .await
        .map_err(AppError::from)?;
    let prefix_sunborn = format!("sunborn:{}:", tenant_id);
    let exact_deleted_sunborn = state
        .cache
        .delete_by_prefix(&prefix_sunborn)
        .await
        .map_err(AppError::from)?;
    let prefix_legacy = format!("tenant:{}:", tenant_id);
    let exact_deleted_legacy = state
        .cache
        .delete_by_prefix(&prefix_legacy)
        .await
        .map_err(AppError::from)?;
    let exact_deleted = exact_deleted_sunborn + exact_deleted_legacy;

    let vector_deleted = state
        .vector_store
        .delete_by_tenant(&tenant)
        .await
        .map_err(AppError::from)?;
    let execution_deleted = state
        .execution_store
        .evict_by_tenant(&tenant)
        .await
        .map_err(AppError::from)?;
    let learning_deleted = state.learning_store.purge_tenant(&tenant);
    let _credentials_revoked = state.api_key_registry.revoke_tenant_keys(&tenant);
    state.quota_ledger.remove_tenant(&tenant_id).await;

    // 1. Independent Read-After-Delete Verification Pass
    let (verified_erasure, residuals) =
        verify_multi_store_erasure_integrity(&state, &tenant_id).await;
    let final_status = if verified_erasure {
        "erased"
    } else {
        "partial_failure"
    };

    tracing::info!(
        tenant = %tenant_id,
        shred_duration_nanos,
        db_deleted,
        exact_deleted,
        vector_deleted,
        execution_deleted,
        learning_deleted,
        verified_multi_store_erasure = verified_erasure,
        "Completed tenant cascade offboarding"
    );

    Ok(Json(json!({
        "tenant_id": tenant_id,
        "status": final_status,
        "cryptographic_shred_time_ns": shred_duration_nanos,
        "deleted_db_entries": db_deleted,
        "deleted_exact_cache": exact_deleted,
        "deleted_vectors": vector_deleted,
        "deleted_execution_tools": execution_deleted,
        "deleted_learning_events": learning_deleted,
        "verified_multi_store_erasure": verified_erasure,
        "residual_records": {
            "db": residuals.db,
            "exact_cache": residuals.exact_cache,
            "vector_store": residuals.vector_store,
            "execution_store": residuals.execution_store,
            "learning_store": residuals.learning_store,
        }
    })))
}
