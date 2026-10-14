use crate::model::{ExecutionNode, ExecutionPlan, NodeExecutionStatus, PlanEvaluation};
use crate::signature::ToolSignatureEngine;
use crate::store::ExecutionCacheStore;
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
            // Check if node is a candidate for reuse
            let can_reuse = node.is_deterministic && node.tool_call.is_some();

            if can_reuse {
                let tool_call = node.tool_call.as_ref().unwrap();
                let signature = ToolSignatureEngine::compute_signature(
                    tenant_id,
                    app_id,
                    &tool_call.tool_name,
                    &tool_version_or_default(&tool_call.tool_version),
                    &tool_call.arguments,
                    tool_call.context_hash.as_deref(),
                )?;

                if let Some(cached_result) = store.get(tenant_id, &signature).await? {
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
        plan.add_node(ExecutionNode {
            id: "node_1".into(),
            name: "Run SQL Query".into(),
            dependencies: vec![],
            tool_call: Some(ToolCall {
                tool_name: "sql_runner".into(),
                tool_version: "1.0.0".into(),
                arguments: tool_args,
                context_hash: None,
            }),
            is_deterministic: true,
        });

        plan.add_node(ExecutionNode {
            id: "node_2".into(),
            name: "Synthesize Report".into(),
            dependencies: vec!["node_1".into()],
            tool_call: Some(ToolCall {
                tool_name: "llm_synthesizer".into(),
                tool_version: "1.0.0".into(),
                arguments: serde_json::json!({"prompt": "Summarize sales"}),
                context_hash: None,
            }),
            is_deterministic: false, // non-deterministic -> must not reuse
        });

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
        plan.add_node(ExecutionNode {
            id: "A".into(),
            name: "A".into(),
            dependencies: vec!["B".into()],
            tool_call: None,
            is_deterministic: true,
        });
        plan.add_node(ExecutionNode {
            id: "B".into(),
            name: "B".into(),
            dependencies: vec!["A".into()],
            tool_call: None,
            is_deterministic: true,
        });

        let result = ExecutionPlanner::plan_reuse(&tenant, &app, &plan, &store).await;
        assert!(result.is_err());
    }
}
