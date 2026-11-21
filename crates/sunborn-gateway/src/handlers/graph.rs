//! Handlers for the Graphify Multi-Relational Knowledge Graph & Causal Invalidation Engine.

use super::*;
use sunborn_invalidation::{
    compute_affected, detect_communities, detect_god_nodes, explain_node, extract_subgraph,
    AffectedReport, BlastRadiusConfig, GraphifyConfidence, GraphifyEdge, GraphifyNode,
    GraphifyNodeType, GraphifyRelation, TraversalMode, CLUSTER_INVALIDATION_CHANNEL,
};

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
}

#[derive(Debug, Deserialize)]
pub struct CausalInvalidatePayload {
    pub mutated_roots: Vec<String>,
    #[serde(default)]
    pub tenant_id: Option<String>,
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
        Some(node) => Ok(Json(json!({
            "status": "deleted",
            "id": id,
            "removed_node": node,
            "total_nodes": state.graphify_graph.node_count(),
            "total_edges": state.graphify_graph.edge_count(),
        }))),
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
    let roots: Vec<&str> = payload.mutated_roots.iter().map(|s| s.as_str()).collect();

    let cfg = BlastRadiusConfig {
        max_depth: payload.max_depth.unwrap_or(16),
        allowed_relations: payload
            .allowed_relations
            .map(|rels| rels.into_iter().collect()),
        include_cache_entries: payload.include_cache_entries.unwrap_or(true),
    };

    let report = compute_affected(&state.graphify_graph, &roots, Some(cfg));
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
    let roots: Vec<&str> = payload.mutated_roots.iter().map(|s| s.as_str()).collect();

    let cfg = BlastRadiusConfig {
        max_depth: payload.max_depth.unwrap_or(16),
        allowed_relations: payload
            .allowed_relations
            .map(|rels| rels.into_iter().collect()),
        include_cache_entries: true,
    };

    // 1. Calculate multi-relational causal blast radius
    let report = compute_affected(&state.graphify_graph, &roots, Some(cfg));

    let mut purged_keys = Vec::new();
    let mut failed_keys = Vec::new();

    // 2. Fence root assets: bump epochs and advance watermarks
    for root in &payload.mutated_roots {
        if let Ok(uuid) = Uuid::parse_str(root) {
            let _ = state.epoch_lease_manager.bump_epoch(uuid).await;
        }
        state.watermark_fencer.advance_watermark(root);
    }

    // 3. Purge affected cache keys across L1 exact memory cache and vector store
    for key in &report.affected_cache_keys {
        let mut key_failed = false;

        if let Err(e) = state.cache.delete(key).await {
            tracing::error!(key = %key, error = %e, "L1 cache delete failed during causal invalidation");
            failed_keys.push((key.clone(), format!("exact_cache: {e}")));
            key_failed = true;
        }

        if let Err(e) = state.vector_store.delete(key).await {
            tracing::error!(key = %key, error = %e, "Vector store delete failed during causal invalidation");
            failed_keys.push((key.clone(), format!("vector_store: {e}")));
            key_failed = true;
        }

        if !key_failed {
            purged_keys.push(key.clone());
        }
    }

    // 4. Purge from GraphMemoryCache if initialized
    if let Some(ref gmc) = state.graph_memory_cache {
        for key in &report.affected_cache_keys {
            let _ = gmc.cache().delete(key).await;
        }
    }

    // 5. Durable outbox audit recording
    let tenant_str = payload
        .tenant_id
        .clone()
        .unwrap_or_else(|| "system".to_string());
    let outbox_record = sunborn_db::models::OutboxRecord {
        id: 0,
        tenant_id: tenant_str.clone(),
        entity_type: "graph_causal_invalidation".to_string(),
        entity_id: payload.mutated_roots.join(","),
        action: "causal_invalidate".to_string(),
        payload: json!({
            "mutated_roots": payload.mutated_roots,
            "affected_count": report.total_affected,
            "purged_cache_keys": purged_keys,
            "failed_keys": failed_keys,
        }),
        status: "PROCESSED".to_string(),
        created_at: Utc::now(),
    };
    let _ = state.db.insert_outbox_event(outbox_record).await;

    // 6. Cluster invalidation broadcast
    if let Some(transport) = state.orchestrator.cluster_transport() {
        let msg = sunborn_invalidation::ClusterInvalidationMessage {
            message_id: Uuid::new_v4(),
            origin_pod_id: "sunborn-gateway".to_string(),
            tenant_id: TenantId::new(tenant_str.clone()),
            mutated_doc_hashes: Vec::new(),
            mutated_source_ids: Vec::new(),
            mutated_cache_keys: purged_keys.clone(),
            monotonic_epoch: 1,
            timestamp: Utc::now(),
            stream_id: String::new(),
        };
        let _ = transport.publish(CLUSTER_INVALIDATION_CHANNEL, &msg).await;
    }

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
