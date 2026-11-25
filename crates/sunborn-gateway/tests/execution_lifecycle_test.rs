//! Integration & fault-injection tests for G4 Execution-Effect Safety:
//! - Unified `EnforcedExecutionRuntime` (`sunborn-effects::SubgraphPlanner` +
//!   `sunborn-execution::ExecutionPlanner` + live `ToolExecutor`)
//! - All 5 effect categories (`PureDeterministic`, `ReadOnlyIdempotent`,
//!   `TimeSensitiveNonDeterministic`, idempotent mutation deduplication,
//!   irreversible `NonIdempotentMutation`)
//! - Upstream taint propagation never skipping downstream mutating nodes
//! - Failure injection when tool executes externally but cache recording fails
//! - Live gateway `/v1/requests` and `/v1/execution/plan` G4 enforcement

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};
use sunborn_core::{
    AppConfig, ApplicationId, RequestEnvelope, ResponseEnvelope, SunbornError, TenantId,
};
use sunborn_execution::{
    ExecutionCacheStore, ExecutionEffect, ExecutionNode, ExecutionPlan, MemoryExecutionCacheStore,
    NodeExecutionStatus, PlanEvaluation, ToolCall, ToolResult,
};
use sunborn_gateway::{
    create_router,
    execution_runtime::{EnforcedExecutionRuntime, ToolExecutor},
    AppState,
};
use tower::ServiceExt;

struct CountingToolExecutor {
    calls: Mutex<HashMap<String, usize>>,
    total_calls: AtomicUsize,
}

impl CountingToolExecutor {
    fn new() -> Self {
        Self {
            calls: Mutex::new(HashMap::new()),
            total_calls: AtomicUsize::new(0),
        }
    }

    fn count_for(&self, node_id: &str) -> usize {
        *self.calls.lock().unwrap().get(node_id).unwrap_or(&0)
    }

    fn total(&self) -> usize {
        self.total_calls.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl ToolExecutor for CountingToolExecutor {
    async fn execute_tool(
        &self,
        node_id: &str,
        _effect: &ExecutionEffect,
        call: &ToolCall,
    ) -> Result<Value, SunbornError> {
        let count = {
            let mut guard = self.calls.lock().unwrap();
            let entry = guard.entry(node_id.to_string()).or_insert(0);
            *entry += 1;
            *entry
        };
        self.total_calls.fetch_add(1, Ordering::Relaxed);
        Ok(json!({
            "node_id": node_id,
            "tool": call.tool_name,
            "exec_seq": count,
        }))
    }
}

struct FailingWriteStore {
    inner: MemoryExecutionCacheStore,
    fail_on_set: bool,
}

#[async_trait]
impl ExecutionCacheStore for FailingWriteStore {
    async fn get(
        &self,
        tenant_id: &TenantId,
        signature: &str,
    ) -> Result<Option<ToolResult>, SunbornError> {
        self.inner.get(tenant_id, signature).await
    }

    async fn set(
        &self,
        tenant_id: &TenantId,
        signature: &str,
        result: &ToolResult,
        ttl_secs: Option<u64>,
    ) -> Result<(), SunbornError> {
        if self.fail_on_set {
            return Err(SunbornError::cache_unavailable(
                "Injected ExecutionCacheStore::set failure after external tool execution",
            ));
        }
        self.inner.set(tenant_id, signature, result, ttl_secs).await
    }

    async fn evict(&self, tenant_id: &TenantId, signature: &str) -> Result<bool, SunbornError> {
        self.inner.evict(tenant_id, signature).await
    }

    async fn evict_by_tool(
        &self,
        tenant_id: &TenantId,
        tool_name: &str,
    ) -> Result<usize, SunbornError> {
        self.inner.evict_by_tool(tenant_id, tool_name).await
    }

    async fn evict_by_tenant(&self, tenant_id: &TenantId) -> Result<usize, SunbornError> {
        self.inner.evict_by_tenant(tenant_id).await
    }

    async fn count_by_tenant(&self, tenant_id: &TenantId) -> Result<usize, SunbornError> {
        self.inner.count_by_tenant(tenant_id).await
    }
}

fn tc(name: &str, arg: &str) -> Option<ToolCall> {
    Some(ToolCall {
        tool_name: name.to_string(),
        tool_version: "1.0".to_string(),
        arguments: json!({ "input": arg }),
        context_hash: Some("ctx-v1".to_string()),
    })
}

#[tokio::test]
async fn test_enforced_runtime_covers_all_effect_classes_and_mutation_safety() {
    let runtime = EnforcedExecutionRuntime::new();
    let store = MemoryExecutionCacheStore::new();
    let executor = CountingToolExecutor::new();
    let tenant = TenantId::new("tenant-g4");
    let app = ApplicationId::new("app-agent");

    let mut plan = ExecutionPlan::new();
    plan.add_node(ExecutionNode::new(
        "pure_calc",
        "Pure Math",
        vec![],
        tc("math", "42"),
        true,
    ));
    plan.add_node(
        ExecutionNode::new(
            "read_ledger",
            "Versioned Read",
            vec![],
            tc("ledger_read", "acct-1"),
            true,
        )
        .with_effect(ExecutionEffect::ReadOnlyIdempotent {
            required_state_version: 2,
            source_ids: vec![],
        }),
    );
    plan.add_node(
        ExecutionNode::new(
            "quote_fx",
            "FX Quote",
            vec![],
            tc("fx_quote", "USDINR"),
            false,
        )
        .with_effect(ExecutionEffect::TimeSensitiveNonDeterministic { max_ttl_secs: 60 }),
    );
    plan.add_node(
        ExecutionNode::new(
            "idem_transfer",
            "Idempotent Ledger Transfer",
            vec!["pure_calc".to_string()],
            tc("transfer", "500"),
            false,
        )
        .with_effect(ExecutionEffect::MutatingSideEffecting {
            operation_type: "idempotent".to_string(),
            idempotency_key: "transfer-key-001".to_string(),
        }),
    );
    plan.add_node(
        ExecutionNode::new(
            "irrev_webhook",
            "Non-Idempotent Webhook",
            vec!["read_ledger".to_string()],
            tc("send_webhook", "notify"),
            false,
        )
        .with_effect(ExecutionEffect::MutatingSideEffecting {
            operation_type: "external_action".to_string(),
            idempotency_key: "".to_string(),
        }),
    );

    // Run 1: all 5 nodes execute live
    let rep1 = runtime
        .execute_workflow(&tenant, &app, &plan, &store, &executor)
        .await
        .unwrap();
    assert_eq!(rep1.total_nodes, 5);
    assert_eq!(rep1.executed_nodes, 5);
    assert_eq!(rep1.reused_nodes, 0);
    assert_eq!(rep1.recording_failures, 0);
    assert_eq!(executor.count_for("idem_transfer"), 1);
    assert_eq!(executor.count_for("irrev_webhook"), 1);

    // Verify neither mutating node was ever persisted in the tool cache
    assert_eq!(store.count_by_tenant(&tenant).await.unwrap(), 3);

    // Run 2:
    // - pure_calc, read_ledger, quote_fx -> reused from cache
    // - idem_transfer -> deduplicated by idempotency ledger (exec count stays 1, not reused from tool cache)
    // - irrev_webhook -> MUST execute live a second time (exec count becomes 2)
    let rep2 = runtime
        .execute_workflow(&tenant, &app, &plan, &store, &executor)
        .await
        .unwrap();
    assert_eq!(rep2.reused_nodes, 3);
    assert_eq!(rep2.executed_nodes, 1);
    assert_eq!(executor.count_for("pure_calc"), 1);
    assert_eq!(executor.count_for("read_ledger"), 1);
    assert_eq!(executor.count_for("quote_fx"), 1);
    assert_eq!(executor.count_for("idem_transfer"), 1);
    assert_eq!(executor.count_for("irrev_webhook"), 2);
    assert!(!rep2.node_outcomes["irrev_webhook"].reused_from_cache);
    assert!(rep2.node_outcomes["irrev_webhook"].executed_live);
}

#[tokio::test]
async fn test_upstream_recompute_taint_never_skips_downstream_mutators_or_serves_stale_children() {
    let runtime = EnforcedExecutionRuntime::new();
    let store = MemoryExecutionCacheStore::new();
    let executor = CountingToolExecutor::new();
    let tenant = TenantId::new("tenant-taint");
    let app = ApplicationId::new("app-dag");

    let mut plan = ExecutionPlan::new();
    plan.add_node(
        ExecutionNode::new(
            "step_fetch",
            "Fetch Doc",
            vec![],
            tc("fetch", "doc-1"),
            true,
        )
        .with_effect(ExecutionEffect::ReadOnlyIdempotent {
            required_state_version: 1,
            source_ids: vec![],
        }),
    );
    plan.add_node(ExecutionNode::new(
        "step_parse",
        "Parse Doc",
        vec!["step_fetch".to_string()],
        tc("parse", "doc-1"),
        true,
    ));
    plan.add_node(
        ExecutionNode::new(
            "step_commit",
            "Commit Mutation",
            vec!["step_parse".to_string()],
            tc("commit_db", "write-1"),
            false,
        )
        .with_effect(ExecutionEffect::MutatingSideEffecting {
            operation_type: "external_action".to_string(),
            idempotency_key: "".to_string(),
        }),
    );
    plan.add_node(ExecutionNode::new(
        "step_audit_fmt",
        "Format Receipt",
        vec!["step_commit".to_string()],
        tc("format_receipt", "fmt"),
        true,
    ));

    // Seed all reusable nodes
    let rep1 = runtime
        .execute_workflow(&tenant, &app, &plan, &store, &executor)
        .await
        .unwrap();
    assert_eq!(rep1.executed_nodes, 4);

    // Invalidate ONLY root fetch node while leaving step_parse and step_audit_fmt in cache
    let evicted = store.evict_by_tool(&tenant, "fetch").await.unwrap();
    assert_eq!(evicted, 1);

    // Run 2: root recomputes -> downstream pure parse MUST recompute, commit MUST execute,
    // and trailing pure step_audit_fmt MUST also recompute (never skipped or served from stale cache)
    let rep2 = runtime
        .execute_workflow(&tenant, &app, &plan, &store, &executor)
        .await
        .unwrap();
    assert_eq!(rep2.reused_nodes, 0);
    assert_eq!(rep2.executed_nodes, 4);
    assert_eq!(executor.count_for("step_fetch"), 2);
    assert_eq!(executor.count_for("step_parse"), 2);
    assert_eq!(executor.count_for("step_commit"), 2);
    assert_eq!(executor.count_for("step_audit_fmt"), 2);
}

#[tokio::test]
async fn test_fault_injection_external_tool_succeeds_but_store_recording_fails() {
    let runtime = EnforcedExecutionRuntime::new();
    let failing_store = FailingWriteStore {
        inner: MemoryExecutionCacheStore::new(),
        fail_on_set: true,
    };
    let executor = CountingToolExecutor::new();
    let tenant = TenantId::new("tenant-fault");
    let app = ApplicationId::new("app-fault");

    let mut plan = ExecutionPlan::new();
    plan.add_node(ExecutionNode::new(
        "pure_step",
        "Compute",
        vec![],
        tc("calc", "x"),
        true,
    ));
    plan.add_node(
        ExecutionNode::new(
            "side_effect_step",
            "Transfer",
            vec!["pure_step".to_string()],
            tc("transfer", "100"),
            false,
        )
        .with_effect(ExecutionEffect::MutatingSideEffecting {
            operation_type: "external_action".to_string(),
            idempotency_key: "".to_string(),
        }),
    );

    let rep = runtime
        .execute_workflow(&tenant, &app, &plan, &failing_store, &executor)
        .await
        .unwrap();
    assert_eq!(rep.executed_nodes, 2);
    assert_eq!(rep.recording_failures, 1);
    assert!(rep.node_outcomes["pure_step"].recording_failed);
    assert!(!rep.node_outcomes["side_effect_step"].recording_failed);
    // Neither node gets retried silently inside execute_workflow
    assert_eq!(executor.total(), 2);
}

#[tokio::test]
async fn test_gateway_routes_enforce_g4_on_requests_and_unified_execution_plan() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    let tenant = "tenant-g4-http";
    let app_id = "app-http";
    let api_key = "key-g4-http";
    state.api_key_registry.register_key(
        api_key,
        TenantId::new(tenant),
        ApplicationId::new(app_id),
        vec!["*".to_string()],
        false,
    );

    // 1. Pre-seed tool result in execution_store for a node with version=0 (which SubgraphPlanner
    // marks UnknownAbstain / non-reusable even if ExecutionPlanner would look up its signature)
    let mut plan = ExecutionPlan::new();
    plan.add_node(
        ExecutionNode::new(
            "v0_read",
            "Unversioned Read",
            vec![],
            tc("lookup", "k1"),
            true,
        )
        .with_effect(ExecutionEffect::ReadOnlyIdempotent {
            required_state_version: 0,
            source_ids: vec![],
        }),
    );
    plan.add_node(ExecutionNode::new(
        "safe_pure",
        "Pure",
        vec![],
        tc("pure_op", "k2"),
        true,
    ));

    let sig_v0 = sunborn_execution::ToolSignatureEngine::compute_signature(
        &TenantId::new(tenant),
        &ApplicationId::new(app_id),
        "lookup",
        "1.0",
        &json!({ "input": "k1" }),
        Some("ctx-v1"),
    )
    .unwrap();
    let sig_pure = sunborn_execution::ToolSignatureEngine::compute_signature(
        &TenantId::new(tenant),
        &ApplicationId::new(app_id),
        "pure_op",
        "1.0",
        &json!({ "input": "k2" }),
        Some("ctx-v1"),
    )
    .unwrap();

    for (tool_name, sig) in [("lookup", sig_v0), ("pure_op", sig_pure)] {
        state
            .execution_store
            .set(
                &TenantId::new(tenant),
                &sig,
                &ToolResult {
                    tool_name: tool_name.to_string(),
                    signature: sig.clone(),
                    output: json!({ "cached": true }),
                    is_error: false,
                    execution_latency_ms: 5,
                    created_at: chrono::Utc::now(),
                },
                Some(3600),
            )
            .await
            .unwrap();
    }

    let plan_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/execution/plan")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "tenant_id": tenant,
                        "application_id": app_id,
                        "plan": plan,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(plan_res.status(), StatusCode::OK);
    let eval: PlanEvaluation =
        serde_json::from_slice(&plan_res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(matches!(
        eval.node_statuses.get("safe_pure"),
        Some(NodeExecutionStatus::Cached { .. })
    ));
    assert!(
        matches!(
            eval.node_statuses.get("v0_read"),
            Some(NodeExecutionStatus::PendingExecution)
        ),
        "SubgraphPlanner intersection must downgrade version=0 stateful reads to PendingExecution"
    );

    // 2. /v1/requests with mutating ExecutionPlan in metadata must fail G4 and never cache-hit
    let mut mut_plan = ExecutionPlan::new();
    mut_plan.add_node(
        ExecutionNode::new(
            "charge_card",
            "Charge",
            vec![],
            tc("stripe_charge", "99.00"),
            false,
        )
        .with_effect(ExecutionEffect::MutatingSideEffecting {
            operation_type: "external_action".to_string(),
            idempotency_key: "".to_string(),
        }),
    );
    let mut req = RequestEnvelope::new(tenant, app_id, "gpt-4o", "Charge customer invoice 99");
    req.metadata["execution_plan"] = serde_json::to_value(&mut_plan).unwrap();

    for _ in 0..2 {
        let r = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/requests")
                    .header("content-type", "application/json")
                    .header("x-api-key", api_key)
                    .body(Body::from(serde_json::to_vec(&req).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        let env: ResponseEnvelope =
            serde_json::from_slice(&r.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert!(
            !env.cached,
            "Mutating execution plan must never be cached or reused"
        );
        let gates = env
            .decision
            .five_gate_evidence
            .expect("five_gate_evidence attached");
        assert!(!gates.g4_execution_effects.passed);
        assert!(!gates.eligible_reuse);
    }
}
