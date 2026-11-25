//! Unified Enforced Agent Execution Runtime (G4).
//!
//! Connects `sunborn_effects::SubgraphPlanner`, `sunborn_execution::ExecutionPlanner`,
//! `ExecutionCacheStore`, and a real `ToolExecutor` runtime into a single enforced
//! workflow execution lifecycle covering:
//! - Pure deterministic steps
//! - Versioned read-only stateful lookups
//! - Time-sensitive external reads
//! - Idempotent mutations (with deduplication ledger)
//! - Non-idempotent / irreversible mutations (never replayed or cached)
//! - Fault-tolerant handling when a tool succeeds externally but result recording fails

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;
use sunborn_core::error::SunbornError;
use sunborn_core::model::{ApplicationId, TenantId};
use sunborn_core::sync::{read_lock, write_lock};
use sunborn_effects::dag_planner::SubgraphPlanner;
use sunborn_execution::{
    ExecutionCacheStore, ExecutionEffect, ExecutionNode, ExecutionPlan, ExecutionPlanner,
    NodeExecutionStatus, PlanEvaluation, ToolCall, ToolResult, ToolSignatureEngine,
};

use crate::five_gates::FiveGateEvaluator;

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute_tool(
        &self,
        node_id: &str,
        effect: &ExecutionEffect,
        call: &ToolCall,
    ) -> Result<serde_json::Value, SunbornError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLifecycleOutcome {
    pub node_id: String,
    pub reused_from_cache: bool,
    pub executed_live: bool,
    pub recording_failed: bool,
    pub effect_reusable_by_subgraph_planner: bool,
    pub output: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecutionReport {
    pub total_nodes: usize,
    pub reused_nodes: usize,
    pub executed_nodes: usize,
    pub recording_failures: usize,
    pub node_outcomes: HashMap<String, NodeLifecycleOutcome>,
    pub plan_evaluation: PlanEvaluation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExternalOutcomeStatus {
    ConfirmedExecuted(serde_json::Value),
    ConfirmedNotExecuted,
    StillUnknown(String),
}

#[async_trait]
pub trait ExternalOutcomeReconciler: Send + Sync {
    async fn reconcile_operation(
        &self,
        tenant_id: &TenantId,
        idempotency_key: &str,
    ) -> Result<ExternalOutcomeStatus, SunbornError>;
}

/// Unified execution runtime enforcing both `sunborn-effects` taint analysis and
/// `sunborn-execution` topological planning & tool result caching.
pub struct EnforcedExecutionRuntime {
    /// Tracks committed idempotency keys for `MutatingSideEffecting` operations
    /// so duplicate workflow submissions do not double-execute idempotent writes.
    committed_idempotency_keys: RwLock<HashMap<String, serde_json::Value>>,
    /// Tracks uncertain/ambiguous external mutation outcomes (`UNKNOWN_OUTCOME`) pending
    /// reconciliation with the external provider (§11.4). Never blind-retried as a miss.
    unknown_outcomes: RwLock<HashMap<String, String>>,
}

impl Default for EnforcedExecutionRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl EnforcedExecutionRuntime {
    pub fn new() -> Self {
        Self {
            committed_idempotency_keys: RwLock::new(HashMap::new()),
            unknown_outcomes: RwLock::new(HashMap::new()),
        }
    }

    pub fn is_unknown_outcome(
        &self,
        tenant_id: &TenantId,
        app_id: &ApplicationId,
        key: &str,
    ) -> bool {
        let scoped = format!("{}:{}:{}", tenant_id.0, app_id.0, key);
        read_lock(&self.unknown_outcomes).contains_key(&scoped)
    }

    /// Evaluates a plan using BOTH `sunborn-effects::SubgraphPlanner` and
    /// `sunborn-execution::ExecutionPlanner`, reconciling them conservatively.
    pub async fn plan_unified(
        &self,
        tenant_id: &TenantId,
        app_id: &ApplicationId,
        plan: &ExecutionPlan,
        store: &dyn ExecutionCacheStore,
    ) -> Result<PlanEvaluation, SunbornError> {
        let effects_dag = FiveGateEvaluator::execution_plan_to_effects_dag(plan);
        let effects_plan = SubgraphPlanner::new().analyze(&effects_dag);
        let mut eval = ExecutionPlanner::plan_reuse(tenant_id, app_id, plan, store).await?;

        // Enforce strict intersection: a node may only stay Cached if BOTH
        // `SubgraphPlanner` AND `ExecutionPlanner` approve its reuse.
        let mut downgraded = Vec::new();
        for node_id in &eval.reused_node_ids {
            if !effects_plan.is_reusable(node_id) {
                downgraded.push(node_id.clone());
            }
        }
        if !downgraded.is_empty() {
            eval.reused_node_ids.retain(|id| !downgraded.contains(id));
            for id in downgraded {
                if !eval.pending_node_ids.contains(&id) {
                    eval.pending_node_ids.push(id.clone());
                }
                eval.node_statuses
                    .insert(id, NodeExecutionStatus::PendingExecution);
            }
            eval.cached_nodes = eval.reused_node_ids.len();
        }

        Ok(eval)
    }

    /// Executes an entire agent workflow DAG in topological order with enforced effect safety.
    /// Guarantees:
    /// 1. Reusable nodes reuse cached outputs only if no upstream ancestor was recomputed/tainted.
    /// 2. Downstream nodes (including mutating/irreversible steps) are NEVER skipped.
    /// 3. Mutating nodes are never stored into the replayable tool cache.
    /// 4. If tool execution succeeds externally but `store.set` fails, the failure is recorded
    ///    explicitly without retrying side-effecting operations.
    pub async fn execute_workflow<E: ToolExecutor + ?Sized>(
        &self,
        tenant_id: &TenantId,
        app_id: &ApplicationId,
        plan: &ExecutionPlan,
        store: &dyn ExecutionCacheStore,
        executor: &E,
    ) -> Result<WorkflowExecutionReport, SunbornError> {
        self.execute_workflow_with_reconciler(tenant_id, app_id, plan, store, executor, None)
            .await
    }

    pub async fn execute_workflow_with_reconciler<E: ToolExecutor + ?Sized>(
        &self,
        tenant_id: &TenantId,
        app_id: &ApplicationId,
        plan: &ExecutionPlan,
        store: &dyn ExecutionCacheStore,
        executor: &E,
        reconciler: Option<&dyn ExternalOutcomeReconciler>,
    ) -> Result<WorkflowExecutionReport, SunbornError> {
        let effects_dag = FiveGateEvaluator::execution_plan_to_effects_dag(plan);
        let effects_plan = SubgraphPlanner::new().analyze(&effects_dag);
        let eval = self.plan_unified(tenant_id, app_id, plan, store).await?;

        let ordered_nodes = Self::topo_order(&plan.nodes)?;
        let mut outputs: HashMap<String, serde_json::Value> = HashMap::new();
        let mut outcomes: HashMap<String, NodeLifecycleOutcome> = HashMap::new();
        let mut recomputed_or_mutated: HashSet<String> = HashSet::new();
        let mut reused_count = 0usize;
        let mut executed_count = 0usize;
        let mut recording_failures = 0usize;

        for node in ordered_nodes {
            let upstream_recomputed = node
                .dependencies
                .iter()
                .any(|d| recomputed_or_mutated.contains(d));
            let effects_reusable = effects_plan.is_reusable(&node.id);

            let can_use_cached = !upstream_recomputed
                && effects_reusable
                && matches!(
                    eval.node_statuses.get(&node.id),
                    Some(NodeExecutionStatus::Cached { .. })
                );

            if can_use_cached {
                if let Some(NodeExecutionStatus::Cached { output, .. }) =
                    eval.node_statuses.get(&node.id)
                {
                    reused_count += 1;
                    outputs.insert(node.id.clone(), output.clone());
                    outcomes.insert(
                        node.id.clone(),
                        NodeLifecycleOutcome {
                            node_id: node.id.clone(),
                            reused_from_cache: true,
                            executed_live: false,
                            recording_failed: false,
                            effect_reusable_by_subgraph_planner: effects_reusable,
                            output: output.clone(),
                        },
                    );
                    continue;
                }
            }

            // Must execute live
            recomputed_or_mutated.insert(node.id.clone());
            let Some(ref tool_call) = node.tool_call else {
                let empty = serde_json::json!({ "status": "no_tool_call" });
                outputs.insert(node.id.clone(), empty.clone());
                outcomes.insert(
                    node.id.clone(),
                    NodeLifecycleOutcome {
                        node_id: node.id.clone(),
                        reused_from_cache: false,
                        executed_live: false,
                        recording_failed: false,
                        effect_reusable_by_subgraph_planner: effects_reusable,
                        output: empty,
                    },
                );
                continue;
            };

            // Idempotent & UnknownOutcome reconciliation guard (§11.4)
            let mut active_scoped_idem: Option<String> = None;
            if let ExecutionEffect::MutatingSideEffecting {
                operation_type,
                idempotency_key,
            } = &node.effect
            {
                if !idempotency_key.is_empty() {
                    let scoped_idem = format!("{}:{}:{}", tenant_id.0, app_id.0, idempotency_key);
                    if read_lock(&self.unknown_outcomes).contains_key(&scoped_idem) {
                        if let Some(rec) = reconciler {
                            match rec.reconcile_operation(tenant_id, idempotency_key).await? {
                                ExternalOutcomeStatus::ConfirmedExecuted(val) => {
                                    write_lock(&self.unknown_outcomes).remove(&scoped_idem);
                                    write_lock(&self.committed_idempotency_keys)
                                        .insert(scoped_idem.clone(), val.clone());
                                    outputs.insert(node.id.clone(), val.clone());
                                    outcomes.insert(
                                        node.id.clone(),
                                        NodeLifecycleOutcome {
                                            node_id: node.id.clone(),
                                            reused_from_cache: false,
                                            executed_live: false,
                                            recording_failed: false,
                                            effect_reusable_by_subgraph_planner: false,
                                            output: val,
                                        },
                                    );
                                    continue;
                                }
                                ExternalOutcomeStatus::ConfirmedNotExecuted => {
                                    write_lock(&self.unknown_outcomes).remove(&scoped_idem);
                                }
                                ExternalOutcomeStatus::StillUnknown(detail) => {
                                    return Err(SunbornError::internal(format!(
                                        "UNKNOWN_OUTCOME blocks retry for '{}': {}",
                                        node.id, detail
                                    )));
                                }
                            }
                        } else {
                            return Err(SunbornError::internal(format!(
                                "UNKNOWN_OUTCOME recorded for '{}' ({}); reconciler required before retry",
                                node.id, idempotency_key
                            )));
                        }
                    }
                    if operation_type == "idempotent" {
                        if let Some(prev_out) =
                            read_lock(&self.committed_idempotency_keys).get(&scoped_idem)
                        {
                            outputs.insert(node.id.clone(), prev_out.clone());
                            outcomes.insert(
                                node.id.clone(),
                                NodeLifecycleOutcome {
                                    node_id: node.id.clone(),
                                    reused_from_cache: false,
                                    executed_live: false,
                                    recording_failed: false,
                                    effect_reusable_by_subgraph_planner: false,
                                    output: prev_out.clone(),
                                },
                            );
                            continue;
                        }
                    }
                    active_scoped_idem = Some(scoped_idem);
                }
            }

            let t0 = std::time::Instant::now();
            let output = match executor
                .execute_tool(&node.id, &node.effect, tool_call)
                .await
            {
                Ok(out) => out,
                Err(err) => {
                    if let Some(scoped_idem) = active_scoped_idem {
                        write_lock(&self.unknown_outcomes).insert(scoped_idem, err.to_string());
                    }
                    return Err(err);
                }
            };
            let elapsed_ms = t0.elapsed().as_millis() as u64;
            executed_count += 1;

            if let ExecutionEffect::MutatingSideEffecting {
                operation_type,
                idempotency_key,
            } = &node.effect
            {
                if operation_type == "idempotent" && !idempotency_key.is_empty() {
                    let scoped_idem = format!("{}:{}:{}", tenant_id.0, app_id.0, idempotency_key);
                    write_lock(&self.committed_idempotency_keys)
                        .insert(scoped_idem, output.clone());
                }
            }

            // Record into execution cache ONLY if the node effect is inherently reusable
            let mut rec_failed = false;
            if effects_reusable
                && !matches!(node.effect, ExecutionEffect::MutatingSideEffecting { .. })
            {
                let ver = if tool_call.tool_version.is_empty() {
                    "1.0"
                } else {
                    tool_call.tool_version.as_str()
                };
                let sig = ToolSignatureEngine::compute_signature(
                    tenant_id,
                    app_id,
                    &tool_call.tool_name,
                    ver,
                    &tool_call.arguments,
                    tool_call.context_hash.as_deref(),
                )?;
                let ttl = match &node.effect {
                    ExecutionEffect::TimeSensitiveNonDeterministic { max_ttl_secs } => {
                        Some(*max_ttl_secs)
                    }
                    _ => Some(3600),
                };
                let tr = ToolResult {
                    tool_name: tool_call.tool_name.clone(),
                    signature: sig.clone(),
                    output: output.clone(),
                    is_error: false,
                    execution_latency_ms: elapsed_ms,
                    created_at: Utc::now(),
                };
                if let Err(err) = store.set(tenant_id, &sig, &tr, ttl).await {
                    tracing::error!(
                        node_id = %node.id,
                        error = %err,
                        "Tool execution succeeded externally, but result recording failed; marking non-replayable"
                    );
                    rec_failed = true;
                    recording_failures += 1;
                }
            }

            outputs.insert(node.id.clone(), output.clone());
            outcomes.insert(
                node.id.clone(),
                NodeLifecycleOutcome {
                    node_id: node.id.clone(),
                    reused_from_cache: false,
                    executed_live: true,
                    recording_failed: rec_failed,
                    effect_reusable_by_subgraph_planner: effects_reusable,
                    output,
                },
            );
        }

        Ok(WorkflowExecutionReport {
            total_nodes: plan.nodes.len(),
            reused_nodes: reused_count,
            executed_nodes: executed_count,
            recording_failures,
            node_outcomes: outcomes,
            plan_evaluation: eval,
        })
    }

    fn topo_order(nodes: &[ExecutionNode]) -> Result<Vec<ExecutionNode>, SunbornError> {
        let mut node_map: HashMap<String, ExecutionNode> = HashMap::new();
        let mut in_deg: HashMap<String, usize> = HashMap::new();
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();
        for n in nodes {
            node_map.insert(n.id.clone(), n.clone());
            in_deg.insert(n.id.clone(), n.dependencies.len());
            adj.entry(n.id.clone()).or_default();
            for dep in &n.dependencies {
                adj.entry(dep.clone()).or_default().push(n.id.clone());
            }
        }
        let mut q: std::collections::VecDeque<String> = in_deg
            .iter()
            .filter_map(|(k, &d)| if d == 0 { Some(k.clone()) } else { None })
            .collect();
        let mut out = Vec::with_capacity(nodes.len());
        while let Some(id) = q.pop_front() {
            if let Some(n) = node_map.remove(&id) {
                out.push(n);
            }
            if let Some(nexts) = adj.get(&id) {
                for nxt in nexts {
                    if let Some(d) = in_deg.get_mut(nxt) {
                        *d -= 1;
                        if *d == 0 {
                            q.push_back(nxt.clone());
                        }
                    }
                }
            }
        }
        if out.len() != nodes.len() {
            return Err(SunbornError::invalid_payload(
                "Cycle detected in execution DAG",
            ));
        }
        Ok(out)
    }
}
