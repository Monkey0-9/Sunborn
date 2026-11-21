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

    // 1. Create or retrieve active durable task from DB
    let mut task = match state.db.get_active_erasure_task(&tenant_id).await {
        Ok(Some(existing)) => existing,
        _ => state
            .db
            .create_erasure_task(&tenant_id)
            .await
            .map_err(AppError::from)?,
    };

    // Milestone 1 & 2 - O(1) Instant Cryptographic Shredding of TDEK: must succeed before cascading deletion
    if task.current_step == "INITIATED" {
        let shred_duration = state.crypto_registry.shred_tenant(&tenant).map_err(|e| {
            let err_msg = format!("Cryptographic shredding failed: {e}");
            task.status = "FAILED".to_string();
            task.last_error = Some(err_msg.clone());
            let db_clone = state.db.clone();
            let task_clone = task.clone();
            tokio::spawn(async move {
                let _ = db_clone.update_erasure_task(&task_clone).await;
            });
            AppError::from(SunbornError::internal(err_msg))
        })?;
        let shred_duration_nanos = shred_duration.as_nanos() as i64;
        task.shred_duration_ns = Some(shred_duration_nanos);
        task.current_step = "TDEK_SHREDDED".to_string();
        state
            .db
            .update_erasure_task(&task)
            .await
            .map_err(AppError::from)?;
        tracing::info!(tenant = %tenant_id, step = "TDEK_SHREDDED", shred_duration_nanos, "Milestone achieved and persisted to DB");
    }

    // Milestone 3 - PostgreSQL database purge
    if task.current_step == "TDEK_SHREDDED" {
        let db_deleted = state.db.delete_tenant_data(&tenant_id).await.map_err(|e| {
            task.status = "FAILED".to_string();
            task.last_error = Some(e.to_string());
            let db_clone = state.db.clone();
            let task_clone = task.clone();
            tokio::spawn(async move {
                let _ = db_clone.update_erasure_task(&task_clone).await;
            });
            AppError::from(e)
        })?;
        task.deleted_db_entries += db_deleted as i32;
        task.current_step = "DB_PURGED".to_string();
        state
            .db
            .update_erasure_task(&task)
            .await
            .map_err(AppError::from)?;
        tracing::info!(tenant = %tenant_id, step = "DB_PURGED", db_deleted, "Milestone achieved and persisted to DB");
    }

    // Milestone 4 - Exact cache purge
    if task.current_step == "DB_PURGED" {
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
        task.deleted_exact_cache += exact_deleted as i32;
        task.current_step = "EXACT_CACHE_PURGED".to_string();
        state
            .db
            .update_erasure_task(&task)
            .await
            .map_err(AppError::from)?;
        tracing::info!(tenant = %tenant_id, step = "EXACT_CACHE_PURGED", exact_deleted, "Milestone achieved and persisted to DB");
    }

    // Milestone 5 - Semantic vector store purge
    if task.current_step == "EXACT_CACHE_PURGED" {
        let vector_deleted = state
            .vector_store
            .delete_by_tenant(&tenant)
            .await
            .map_err(AppError::from)?;
        task.deleted_vectors += vector_deleted as i32;
        task.current_step = "VECTOR_STORE_PURGED".to_string();
        state
            .db
            .update_erasure_task(&task)
            .await
            .map_err(AppError::from)?;
        tracing::info!(tenant = %tenant_id, step = "VECTOR_STORE_PURGED", vector_deleted, "Milestone achieved and persisted to DB");
    }

    // Milestone 6 - Execution store eviction & WAL rewrite
    if task.current_step == "VECTOR_STORE_PURGED" {
        let execution_deleted = state
            .execution_store
            .evict_by_tenant(&tenant)
            .await
            .map_err(AppError::from)?;
        task.deleted_execution_tools += execution_deleted as i32;
        task.current_step = "EXECUTION_STORE_PURGED".to_string();
        state
            .db
            .update_erasure_task(&task)
            .await
            .map_err(AppError::from)?;
        tracing::info!(tenant = %tenant_id, step = "EXECUTION_STORE_PURGED", execution_deleted, "Milestone achieved and persisted to DB");
    }

    // Milestone 7 - Learning store purge & atomic WAL rewrite
    if task.current_step == "EXECUTION_STORE_PURGED" {
        let learning_deleted = state
            .learning_store
            .purge_tenant(&tenant)
            .map_err(AppError::from)?;
        task.deleted_learning_events += learning_deleted as i32;
        task.current_step = "LEARNING_STORE_PURGED".to_string();
        state
            .db
            .update_erasure_task(&task)
            .await
            .map_err(AppError::from)?;
        tracing::info!(tenant = %tenant_id, step = "LEARNING_STORE_PURGED", learning_deleted, "Milestone achieved and persisted to DB");
    }

    // Milestone 8 - Revoke tenant credentials & clear quota ledger
    if task.current_step == "LEARNING_STORE_PURGED" {
        let _credentials_revoked = state.api_key_registry.revoke_tenant_keys(&tenant);
        state.quota_ledger.remove_tenant(&tenant_id).await;
        task.current_step = "CREDENTIALS_AND_QUOTA_CLEARED".to_string();
        state
            .db
            .update_erasure_task(&task)
            .await
            .map_err(AppError::from)?;
        tracing::info!(tenant = %tenant_id, step = "CREDENTIALS_AND_QUOTA_CLEARED", "Milestone achieved and persisted to DB");
    }

    // Milestone 9 - Independent Read-After-Delete & Physical Disk Verification Pass
    let (verified_erasure, residuals) =
        verify_multi_store_erasure_integrity(&state, &tenant_id).await;

    if verified_erasure {
        task.verified_integrity = true;
        task.status = "COMPLETED".to_string();
        task.current_step = "VERIFIED_ERASED".to_string();
        task.last_error = None;
    } else {
        task.verified_integrity = false;
        task.status = "FAILED".to_string();
        task.last_error = Some("Residual records detected during erasure verification".to_string());
    }
    let _ = state.db.update_erasure_task(&task).await;

    let final_status = if verified_erasure {
        "erased"
    } else {
        "partial_failure"
    };

    tracing::info!(
        tenant = %tenant_id,
        task_id = %task.task_id,
        shred_duration_nanos = task.shred_duration_ns.unwrap_or(0),
        db_deleted = task.deleted_db_entries,
        exact_deleted = task.deleted_exact_cache,
        vector_deleted = task.deleted_vectors,
        execution_deleted = task.deleted_execution_tools,
        learning_deleted = task.deleted_learning_events,
        verified_multi_store_erasure = verified_erasure,
        step = "VERIFIED_ERASED",
        "Completed durable restart-safe tenant cascade offboarding"
    );

    Ok(Json(json!({
        "task_id": task.task_id,
        "tenant_id": tenant_id,
        "status": final_status,
        "current_step": task.current_step,
        "cryptographic_shred_time_ns": task.shred_duration_ns.unwrap_or(0),
        "deleted_db_entries": task.deleted_db_entries,
        "deleted_exact_cache": task.deleted_exact_cache,
        "deleted_vectors": task.deleted_vectors,
        "deleted_execution_tools": task.deleted_execution_tools,
        "deleted_learning_events": task.deleted_learning_events,
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

/// Restart recovery engine: queries the database task table for any interrupted
/// or incomplete erasure workflows and resumes them through remaining steps.
pub async fn resume_interrupted_erasure_tasks(state: &AppState) -> Result<usize, SunbornError> {
    let incomplete = state.db.list_incomplete_erasure_tasks().await?;
    let mut resumed = 0;

    for mut task in incomplete {
        let tenant = TenantId::new(&task.tenant_id);
        tracing::warn!(
            tenant = %task.tenant_id,
            task_id = %task.task_id,
            step = %task.current_step,
            "Resuming interrupted durable tenant erasure state machine from database task table"
        );
        task.status = "IN_PROGRESS".to_string();
        let _ = state.db.update_erasure_task(&task).await;

        if task.current_step == "INITIATED" {
            match state.crypto_registry.shred_tenant(&tenant) {
                Ok(shred_dur) => {
                    task.shred_duration_ns = Some(shred_dur.as_nanos() as i64);
                    task.current_step = "TDEK_SHREDDED".to_string();
                    let _ = state.db.update_erasure_task(&task).await;
                }
                Err(e) => {
                    task.status = "FAILED".to_string();
                    task.last_error = Some(e.to_string());
                    let _ = state.db.update_erasure_task(&task).await;
                    continue;
                }
            }
        }

        if task.current_step == "TDEK_SHREDDED" {
            match state.db.delete_tenant_data(&task.tenant_id).await {
                Ok(del) => {
                    task.deleted_db_entries += del as i32;
                    task.current_step = "DB_PURGED".to_string();
                    let _ = state.db.update_erasure_task(&task).await;
                }
                Err(e) => {
                    task.status = "FAILED".to_string();
                    task.last_error = Some(e.to_string());
                    let _ = state.db.update_erasure_task(&task).await;
                    continue;
                }
            }
        }

        if task.current_step == "DB_PURGED" {
            let prefix_sunborn = format!("sunborn:{}:", task.tenant_id);
            let c1 = state
                .cache
                .delete_by_prefix(&prefix_sunborn)
                .await
                .unwrap_or(0);
            let prefix_legacy = format!("tenant:{}:", task.tenant_id);
            let c2 = state
                .cache
                .delete_by_prefix(&prefix_legacy)
                .await
                .unwrap_or(0);
            task.deleted_exact_cache += (c1 + c2) as i32;
            task.current_step = "EXACT_CACHE_PURGED".to_string();
            let _ = state.db.update_erasure_task(&task).await;
        }

        if task.current_step == "EXACT_CACHE_PURGED" {
            if let Ok(del) = state.vector_store.delete_by_tenant(&tenant).await {
                task.deleted_vectors += del as i32;
                task.current_step = "VECTOR_STORE_PURGED".to_string();
                let _ = state.db.update_erasure_task(&task).await;
            }
        }

        if task.current_step == "VECTOR_STORE_PURGED" {
            if let Ok(del) = state.execution_store.evict_by_tenant(&tenant).await {
                task.deleted_execution_tools += del as i32;
                task.current_step = "EXECUTION_STORE_PURGED".to_string();
                let _ = state.db.update_erasure_task(&task).await;
            }
        }

        if task.current_step == "EXECUTION_STORE_PURGED" {
            if let Ok(del) = state.learning_store.purge_tenant(&tenant) {
                task.deleted_learning_events += del as i32;
                task.current_step = "LEARNING_STORE_PURGED".to_string();
                let _ = state.db.update_erasure_task(&task).await;
            }
        }

        if task.current_step == "LEARNING_STORE_PURGED" {
            let _ = state.api_key_registry.revoke_tenant_keys(&tenant);
            state.quota_ledger.remove_tenant(&task.tenant_id).await;
            task.current_step = "CREDENTIALS_AND_QUOTA_CLEARED".to_string();
            let _ = state.db.update_erasure_task(&task).await;
        }

        if task.current_step == "CREDENTIALS_AND_QUOTA_CLEARED" {
            let (verified, residuals) =
                verify_multi_store_erasure_integrity(state, &task.tenant_id).await;
            if verified {
                task.verified_integrity = true;
                task.status = "COMPLETED".to_string();
                task.current_step = "VERIFIED_ERASED".to_string();
                task.last_error = None;
                resumed += 1;
                tracing::info!(tenant = %task.tenant_id, task_id = %task.task_id, "Successfully recovered and verified tenant erasure task after restart");
            } else {
                task.verified_integrity = false;
                task.status = "FAILED".to_string();
                task.last_error = Some(format!(
                    "Residual records after resumption: db={}, cache={}, vector={}, exec={}, learning={}",
                    residuals.db, residuals.exact_cache, residuals.vector_store, residuals.execution_store, residuals.learning_store
                ));
            }
            let _ = state.db.update_erasure_task(&task).await;
        }
    }

    Ok(resumed)
}
