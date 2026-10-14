use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use sunborn_core::AppConfig;
use sunborn_execution::{ExecutionNode, ExecutionPlan, ToolCall};
use sunborn_gateway::{create_router, AppState};
use tower::ServiceExt;

#[tokio::test]
async fn test_execution_dag_reuse_and_invalidation() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let tenant = "tenant-agent-1";
    let application = "agent-finops";

    // 1. Record a cached tool result for "fetch_stock_price"
    let tool_args = serde_json::json!({ "ticker": "MSFT", "currency": "USD" });
    let record_payload = serde_json::json!({
        "tenant_id": tenant,
        "application_id": application,
        "tool_name": "fetch_stock_price",
        "tool_version": "1.0.0",
        "arguments": tool_args,
        "context_hash": null,
        "output": { "price": 425.50, "timestamp": "2026-10-14T10:00:00Z" },
        "is_error": false,
        "execution_latency_ms": 180,
        "ttl_secs": 3600
    });

    let record_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/execution/results")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&record_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(record_res.status(), StatusCode::OK);
    let record_body = record_res.into_body().collect().await.unwrap().to_bytes();
    let record_json: Value = serde_json::from_slice(&record_body).unwrap();
    assert_eq!(record_json["status"], "stored");
    let signature = record_json["signature"].as_str().unwrap().to_string();
    assert!(!signature.is_empty());

    // 2. Submit an Execution DAG Plan:
    // Node 1: "fetch_stock_price" with identical args (re-ordered keys to test canonicalization)
    // Node 2: "summarize_portfolio" depending on Node 1 (non-deterministic synthesis)
    let reordered_args = serde_json::json!({ "currency": "USD", "ticker": "MSFT" });
    let mut plan = ExecutionPlan::new();
    plan.add_node(ExecutionNode {
        id: "step_fetch".into(),
        name: "Fetch MSFT Price".into(),
        dependencies: vec![],
        tool_call: Some(ToolCall {
            tool_name: "fetch_stock_price".into(),
            tool_version: "1.0.0".into(),
            arguments: reordered_args,
            context_hash: None,
        }),
        is_deterministic: true,
    });
    plan.add_node(ExecutionNode {
        id: "step_synthesize".into(),
        name: "Synthesize Report".into(),
        dependencies: vec!["step_fetch".into()],
        tool_call: Some(ToolCall {
            tool_name: "llm_synthesize".into(),
            tool_version: "1.0.0".into(),
            arguments: serde_json::json!({ "format": "markdown" }),
            context_hash: None,
        }),
        is_deterministic: false,
    });

    let plan_payload = serde_json::json!({
        "tenant_id": tenant,
        "application_id": application,
        "plan": plan
    });

    let plan_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/execution/plan")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&plan_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(plan_res.status(), StatusCode::OK);
    let plan_body = plan_res.into_body().collect().await.unwrap().to_bytes();
    let plan_eval: Value = serde_json::from_slice(&plan_body).unwrap();

    assert_eq!(plan_eval["total_nodes"], 2);
    assert_eq!(plan_eval["cached_nodes"], 1);
    assert_eq!(plan_eval["estimated_latency_saved_ms"], 180);
    assert_eq!(plan_eval["reused_node_ids"][0], "step_fetch");
    assert_eq!(plan_eval["pending_node_ids"][0], "step_synthesize");

    // 3. Evict the tool execution cache
    let evict_payload = serde_json::json!({
        "tenant_id": tenant,
        "tool_name": "fetch_stock_price"
    });

    let evict_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/execution/evict")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&evict_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(evict_res.status(), StatusCode::OK);
    let evict_body = evict_res.into_body().collect().await.unwrap().to_bytes();
    let evict_json: Value = serde_json::from_slice(&evict_body).unwrap();
    assert_eq!(evict_json["evicted_count"], 1);

    // 4. Re-plan: after eviction, Node 1 must be pending execution
    let recheck_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/execution/plan")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&plan_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(recheck_res.status(), StatusCode::OK);
    let recheck_body = recheck_res.into_body().collect().await.unwrap().to_bytes();
    let recheck_eval: Value = serde_json::from_slice(&recheck_body).unwrap();

    assert_eq!(recheck_eval["cached_nodes"], 0);
    assert_eq!(
        recheck_eval["pending_node_ids"].as_array().unwrap().len(),
        2
    );
}
