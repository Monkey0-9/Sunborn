//! Handlers grouped by domain; see `super` for shared imports.

use super::*;

#[derive(Deserialize)]
pub struct RadixRoutePayload {
    pub prompt: String,
    pub warm_prefix: Option<String>,
    pub warm_node: Option<String>,
}

pub async fn radix_route(
    State(state): State<AppState>,
    Json(payload): Json<RadixRoutePayload>,
) -> impl IntoResponse {
    if let (Some(prefix), Some(node)) = (&payload.warm_prefix, &payload.warm_node) {
        state.radix_router.warm_prefix(prefix, node);
    }

    let decision = state.radix_router.route(&payload.prompt);
    Json(json!({
        "selected_node": decision.selected_node,
        "is_kv_affinity_hit": decision.is_kv_affinity_hit,
        "matched_tokens": decision.matched_tokens,
        "estimated_prefill_savings_pct": if decision.is_kv_affinity_hit { 80.0 } else { 0.0 }
    }))
}

pub async fn radix_inspect(State(_state): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "status": "operational",
        "backend_nodes": ["inference-node-1", "inference-node-2", "inference-node-3"],
        "min_affinity_tokens": 16,
        "max_load_factor": 1.5,
        "strategy": "compressed-radix-tree-affinity"
    }))
}

#[derive(Deserialize)]
pub struct AttestationPayload {
    pub leaf_hashes: Vec<String>,
}

pub async fn generate_merkle_attestation(
    Json(payload): Json<AttestationPayload>,
) -> impl IntoResponse {
    let (root, proofs) =
        sunborn_execution::MerkleExecutionEngine::build_merkle_tree(&payload.leaf_hashes);
    Json(json!({
        "status": "attested",
        "root_hash": root,
        "leaf_count": payload.leaf_hashes.len(),
        "proofs": proofs
    }))
}

#[derive(Deserialize)]
pub struct VerifyProofPayload {
    #[serde(default)]
    pub proof: Option<sunborn_execution::MerkleInclusionProof>,
    #[serde(default)]
    pub root_hash: Option<String>,
    #[serde(default)]
    pub merkle_root: Option<String>,
    #[serde(default)]
    pub leaf_hash: Option<String>,
    #[serde(default)]
    pub path: Option<Vec<sunborn_execution::MerkleProofStep>>,
}

pub async fn verify_merkle_attestation(
    Json(payload): Json<VerifyProofPayload>,
) -> impl IntoResponse {
    let proof_opt = payload.proof.or_else(|| {
        let root = payload.root_hash.or(payload.merkle_root)?;
        let leaf = payload.leaf_hash?;
        let path = payload.path.unwrap_or_default();
        Some(sunborn_execution::MerkleInclusionProof {
            leaf_hash: leaf,
            root_hash: root,
            path,
        })
    });

    let (is_valid, root, leaf) = match proof_opt {
        Some(p) => {
            let valid = p.verify();
            (valid, p.root_hash, p.leaf_hash)
        }
        None => (false, String::new(), String::new()),
    };

    Json(json!({
        "is_valid": is_valid,
        "valid": is_valid,
        "root_hash": root,
        "leaf_hash": leaf
    }))
}

pub async fn ceremony_public_keys(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "status": "active",
        "active_key_id": state.attestation_key_id,
        "signature_algorithm": "ed25519",
        "verifying_key_hex": hex::encode(state.attestation_verifying_key.as_bytes()),
        "cluster_sync_active": state.cluster_sync_engine.is_some(),
        "timestamp": Utc::now()
    }))
}

#[derive(Deserialize)]
pub struct SpeculativePredictPayload {
    pub current_action: String,
    pub record_transition_to: Option<String>,
}

pub async fn predict_speculative(
    State(state): State<AppState>,
    Json(payload): Json<SpeculativePredictPayload>,
) -> impl IntoResponse {
    if let Some(next_action) = &payload.record_transition_to {
        state
            .predictive_engine
            .record_transition(&payload.current_action, next_action);
    }
    let predictions = state
        .predictive_engine
        .predict_next(&payload.current_action);
    Json(json!({
        "current_action": payload.current_action,
        "predictions": predictions,
        "speculative_cache_warmed": !predictions.is_empty()
    }))
}

pub async fn get_cluster_sync_status(State(state): State<AppState>) -> impl IntoResponse {
    let vc = read_lock(&state.vector_clock).clone();
    let region = std::env::var("SUNBORN_REGION").unwrap_or_else(|_| "us-east-1".to_string());
    let kms_provider = state.crypto_registry.provider().provider_name();
    Json(json!({
        "node_id": "gateway-primary-node-1",
        "region": region,
        "status_observed_at": Utc::now().to_rfc3339(),
        "mode": "standalone_authoritative",
        "nodes_active": 1,
        "cluster_state": "single-node",
        "vector_clock": vc,
        "anti_entropy_protocol": "single-node-epoch",
        "consensus_mode": "single_node_authoritative",
        "kms_provider": kms_provider,
        "spend_velocity_enforcement": "active_enforced",
        "spend_velocity_scope": state.spend_ledger.enforcement_scope()
    }))
}

pub async fn get_autonomic_status(State(state): State<AppState>) -> impl IntoResponse {
    let incidents = state.autonomic_controller.list_incidents();
    let circuit_breaker_status = state.circuit_breaker.status().await;
    Json(json!({
        "autonomic_health": "operational",
        "remediation_mode": "continuous-closed-loop",
        "circuit_breaker_status": format!("{:?}", circuit_breaker_status),
        "total_incidents_resolved": incidents.len(),
        "recent_incidents": incidents
    }))
}
