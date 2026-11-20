use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sunborn_core::AppConfig;
use sunborn_gateway::{create_router, AppState};
use tower::ServiceExt;

#[tokio::test]
async fn test_multi_relational_knowledge_graph_lifecycle_and_topology() {
    let mut config = AppConfig::default();
    config.server.host = "127.0.0.1".to_string();
    config.server.port = 0;
    config.cache.backend = "memory".to_string();
    config.cache.vector_backend = "memory".to_string();
    config.cache.graph_dependency_cache_enabled = true;

    let state = AppState::new(config);
    let app = create_router(state);

    // 1. Add File Node
    let node_f1 = json!({
        "id": "file::src/payments/upi.rs",
        "label": "upi.rs",
        "node_type": "file",
        "source_file": "src/payments/upi.rs",
        "content_hash": 123456789u64,
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/graph/nodes")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(node_f1.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["status"], "created");
    assert_eq!(body["id"], "file::src/payments/upi.rs");

    // 2. Add Function Node
    let node_fn1 = json!({
        "id": "func::initiate_upi_intent",
        "label": "initiate_upi_intent()",
        "node_type": "function",
        "source_file": "src/payments/upi.rs",
        "source_location": "L45-L90",
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/graph/nodes")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(node_fn1.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 3. Add Table Node
    let node_tbl = json!({
        "id": "table::upi_mandates",
        "label": "upi_mandates",
        "node_type": "table",
        "source_file": "migrations/0002_upi.sql",
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/graph/nodes")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(node_tbl.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. Add CacheEntry Node
    let node_cache = json!({
        "id": "cache::upi_vpa_lookup_9876543210@ybl",
        "label": "VPA Resolution Cache",
        "node_type": "cache_entry",
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/graph/nodes")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(node_cache.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 5. Add edges: Defines, ReadsFrom, DependsOn
    let edge1 = json!({
        "source_id": "file::src/payments/upi.rs",
        "target_id": "func::initiate_upi_intent",
        "relation": "defines",
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/graph/edges")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(edge1.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let edge2 = json!({
        "source_id": "func::initiate_upi_intent",
        "target_id": "table::upi_mandates",
        "relation": "reads_from",
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/graph/edges")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(edge2.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let edge3 = json!({
        "source_id": "cache::upi_vpa_lookup_9876543210@ybl",
        "target_id": "table::upi_mandates",
        "relation": "depends_on",
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/graph/edges")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(edge3.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 6. Inspect Topology
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/graph/topology")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let top: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(top["status"], "healthy");
    assert!(top["total_nodes"].as_u64().unwrap() >= 4);
    assert!(top["total_edges"].as_u64().unwrap() >= 3);

    // 7. Retrieve single node
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/graph/nodes/func::initiate_upi_intent")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let n: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(n["node"]["label"], "initiate_upi_intent()");

    // 8. Explain node
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/graph/explain/func::initiate_upi_intent")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let exp: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(exp["status"], "explained");
}

#[tokio::test]
async fn test_causal_blast_radius_and_real_time_invalidation() {
    let mut config = AppConfig::default();
    config.server.host = "127.0.0.1".to_string();
    config.server.port = 0;
    config.cache.backend = "memory".to_string();
    config.cache.vector_backend = "memory".to_string();
    config.cache.graph_dependency_cache_enabled = true;

    let state = AppState::new(config);
    let app = create_router(state);

    // Build topology:
    // "table::gst_slabs" <-reads- "func::calculate_igst" <-depends- "cache::invoice_summary_sac_998313"
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/graph/nodes")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(
                    json!({
                        "id": "table::gst_slabs",
                        "label": "GST SAC Slabs",
                        "node_type": "table",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/graph/nodes")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(
                    json!({
                        "id": "func::calculate_igst",
                        "label": "calculate_igst()",
                        "node_type": "function",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/graph/nodes")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(
                    json!({
                        "id": "cache::invoice_summary_sac_998313",
                        "label": "Cached SAC Invoice Summary",
                        "node_type": "cache_entry",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/graph/edges")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(
                    json!({
                        "source_id": "func::calculate_igst",
                        "target_id": "table::gst_slabs",
                        "relation": "reads_from",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/graph/edges")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(
                    json!({
                        "source_id": "cache::invoice_summary_sac_998313",
                        "target_id": "func::calculate_igst",
                        "relation": "depends_on",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // 1. Dry-run blast radius simulation
    let sim_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/graph/blast-radius")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(
                    json!({
                        "mutated_roots": ["table::gst_slabs"],
                        "max_depth": 8,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(sim_res.status(), StatusCode::OK);
    let bytes = sim_res.into_body().collect().await.unwrap().to_bytes();
    let report: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(report["mutated_roots"][0], "table::gst_slabs");
    assert_eq!(report["total_affected"], 2);
    let affected_keys = report["affected_cache_keys"].as_array().unwrap();
    assert!(affected_keys
        .iter()
        .any(|k| k == "cache::invoice_summary_sac_998313"));

    // 2. Execute Real-Time Causal Invalidation
    let inv_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/graph/invalidate")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(
                    json!({
                        "mutated_roots": ["table::gst_slabs"],
                        "tenant_id": "enterprise-fintech",
                        "max_depth": 8,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(inv_res.status(), StatusCode::OK);
    let bytes = inv_res.into_body().collect().await.unwrap().to_bytes();
    let inv_body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(inv_body["status"], "purged");
    assert_eq!(inv_body["evicted_cache_count"], 1);
    assert_eq!(inv_body["total_affected_nodes"], 2);
    assert!(inv_body["finops"]["avoided_inr"].as_f64().unwrap() >= 0.0);
}

#[tokio::test]
async fn test_graph_community_detection_and_god_nodes() {
    let mut config = AppConfig::default();
    config.server.host = "127.0.0.1".to_string();
    config.server.port = 0;
    config.cache.backend = "memory".to_string();
    config.cache.vector_backend = "memory".to_string();
    config.cache.graph_dependency_cache_enabled = true;

    let state = AppState::new(config);
    let app = create_router(state);

    // Create a cluster of nodes
    for i in 1..=5 {
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/graph/nodes")
                    .header("content-type", "application/json")
                    .header("x-api-key", "sunborn-admin-key")
                    .body(Body::from(
                        json!({
                            "id": format!("module::cluster_a_node_{i}"),
                            "label": format!("Node A{i}"),
                            "node_type": "module",
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Connect them in a chain
    for i in 1..4 {
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/graph/edges")
                    .header("content-type", "application/json")
                    .header("x-api-key", "sunborn-admin-key")
                    .body(Body::from(
                        json!({
                            "source_id": format!("module::cluster_a_node_{i}"),
                            "target_id": format!("module::cluster_a_node_{}", i + 1),
                            "relation": "calls",
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // 1. Communities endpoint
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/graph/communities")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["status"], "computed");

    // 2. God nodes endpoint
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/graph/god-nodes")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["status"], "computed");

    // 3. Subgraph endpoint
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/graph/subgraph/module::cluster_a_node_1")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["status"], "extracted");
    assert_eq!(body["root_id"], "module::cluster_a_node_1");
}
