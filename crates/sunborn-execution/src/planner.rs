use crate::model::{ExecutionNode, ExecutionPlan, NodeExecutionStatus, PlanEvaluation};
use crate::signature::ToolSignatureEngine;
use crate::store::ExecutionCacheStore;
use chrono::Utc;
use std::collections::{HashMap, VecDeque};
use sunborn_core::error::SunbornError;
use sunborn_core::model::{ApplicationId, TenantId};

pub struct ExecutionPlanner;

impl ExecutionPlanner {
    /// Validates the DAG topology, verifies absence of cycles, and evaluates cached step reuse.
    pub async fn plan_reuse(
        tenant_id: &TenantId,
        app_id: &ApplicationId,
        plan: &ExecutionPlan,
        store: &dyn ExecutionCacheStore,
    ) -> Result<PlanEvaluation, SunbornError> {
        let sorted_nodes = Self::topological_sort(&plan.nodes)?;

        let mut node_statuses: HashMap<String, NodeExecutionStatus> = HashMap::new();
        let mut reused_node_ids = Vec::new();
        let mut pending_node_ids = Vec::new();
        let mut estimated_latency_saved_ms = 0u64;

        for node in &sorted_nodes {
            // Invariant 1: Topological Descendant Invalidation
            // If any upstream dependency is pending execution or was recomputed, this node must also recompute.
            let has_pending_dependency = node
                .dependencies
                .iter()
                .any(|dep| pending_node_ids.contains(dep));

            // Invariant 2: Typed Effect Contract Verification
            // Non-reusable is the strict default.
            // MutatingSideEffecting (payments, writes, external actions) is NEVER reusable.
            // PureDeterministic is only reusable if node.is_deterministic == true.
            // ReadOnlyIdempotent is only reusable if required_state_version > 0.
            // TimeSensitiveNonDeterministic is only reusable if max_ttl_secs > 0 and within TTL validity window.
            let (effect_permits_reuse, max_ttl) = match &node.effect {
                crate::model::ExecutionEffect::PureDeterministic => (node.is_deterministic, None),
                crate::model::ExecutionEffect::ReadOnlyIdempotent {
                    required_state_version,
                    ..
                } => (*required_state_version > 0, None),
                crate::model::ExecutionEffect::TimeSensitiveNonDeterministic { max_ttl_secs } => {
                    if *max_ttl_secs > 0 {
                        (true, Some(*max_ttl_secs))
                    } else {
                        (false, None)
                    }
                }
                crate::model::ExecutionEffect::MutatingSideEffecting { .. } => (false, None),
            };

            let can_reuse =
                !has_pending_dependency && effect_permits_reuse && node.tool_call.is_some();

            if can_reuse {
                let tool_call = node.tool_call.as_ref().unwrap();

                // Incorporate upstream dependency signatures into context hash for strict causal lineage
                let mut dep_context_parts = Vec::new();
                for dep in &node.dependencies {
                    if let Some(NodeExecutionStatus::Cached { signature, .. }) =
                        node_statuses.get(dep)
                    {
                        dep_context_parts.push(format!("{dep}:{signature}"));
                    }
                }
                let effective_context_hash = if !dep_context_parts.is_empty() {
                    let mut combined = tool_call.context_hash.clone().unwrap_or_default();
                    if !combined.is_empty() {
                        combined.push(';');
                    }
                    combined.push_str(&dep_context_parts.join(";"));
                    Some(combined)
                } else {
                    tool_call.context_hash.clone()
                };

                let signature = ToolSignatureEngine::compute_signature(
                    tenant_id,
                    app_id,
                    &tool_call.tool_name,
                    &tool_version_or_default(&tool_call.tool_version),
                    &tool_call.arguments,
                    effective_context_hash.as_deref(),
                )?;

                if let Some(cached_result) = store.get(tenant_id, &signature).await? {
                    // Check TTL validity for time-sensitive results
                    let is_valid = if let Some(ttl_secs) = max_ttl {
                        let age = (Utc::now() - cached_result.created_at).num_seconds();
                        age >= 0 && (age as u64) <= ttl_secs
                    } else {
                        true
                    };

                    if is_valid {
                        let latency_saved = cached_result.execution_latency_ms;
                        estimated_latency_saved_ms += latency_saved;
                        reused_node_ids.push(node.id.clone());
                        node_statuses.insert(
                            node.id.clone(),
                            NodeExecutionStatus::Cached {
                                signature,
                                output: cached_result.output,
                                latency_saved_ms: latency_saved,
                            },
                        );
                        continue;
                    }
                }
            }

            // Fallback: Pending execution
            pending_node_ids.push(node.id.clone());
            node_statuses.insert(node.id.clone(), NodeExecutionStatus::PendingExecution);
        }

        let total_nodes = plan.nodes.len();
        let cached_nodes = reused_node_ids.len();

        Ok(PlanEvaluation {
            total_nodes,
            cached_nodes,
            reused_node_ids,
            pending_node_ids,
            estimated_latency_saved_ms,
            node_statuses,
        })
    }

    /// Performs topological sort using Kahn's algorithm; errors if cycles are detected.
    fn topological_sort(nodes: &[ExecutionNode]) -> Result<Vec<ExecutionNode>, SunbornError> {
        let mut node_map: HashMap<String, ExecutionNode> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();

        for node in nodes {
            if node_map.contains_key(&node.id) {
                return Err(SunbornError::invalid_payload(format!(
                    "Duplicate node ID in execution plan: {}",
                    node.id
                )));
            }
            node_map.insert(node.id.clone(), node.clone());
            in_degree.insert(node.id.clone(), 0);
            adj.insert(node.id.clone(), Vec::new());
        }

        // Validate dependencies
        for node in nodes {
            for dep in &node.dependencies {
                if !node_map.contains_key(dep) {
                    return Err(SunbornError::invalid_payload(format!(
                        "Node '{}' depends on undefined node '{}'",
                        node.id, dep
                    )));
                }
                adj.get_mut(dep).unwrap().push(node.id.clone());
                *in_degree.get_mut(&node.id).unwrap() += 1;
            }
        }

        let mut queue: VecDeque<String> = VecDeque::new();
        for (id, deg) in &in_degree {
            if *deg == 0 {
                queue.push_back(id.clone());
            }
        }

        let mut sorted = Vec::with_capacity(nodes.len());
        while let Some(u) = queue.pop_front() {
            sorted.push(node_map.get(&u).unwrap().clone());
            if let Some(neighbors) = adj.get(&u) {
                for v in neighbors {
                    let deg = in_degree.get_mut(v).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(v.clone());
                    }
                }
            }
        }

        if sorted.len() != nodes.len() {
            return Err(SunbornError::invalid_payload(
                "Execution plan contains a cycle or unreachable dependencies",
            ));
        }

        Ok(sorted)
    }
}

fn tool_version_or_default(v: &str) -> String {
    if v.is_empty() {
        "1.0.0".to_string()
    } else {
        v.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ToolCall, ToolResult};
    use crate::store::MemoryExecutionCacheStore;
    use chrono::Utc;

    #[tokio::test]
    async fn test_execution_plan_dag_evaluation_and_reuse() {
        let store = MemoryExecutionCacheStore::new();
        let tenant = TenantId::new("tenant-exec-1");
        let app = ApplicationId::new("agent-app");

        // Seed cache for node 1
        let tool_args = serde_json::json!({"query": "SELECT count(*) FROM sales"});
        let sig = ToolSignatureEngine::compute_signature(
            &tenant,
            &app,
            "sql_runner",
            "1.0.0",
            &tool_args,
            None,
        )
        .unwrap();

        let cached_result = ToolResult {
            tool_name: "sql_runner".into(),
            signature: sig.clone(),
            output: serde_json::json!({"count": 4820}),
            is_error: false,
            execution_latency_ms: 120,
            created_at: Utc::now(),
        };
        store
            .set(&tenant, &sig, &cached_result, None)
            .await
            .unwrap();

        // Build execution plan:
        // Node 1: deterministic SQL runner (should hit cache)
        // Node 2: non-deterministic LLM summary depending on Node 1 (must execute)
        let mut plan = ExecutionPlan::new();
        plan.add_node(ExecutionNode::new(
            "node_1",
            "Run SQL Query",
            vec![],
            Some(ToolCall {
                tool_name: "sql_runner".into(),
                tool_version: "1.0.0".into(),
                arguments: tool_args,
                context_hash: None,
            }),
            true,
        ));

        plan.add_node(ExecutionNode::new(
            "node_2",
            "Synthesize Report",
            vec!["node_1".into()],
            Some(ToolCall {
                tool_name: "llm_synthesizer".into(),
                tool_version: "1.0.0".into(),
                arguments: serde_json::json!({"prompt": "Summarize sales"}),
                context_hash: None,
            }),
            false, // non-deterministic -> must not reuse
        ));

        let eval = ExecutionPlanner::plan_reuse(&tenant, &app, &plan, &store)
            .await
            .unwrap();

        assert_eq!(eval.total_nodes, 2);
        assert_eq!(eval.cached_nodes, 1);
        assert_eq!(eval.reused_node_ids, vec!["node_1"]);
        assert_eq!(eval.pending_node_ids, vec!["node_2"]);
        assert_eq!(eval.estimated_latency_saved_ms, 120);

        match eval.node_statuses.get("node_1").unwrap() {
            NodeExecutionStatus::Cached {
                output,
                latency_saved_ms,
                ..
            } => {
                assert_eq!(output["count"], 4820);
                assert_eq!(*latency_saved_ms, 120);
            }
            _ => panic!("Expected node_1 to be Cached"),
        }

        match eval.node_statuses.get("node_2").unwrap() {
            NodeExecutionStatus::PendingExecution => (),
            _ => panic!("Expected node_2 to be PendingExecution"),
        }
    }

    #[tokio::test]
    async fn test_execution_plan_cycle_detection() {
        let store = MemoryExecutionCacheStore::new();
        let tenant = TenantId::new("t1");
        let app = ApplicationId::new("app1");

        let mut plan = ExecutionPlan::new();
        plan.add_node(ExecutionNode::new("A", "A", vec!["B".into()], None, true));
        plan.add_node(ExecutionNode::new("B", "B", vec!["A".into()], None, true));

        let result = ExecutionPlanner::plan_reuse(&tenant, &app, &plan, &store).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_side_effecting_mutating_node_never_reused() {
        use crate::model::ExecutionEffect;
        let store = MemoryExecutionCacheStore::new();
        let tenant = TenantId::new("t1");
        let app = ApplicationId::new("app1");

        let args = serde_json::json!({"recipient": "alice@corp.com", "amount": 500});
        let sig = ToolSignatureEngine::compute_signature(
            &tenant,
            &app,
            "send_payment",
            "1.0",
            &args,
            None,
        )
        .unwrap();
        store
            .set(
                &tenant,
                &sig,
                &crate::model::ToolResult {
                    tool_name: "send_payment".to_string(),
                    signature: sig.clone(),
                    output: serde_json::json!({"tx_id": "tx_999", "status": "COMPLETED"}),
                    is_error: false,
                    execution_latency_ms: 250,
                    created_at: chrono::Utc::now(),
                },
                None,
            )
            .await
            .unwrap();

        let mut plan = ExecutionPlan::new();
        plan.add_node(
            ExecutionNode::new(
                "pay_node",
                "Execute Payment",
                vec![],
                Some(ToolCall {
                    tool_name: "send_payment".into(),
                    tool_version: "1.0".into(),
                    arguments: args,
                    context_hash: None,
                }),
                true,
            )
            .with_effect(ExecutionEffect::MutatingSideEffecting {
                operation_type: "financial_transfer".into(),
                idempotency_key: "tx_idem_1".into(),
            }),
        );

        let eval = ExecutionPlanner::plan_reuse(&tenant, &app, &plan, &store)
            .await
            .unwrap();
        assert_eq!(
            eval.cached_nodes, 0,
            "Mutating side-effecting operations must NEVER be reused from cache"
        );
        assert_eq!(eval.pending_node_ids, vec!["pay_node"]);
    }

    #[tokio::test]
    async fn test_topological_descendant_invalidation_cascades_safely() {
        let store = MemoryExecutionCacheStore::new();
        let tenant = TenantId::new("t1");
        let app = ApplicationId::new("app1");

        // Seed cache for node 1, node 2, node 3
        for i in 1..=3 {
            let name = format!("tool_{}", i);
            let args = serde_json::json!({"step": i});
            let sig =
                ToolSignatureEngine::compute_signature(&tenant, &app, &name, "1.0", &args, None)
                    .unwrap();
            store
                .set(
                    &tenant,
                    &sig.clone(),
                    &crate::model::ToolResult {
                        tool_name: name,
                        signature: sig,
                        output: serde_json::json!({"result": i}),
                        is_error: false,
                        execution_latency_ms: 50,
                        created_at: chrono::Utc::now(),
                    },
                    None,
                )
                .await
                .unwrap();
        }

        // Plan:
        // node_1 (cached)
        // node_2 depends on node_1 (uncached / mutated args)
        // node_3 depends on node_2 (cached in store, but MUST recompute because dependency node_2 is pending!)
        let mut plan = ExecutionPlan::new();
        plan.add_node(ExecutionNode::new(
            "node_1",
            "Step 1",
            vec![],
            Some(ToolCall {
                tool_name: "tool_1".into(),
                tool_version: "1.0".into(),
                arguments: serde_json::json!({"step": 1}),
                context_hash: None,
            }),
            true,
        ));

        // node_2 arguments mutated to step=999 (not in cache)
        plan.add_node(ExecutionNode::new(
            "node_2",
            "Step 2 Mutated",
            vec!["node_1".into()],
            Some(ToolCall {
                tool_name: "tool_2".into(),
                tool_version: "1.0".into(),
                arguments: serde_json::json!({"step": 999}),
                context_hash: None,
            }),
            true,
        ));

        // node_3 in cache, but depends on node_2
        plan.add_node(ExecutionNode::new(
            "node_3",
            "Step 3",
            vec!["node_2".into()],
            Some(ToolCall {
                tool_name: "tool_3".into(),
                tool_version: "1.0".into(),
                arguments: serde_json::json!({"step": 3}),
                context_hash: None,
            }),
            true,
        ));

        let eval = ExecutionPlanner::plan_reuse(&tenant, &app, &plan, &store)
            .await
            .unwrap();
        assert_eq!(eval.reused_node_ids, vec!["node_1"]);
        assert!(eval.pending_node_ids.contains(&"node_2".to_string()));
        assert!(
            eval.pending_node_ids.contains(&"node_3".to_string()),
            "Downstream descendant of node_2 must be recomputed!"
        );
    }

    #[tokio::test]
    async fn test_time_sensitive_ttl_expiry_aborts_reuse() {
        let store = MemoryExecutionCacheStore::new();
        let tenant = TenantId::new("t-ttl");
        let app = ApplicationId::new("app-ttl");

        let args = serde_json::json!({"market": "BTC-USD"});
        let sig =
            ToolSignatureEngine::compute_signature(&tenant, &app, "get_price", "1.0", &args, None)
                .unwrap();

        // Stale result created 120 seconds ago
        let stale_result = crate::model::ToolResult {
            tool_name: "get_price".into(),
            signature: sig.clone(),
            output: serde_json::json!({"price": 65000}),
            is_error: false,
            execution_latency_ms: 80,
            created_at: Utc::now() - chrono::Duration::seconds(120),
        };
        store.set(&tenant, &sig, &stale_result, None).await.unwrap();

        // Plan with node requiring max_ttl_secs = 30 seconds
        let mut plan = ExecutionPlan::new();
        plan.add_node(
            ExecutionNode::new(
                "price_node",
                "Fetch Current Price",
                vec![],
                Some(ToolCall {
                    tool_name: "get_price".into(),
                    tool_version: "1.0".into(),
                    arguments: args,
                    context_hash: None,
                }),
                false,
            )
            .with_effect(
                crate::model::ExecutionEffect::TimeSensitiveNonDeterministic { max_ttl_secs: 30 },
            ),
        );

        let eval = ExecutionPlanner::plan_reuse(&tenant, &app, &plan, &store)
            .await
            .unwrap();
        assert_eq!(
            eval.cached_nodes, 0,
            "Expired time-sensitive result must not be reused"
        );
        assert_eq!(eval.pending_node_ids, vec!["price_node"]);
    }

    #[tokio::test]
    async fn test_dag_validation_rejects_missing_dep_and_duplicate_nodes() {
        let tenant = TenantId::new("t-val");
        let app = ApplicationId::new("app-val");
        let store = MemoryExecutionCacheStore::new();

        // 1. Missing dependency
        let mut plan_missing = ExecutionPlan::new();
        plan_missing.add_node(ExecutionNode::new(
            "node_b",
            "Node B",
            vec!["node_ghost".to_string()], // non-existent node
            None,
            true,
        ));
        assert!(
            ExecutionPlanner::plan_reuse(&tenant, &app, &plan_missing, &store)
                .await
                .is_err()
        );

        // 2. Duplicate node IDs
        let mut plan_dup = ExecutionPlan::new();
        plan_dup.add_node(ExecutionNode::new("same_id", "Node 1", vec![], None, true));
        plan_dup.add_node(ExecutionNode::new("same_id", "Node 2", vec![], None, true));
        assert!(
            ExecutionPlanner::plan_reuse(&tenant, &app, &plan_dup, &store)
                .await
                .is_err()
        );
    }
}
