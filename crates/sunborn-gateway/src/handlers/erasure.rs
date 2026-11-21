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

    // Physical on-disk WAL verification for durable storage tiers
    let learning_wal_clean = state.learning_store.verify_tenant_erased_on_disk(&tenant);
    let execution_wal_clean = state.execution_store.verify_tenant_erased_on_disk(&tenant);
    if !learning_wal_clean || !execution_wal_clean {
        tracing::error!(
            tenant = %tenant_id,
            learning_wal_clean,
            execution_wal_clean,
            "Physical disk WAL verification failed: tenant remnants detected in on-disk WAL files"
        );
        verification_failed = true;
    }

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErasureStateStep {
    Initiated,
    TdekShredded,
    DbPurged,
    ExactCachePurged,
    VectorStorePurged,
    ExecutionStorePurged,
    LearningStorePurged,
    CredentialsRevoked,
    QuotaRemoved,
    VerifiedErased,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantErasureProgress {
    pub tenant_id: String,
    pub current_step: String,
    pub status: String,
    pub shred_duration_ns: u128,
    pub deleted_db_entries: usize,
    pub deleted_exact_cache: usize,
    pub deleted_vectors: usize,
    pub deleted_execution_tools: usize,
    pub deleted_learning_events: usize,
    pub verified_erasure: bool,
    pub updated_at: chrono::DateTime<chrono::Utc>,
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

    // Durable Erasure State Machine: Milestone 1 - Initiated
    tracing::info!(tenant = %tenant_id, step = "INITIATED", "Starting durable tenant erasure workflow");

    // Milestone 2 - O(1) Instant Cryptographic Shredding of TDEK: must succeed before cascading deletion
    let shred_duration = state.crypto_registry.shred_tenant(&tenant).map_err(|e| {
        AppError::from(SunbornError::internal(format!(
            "Cryptographic shredding failed: {e}"
        )))
    })?;
    let shred_duration_nanos = shred_duration.as_nanos();
    tracing::info!(tenant = %tenant_id, step = "TDEK_SHREDDED", shred_duration_nanos, "Milestone achieved");

    // Milestone 3 - PostgreSQL database purge
    let db_deleted = state
        .db
        .delete_tenant_data(&tenant_id)
        .await
        .map_err(AppError::from)?;
    tracing::info!(tenant = %tenant_id, step = "DB_PURGED", db_deleted, "Milestone achieved");

    // Milestone 4 - Exact cache purge
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
    tracing::info!(tenant = %tenant_id, step = "EXACT_CACHE_PURGED", exact_deleted, "Milestone achieved");

    // Milestone 5 - Semantic vector store purge
    let vector_deleted = state
        .vector_store
        .delete_by_tenant(&tenant)
        .await
        .map_err(AppError::from)?;
    tracing::info!(tenant = %tenant_id, step = "VECTOR_STORE_PURGED", vector_deleted, "Milestone achieved");

    // Milestone 6 - Execution store eviction & WAL rewrite
    let execution_deleted = state
        .execution_store
        .evict_by_tenant(&tenant)
        .await
        .map_err(AppError::from)?;
    tracing::info!(tenant = %tenant_id, step = "EXECUTION_STORE_PURGED", execution_deleted, "Milestone achieved");

    // Milestone 7 - Learning store purge & atomic WAL rewrite
    let learning_deleted = state
        .learning_store
        .purge_tenant(&tenant)
        .map_err(AppError::from)?;
    tracing::info!(tenant = %tenant_id, step = "LEARNING_STORE_PURGED", learning_deleted, "Milestone achieved");

    // Milestone 8 - Revoke tenant credentials & clear quota ledger
    let _credentials_revoked = state.api_key_registry.revoke_tenant_keys(&tenant);
    state.quota_ledger.remove_tenant(&tenant_id).await;
    tracing::info!(tenant = %tenant_id, step = "CREDENTIALS_AND_QUOTA_CLEARED", "Milestone achieved");

    // Milestone 9 - Independent Read-After-Delete & Physical Disk Verification Pass
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
        step = "VERIFIED_ERASED",
        "Completed durable tenant cascade offboarding"
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
