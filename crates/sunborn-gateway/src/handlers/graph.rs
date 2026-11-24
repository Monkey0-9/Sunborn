//! Handlers for the Graphify Multi-Relational Knowledge Graph & Causal Invalidation Engine.

use super::*;
use sunborn_invalidation::{
    compute_affected, detect_communities, detect_god_nodes, explain_node, extract_subgraph,
    AffectedReport, BlastRadiusConfig, GraphifyConfidence, GraphifyEdge, GraphifyNode,
    GraphifyNodeType, GraphifyRelation, TraversalMode, CLUSTER_INVALIDATION_CHANNEL,
};

/// Persists the Graphify knowledge graph asynchronously to disk if `SUNBORN_GRAPHIFY_STORE_PATH` is configured.
pub fn persist_graphify_if_configured(
    graph: std::sync::Arc<sunborn_invalidation::GraphifyKnowledgeGraph>,
) {
    if let Ok(path) = std::env::var("SUNBORN_GRAPHIFY_STORE_PATH") {
        tokio::task::spawn_blocking(move || {
            if let Err(e) = graph.save_to_json(&path) {
                tracing::warn!(error = %e, path = %path, "Failed to persist Graphify knowledge graph to disk");
            }
        });
    }
}

#[derive(Debug, Deserialize)]
pub struct AddNodePayload {
    pub id: String,
    pub label: String,
    pub node_type: GraphifyNodeType,
    #[serde(default)]
    pub source_file: Option<String>,
    #[serde(default)]
    pub source_location: Option<String>,
    #[serde(default)]
    pub content_hash: Option<u64>,
    #[serde(default)]
    pub community_id: Option<u32>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct AddEdgePayload {
    pub source_id: String,
    pub target_id: String,
    pub relation: GraphifyRelation,
    #[serde(default)]
    pub confidence: Option<GraphifyConfidence>,
    #[serde(default)]
    pub weight: Option<f32>,
    #[serde(default)]
    pub source_file: Option<String>,
    #[serde(default)]
    pub source_location: Option<String>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct BlastRadiusPayload {
    pub mutated_roots: Vec<String>,
    #[serde(default)]
    pub max_depth: Option<usize>,
    #[serde(default)]
    pub allowed_relations: Option<Vec<GraphifyRelation>>,
    #[serde(default)]
    pub include_cache_entries: Option<bool>,
    #[serde(default)]
    pub tenant_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CausalInvalidatePayload {
    pub mutated_roots: Vec<String>,
    /// Owning tenant. Required: an unscoped invalidation cannot be proven safe.
    pub tenant_id: String,
    #[serde(default)]
    pub max_depth: Option<usize>,
    #[serde(default)]
    pub allowed_relations: Option<Vec<GraphifyRelation>>,
}

/// POST /v1/graph/nodes
pub async fn add_graph_node(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<AddNodePayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    let numeric_id = state.graphify_graph.next_numeric_id();
    let mut node = GraphifyNode::new(
        payload.id.clone(),
        numeric_id,
        payload.label.clone(),
        payload.node_type,
    );

    if let Some(file) = payload.source_file {
        node = node.with_source(file, payload.source_location);
    }
    if let Some(hash) = payload.content_hash {
        node = node.with_hash(hash);
    }
    if let Some(comm) = payload.community_id {
        node = node.with_community(comm);
    }
    if let Some(meta) = payload.metadata {
        node = node.with_metadata(meta);
    }

    let allocated_numeric_id = state.graphify_graph.add_node(node);
    persist_graphify_if_configured(state.graphify_graph.clone());

    Ok(Json(json!({
        "status": "created",
        "id": payload.id,
        "numeric_id": allocated_numeric_id,
        "node_type": payload.node_type.as_str(),
        "total_nodes": state.graphify_graph.node_count(),
    })))
}

/// GET /v1/graph/nodes/{*id}
pub async fn get_graph_node(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    match state.graphify_graph.get_node(&id) {
        Some(node) => Ok(Json(json!({
            "status": "found",
            "node": node,
        }))),
        None => Err(SunbornError::new(
            sunborn_core::ErrorCode::CacheMiss,
            format!("Node with id '{id}' not found in knowledge graph"),
        )
        .into()),
    }
}

/// DELETE /v1/graph/nodes/{*id}
pub async fn delete_graph_node(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    match state.graphify_graph.remove_node(&id) {
        Some(node) => {
            persist_graphify_if_configured(state.graphify_graph.clone());
            Ok(Json(json!({
                "status": "deleted",
                "id": id,
                "removed_node": node,
                "total_nodes": state.graphify_graph.node_count(),
                "total_edges": state.graphify_graph.edge_count(),
            })))
        }
        None => Err(SunbornError::new(
            sunborn_core::ErrorCode::CacheMiss,
            format!("Node with id '{id}' not found"),
        )
        .into()),
    }
}

/// POST /v1/graph/edges
pub async fn add_graph_edge(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<AddEdgePayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    let mut edge = GraphifyEdge::new(
        payload.source_id.clone(),
        payload.target_id.clone(),
        payload.relation,
    );

    if let Some(conf) = payload.confidence {
        edge = edge.with_confidence(conf);
    }
    if let Some(w) = payload.weight {
        edge = edge.with_weight(w);
    }
    if let Some(loc) = payload.source_location.or(payload.source_file) {
        edge = edge.with_location(loc);
    }

    state.graphify_graph.add_edge(edge);
    persist_graphify_if_configured(state.graphify_graph.clone());

    Ok(Json(json!({
        "status": "linked",
        "source_id": payload.source_id,
        "target_id": payload.target_id,
        "relation": payload.relation.as_str(),
        "total_edges": state.graphify_graph.edge_count(),
    })))
}

/// POST /v1/graph/blast-radius (Read-only simulation)
pub async fn calculate_graph_blast_radius(
    State(state): State<AppState>,
    Json(payload): Json<BlastRadiusPayload>,
) -> Result<Json<AffectedReport>, AppError> {
    let graph = state.graphify_graph.clone();
    let roots_owned: Vec<String> = payload.mutated_roots.clone();

    let cfg = BlastRadiusConfig {
        max_depth: payload.max_depth.unwrap_or(16),
        allowed_relations: payload
            .allowed_relations
            .map(|rels| rels.into_iter().collect()),
        include_cache_entries: payload.include_cache_entries.unwrap_or(true),
        tenant_id: payload.tenant_id,
    };

    let report = tokio::task::spawn_blocking(move || {
        let roots_ref: Vec<&str> = roots_owned.iter().map(|s| s.as_str()).collect();
        compute_affected(&graph, &roots_ref, Some(cfg))
    })
    .await
    .map_err(|e| SunbornError::internal(format!("Graph blast radius task failed: {e}")))?;

    Ok(Json(report))
}

/// POST /v1/graph/invalidate (Real-Time Causal Invalidation)
pub async fn invalidate_graph_causal(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<CausalInvalidatePayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    let start = Instant::now();
    let tenant_str = payload.tenant_id.trim().to_string();
    if tenant_str.is_empty() {
        return Err(SunbornError::forbidden(
            "tenant_id is required for causal invalidation; an unscoped purge is refused",
        )
        .into());
    }

    let graph = state.graphify_graph.clone();
    let roots_owned: Vec<String> = payload.mutated_roots.clone();

    let cfg = BlastRadiusConfig {
        max_depth: payload.max_depth.unwrap_or(16),
        allowed_relations: payload
            .allowed_relations
            .map(|rels| rels.into_iter().collect()),
        include_cache_entries: true,
        tenant_id: Some(tenant_str.clone()),
    };

    // 1. Calculate multi-relational causal blast radius off the Tokio worker thread
    let report = tokio::task::spawn_blocking(move || {
        let roots_ref: Vec<&str> = roots_owned.iter().map(|s| s.as_str()).collect();
        compute_affected(&graph, &roots_ref, Some(cfg))
    })
    .await
    .map_err(|e| SunbornError::internal(format!("Graph causal invalidation task failed: {e}")))?;

    // Fail closed before touching any durable state: a root we cannot attribute
    // to the tenant must not be silently skipped, because the caller would read
    // a success while stale entries survive.
    if !report.rejected_roots.is_empty() {
        return Err(SunbornError::forbidden(format!(
            "refusing causal invalidation: {} root(s) are not owned by tenant '{}': {}",
            report.rejected_roots.len(),
            tenant_str,
            report.rejected_roots.join(", ")
        ))
        .into());
    }

    let mut purged_keys = Vec::new();
    let mut failed_keys = Vec::new();

    // 2. Fence root assets: bump epochs and advance watermarks (fail-closed on lease/epoch failure)
    let mut highest_epoch = 1u64;
    let mut valid_source_uuids = Vec::new();
    for root in &payload.mutated_roots {
        if let Ok(uuid) = Uuid::parse_str(root) {
            valid_source_uuids.push(uuid);
            let bumped = state
                .epoch_lease_manager
                .bump_epoch(uuid)
                .await
                .map_err(AppError::from)?;
            if bumped > highest_epoch {
                highest_epoch = bumped;
            }
        }
        state.watermark_fencer.advance_watermark(root);
    }

    // 3. Durable-first ordering: persist PENDING outbox intent before modifying storage tiers
    let outbox_record = sunborn_db::models::OutboxRecord {
        id: 0,
        tenant_id: tenant_str.clone(),
        entity_type: "graph_causal_invalidation".to_string(),
        entity_id: payload.mutated_roots.join(","),
        action: "causal_invalidate".to_string(),
        payload: json!({
            "mutated_roots": payload.mutated_roots,
            "affected_count": report.total_affected,
            "affected_cache_keys": report.affected_cache_keys,
        }),
        status: "PENDING".to_string(),
        created_at: Utc::now(),
    };
    let outbox_id = state
        .db
        .insert_outbox_event(outbox_record)
        .await
        .map_err(AppError::from)?;

    // 4. Purge affected cache keys across exact cache and vector store
    for raw_key in &report.affected_cache_keys {
        let key = raw_key.strip_prefix("cache::").unwrap_or(raw_key.as_str());
        let mut key_failed = false;

        if let Err(e) = state.cache.delete(key).await {
            tracing::error!(key = %key, error = %e, "Cache delete failed during causal invalidation");
            failed_keys.push((key.to_string(), format!("exact_cache: {e}")));
            key_failed = true;
        }

        if let Err(e) = state.vector_store.delete(key).await {
            tracing::error!(key = %key, error = %e, "Vector store delete failed during causal invalidation");
            failed_keys.push((key.to_string(), format!("vector_store: {e}")));
            key_failed = true;
        }

        if !key_failed {
            purged_keys.push(key.to_string());
        }
    }

    // 5. Purge from GraphMemoryCache if initialized. A failure here is a failed
    //    purge and must be reported, not swallowed.
    if let Some(ref gmc) = state.graph_memory_cache {
        for raw_key in &report.affected_cache_keys {
            let key = raw_key.strip_prefix("cache::").unwrap_or(raw_key.as_str());
            if let Err(e) = gmc.cache().delete(key).await {
                tracing::error!(key = %key, error = %e, "Graph memory cache delete failed during causal invalidation");
                failed_keys.push((key.to_string(), format!("graph_memory: {e}")));
            }
        }
    }

    // 6. Cluster invalidation broadcast with real epoch and source IDs
    let mut cluster_broadcast_sent = false;
    if let Some(transport) = state.orchestrator.cluster_transport() {
        let msg = sunborn_invalidation::ClusterInvalidationMessage {
            message_id: Uuid::new_v4(),
            origin_pod_id: "sunborn-gateway".to_string(),
            tenant_id: TenantId::new(tenant_str.clone()),
            mutated_doc_hashes: Vec::new(),
            mutated_source_ids: valid_source_uuids,
            mutated_cache_keys: purged_keys.clone(),
            monotonic_epoch: highest_epoch,
            timestamp: Utc::now(),
            stream_id: String::new(),
        };
        match transport.publish(CLUSTER_INVALIDATION_CHANNEL, &msg).await {
            Ok(_) => {
                cluster_broadcast_sent = true;
            }
            Err(err) => {
                tracing::error!(error = %err, "Cluster invalidation broadcast failed");
                if let Err(mark_err) = state
                    .db
                    .mark_outbox_failed(outbox_id, &err.to_string())
                    .await
                {
                    tracing::error!(
                        outbox_id,
                        error = %mark_err,
                        "Outbox event could not be marked FAILED; it stays PENDING for worker retry"
                    );
                }
                // Fail closed on publish failure across all modes: outbox remains FAILED/PENDING for DLQ retry
                return Err(AppError::from(err));
            }
        }
    }

    // 7. Acknowledge outbox status after purges and broadcast succeed
    if failed_keys.is_empty() {
        state
            .db
            .mark_outbox_processed(outbox_id)
            .await
            .map_err(AppError::from)?;
    } else {
        let err_msg = format!(
            "Partial failure: {} cache keys failed eviction during causal invalidation",
            failed_keys.len()
        );
        if let Err(mark_err) = state.db.mark_outbox_failed(outbox_id, &err_msg).await {
            tracing::error!(
                outbox_id,
                error = %mark_err,
                "Outbox event could not be marked FAILED after partial purge; it stays PENDING and will be retried"
            );
            return Err(SunbornError::database(format!(
                "{err_msg}; additionally failed to transition outbox state: {mark_err}"
            ))
            .into());
        }
        // Fail closed in ALL execution modes: partial purge is NEVER reported as success
        return Err(SunbornError::cache(err_msg).into());
    }

    // 8. Snapshot Graphify graph if durable path configured
    persist_graphify_if_configured(state.graphify_graph.clone());

    let elapsed_ms = start.elapsed().as_millis() as u64;

    // 7. Calculate Sovereign Indian FinOps metrics (avoided compute)
    let avoided_tokens = (purged_keys.len() * 1500) as u32; // ~1500 tokens per cached inference
    let sarvam_pricing = ModelPricing::sarvam_2b();
    let avoided_usd =
        FinOpsEngine::calculate_token_cost_avoided(&sarvam_pricing, avoided_tokens, 0);
    let avoided_micro_paise = MicroPaise::from_usd(avoided_usd, MicroPaise::DEFAULT_INR_PER_USD)
        .unwrap_or(MicroPaise::ZERO);

    Ok(Json(json!({
        "status": if failed_keys.is_empty() { "purged" } else { "partially_purged" },
        "outbox_id": outbox_id,
        "bumped_epoch": highest_epoch,
        "cluster_broadcast_sent": cluster_broadcast_sent,
        "mutated_roots": payload.mutated_roots,
        "max_depth_reached": report.max_depth_reached,
        "total_affected_nodes": report.total_affected,
        "affected_nodes": report.affected_nodes,
        "affected_cache_keys": report.affected_cache_keys,
        "purged_keys": purged_keys,
        "evicted_cache_count": purged_keys.len(),
        "failed_keys": failed_keys,
        "latency_ms": elapsed_ms,
        "finops": {
            "avoided_tokens_estimate": avoided_tokens,
            "avoided_usd": avoided_usd,
            "avoided_inr": avoided_micro_paise.to_inr(),
            "avoided_inr_formatted": avoided_micro_paise.format_indian_currency(),
            "avoided_micro_paise": avoided_micro_paise.0,
        },
        "timestamp": Utc::now().to_rfc3339(),
    })))
}

/// GET /v1/graph/explain/{*id}
pub async fn explain_graph_node(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    match explain_node(&state.graphify_graph, &id) {
        Some(explanation) => Ok(Json(json!({
            "status": "explained",
            "explanation": explanation,
        }))),
        None => Err(SunbornError::new(
            sunborn_core::ErrorCode::CacheMiss,
            format!("Node with id '{id}' not found in knowledge graph"),
        )
        .into()),
    }
}

/// GET /v1/graph/communities
pub async fn get_graph_communities(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let report = detect_communities(&state.graphify_graph, 15);
    Ok(Json(json!({
        "status": "computed",
        "communities": report,
    })))
}

/// GET /v1/graph/god-nodes
pub async fn get_graph_god_nodes(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let report = detect_god_nodes(&state.graphify_graph, 15);
    Ok(Json(json!({
        "status": "computed",
        "god_nodes": report,
    })))
}

/// GET /v1/graph/subgraph/{*id}
pub async fn get_graph_subgraph(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let sub = extract_subgraph(&state.graphify_graph, &id, 3, TraversalMode::Bfs);
    Ok(Json(json!({
        "status": "extracted",
        "root_id": id,
        "subgraph": sub,
    })))
}

/// GET /v1/graph/topology
pub async fn get_graph_topology(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let total_nodes = state.graphify_graph.node_count();
    let total_edges = state.graphify_graph.edge_count();

    let all_nodes = state.graphify_graph.all_nodes();
    let mut type_counts = std::collections::HashMap::new();

    for n in &all_nodes {
        *type_counts.entry(n.node_type.as_str()).or_insert(0usize) += 1;
    }

    Ok(Json(json!({
        "status": "healthy",
        "engine": "graphify-multi-relational-v2",
        "total_nodes": total_nodes,
        "total_edges": total_edges,
        "node_type_counts": type_counts,
        "sample_nodes": all_nodes.into_iter().take(20).collect::<Vec<_>>(),
    })))
}
