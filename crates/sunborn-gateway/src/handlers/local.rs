//! Handlers grouped by domain; see `super` for shared imports.

use super::*;

#[derive(Deserialize)]
pub struct SaveLocalProviderPayload {
    pub provider: String,
    pub api_key: String,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
}

pub async fn get_local_cockpit_html() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("../templates/local_cockpit.html"))
}

pub async fn get_local_status(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let vault = sunborn_identity::LocalCredentialVault::new_default();
    let providers = vault.list_providers();
    let is_prod = sunborn_core::is_production_mode();

    Ok(Json(json!({
        "status": "ready",
        "mode": if is_prod { "production_cluster" } else { "local_byok" },
        "storage_tier": {
            "execution": state.execution_store.storage_tier(),
            "learning": state.learning_store.storage_tier(),
            "exact_cache": if state.is_dragonfly_cache_active() { "dragonfly_l2" } else { "memory_l1" },
            "vector_store": if state.is_qdrant_vector_active() { "qdrant_dense" } else { "memory_l1" },
        },
        "configured_providers": providers,
        "localhost_only": !is_prod,
    })))
}

pub async fn save_local_provider(
    State(_state): State<AppState>,
    Json(payload): Json<SaveLocalProviderPayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    let vault = sunborn_identity::LocalCredentialVault::new_default();
    vault
        .store_provider(
            &payload.provider,
            &payload.api_key,
            payload.base_url.clone(),
            payload.default_model.clone(),
        )
        .map_err(AppError::from)?;

    let preview = sunborn_identity::LocalCredentialVault::mask_key(&payload.api_key);

    Ok(Json(json!({
        "status": "saved",
        "provider": payload.provider.to_lowercase(),
        "key_preview": preview,
        "base_url": payload.base_url,
        "default_model": payload.default_model,
    })))
}

pub async fn delete_local_provider(
    Path(provider_name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let vault = sunborn_identity::LocalCredentialVault::new_default();
    let removed = vault
        .remove_provider(&provider_name)
        .map_err(AppError::from)?;

    Ok(Json(json!({
        "status": if removed { "removed" } else { "not_found" },
        "provider": provider_name,
    })))
}

#[derive(Deserialize)]
pub struct ConfigureLocalSettingsPayload {
    pub default_spend_ceiling_usd: Option<f64>,
    pub exact_cache_enabled: Option<bool>,
    pub semantic_cache_enabled: Option<bool>,
}

pub async fn configure_local_settings(
    State(state): State<AppState>,
    Json(payload): Json<ConfigureLocalSettingsPayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    if let Some(ceiling) = payload.default_spend_ceiling_usd {
        if ceiling >= 0.0 && ceiling.is_finite() {
            state
                .spend_velocity_controller
                .set_tenant_ceiling("local-user", ceiling);
        }
    }

    Ok(Json(json!({
        "status": "updated",
        "settings": {
            "default_spend_ceiling_usd": payload.default_spend_ceiling_usd,
            "exact_cache_enabled": payload.exact_cache_enabled.unwrap_or(true),
            "semantic_cache_enabled": payload.semantic_cache_enabled.unwrap_or(true),
        }
    })))
}
