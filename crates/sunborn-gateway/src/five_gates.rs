//! Unified Five-Gate Eligibility Contract:
//! `EligibleReuse = G1 && G2 && G3 && G4 && G5`
//!
//! Every cache admission/reuse decision passes through this auditable boundary,
//! recording gate-by-gate pass/fail evidence across:
//! - G1: Identity and task equivalence
//! - G2: Context and authorization (tenant, app, model, principal/session scope)
//! - G3: Dependency freshness (epoch leases, content hashes, 5D temporal validity)
//! - G4: Execution-effect safety (effect classes & sunborn-effects SubgraphPlanner)
//! - G5: Evidence and policy control (ReuseCertificate, policy selection, conformal bounds)

use crate::middleware::auth::SecurityContext;
use sunborn_core::{FiveGateEvidence, GateCheckResult, RequestEnvelope};
use sunborn_effects::dag_planner::{AgentExecutionDAG, DAGNode, SubgraphPlanner};
use sunborn_effects::effect_class::EffectClass;
use sunborn_execution::{ExecutionEffect, ExecutionPlan};

pub struct FiveGateEvaluator;

impl FiveGateEvaluator {
    /// G2 — Context & Authorization gate (tenant, app, model, and session/user scope).
    pub fn evaluate_g2_context(
        req: &RequestEnvelope,
        sec_ctx: &SecurityContext,
        candidate_metadata: Option<&serde_json::Value>,
    ) -> GateCheckResult {
        if !sec_ctx.is_authenticated {
            return GateCheckResult::fail("G2_UNAUTHENTICATED");
        }
        if req.tenant_id != sec_ctx.tenant_id || req.application_id != sec_ctx.application_id {
            return GateCheckResult::fail("G2_TENANT_OR_APP_MISMATCH");
        }
        if !sec_ctx.allows_model(&req.model) {
            return GateCheckResult::fail("G2_MODEL_UNAUTHORIZED");
        }

        if let Some(meta) = candidate_metadata {
            let req_session = req.metadata.get("session_id").and_then(|v| v.as_str());
            let cand_session = meta.get("session_id").and_then(|v| v.as_str());
            if req_session != cand_session {
                return GateCheckResult::fail("G2_SESSION_SCOPE_MISMATCH");
            }
            let req_user = req.metadata.get("user_id").and_then(|v| v.as_str());
            let cand_user = meta.get("user_id").and_then(|v| v.as_str());
            if req_user != cand_user {
                return GateCheckResult::fail("G2_USER_SCOPE_MISMATCH");
            }
        }

        GateCheckResult::pass("G2_CONTEXT_AUTHORIZED")
    }

    /// G4 — Execution-Effect Safety gate.
    /// Evaluates declared `effect_class`, `execution_plan`, and tool annotations through
    /// `sunborn-effects::SubgraphPlanner`. Mutating, non-idempotent, or unknown effects
    /// prohibit whole-response cache reuse.
    pub fn evaluate_g4_effects(req: &RequestEnvelope) -> GateCheckResult {
        if let Some(effect_str) = req.metadata.get("effect_class").and_then(|v| v.as_str()) {
            let parsed = match effect_str {
                "Pure" => EffectClass::Pure,
                "ReadOnlyStateful" => EffectClass::ReadOnlyStateful { version: 1 },
                "ExternalMutableRead" => EffectClass::ExternalMutableRead { ttl_secs: 60 },
                "IdempotentMutation" => EffectClass::IdempotentMutation {
                    idempotency_key: "req".to_string(),
                },
                "NonIdempotentMutation" => EffectClass::NonIdempotentMutation,
                _ => EffectClass::UnknownAbstain,
            };
            if !parsed.is_inherently_reusable() {
                return GateCheckResult::fail(format!("G4_EFFECT_PROHIBITS_REUSE:{effect_str}"));
            }
        }

        if let Some(plan_val) = req.metadata.get("execution_plan") {
            match serde_json::from_value::<ExecutionPlan>(plan_val.clone()) {
                Ok(plan) => {
                    let dag = Self::execution_plan_to_effects_dag(&plan);
                    let reuse = SubgraphPlanner::new().analyze(&dag);
                    if !reuse.prohibited_nodes.is_empty() || !reuse.recompute_nodes.is_empty() {
                        return GateCheckResult::fail(
                            "G4_EXECUTION_DAG_CONTAINS_MUTATING_OR_TAINTED_NODES",
                        );
                    }
                }
                Err(_) => {
                    return GateCheckResult::fail("G4_MALFORMED_EXECUTION_PLAN_ABSTAIN");
                }
            }
        }

        if !req.tools.is_empty()
            && req
                .metadata
                .get("tools_have_side_effects")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        {
            return GateCheckResult::fail("G4_SIDE_EFFECTING_TOOLS_PRESENT");
        }

        GateCheckResult::pass("G4_EFFECT_SAFE")
    }

    /// Maps a `sunborn_execution::ExecutionPlan` into a `sunborn_effects::AgentExecutionDAG`.
    pub fn execution_plan_to_effects_dag(plan: &ExecutionPlan) -> AgentExecutionDAG {
        let mut dag = AgentExecutionDAG::new();
        for node in &plan.nodes {
            let effect_class = match &node.effect {
                ExecutionEffect::PureDeterministic => {
                    if node.is_deterministic {
                        EffectClass::Pure
                    } else {
                        EffectClass::UnknownAbstain
                    }
                }
                ExecutionEffect::ReadOnlyIdempotent {
                    required_state_version,
                    ..
                } => {
                    if *required_state_version > 0 {
                        EffectClass::ReadOnlyStateful {
                            version: *required_state_version,
                        }
                    } else {
                        EffectClass::UnknownAbstain
                    }
                }
                ExecutionEffect::TimeSensitiveNonDeterministic { max_ttl_secs } => {
                    if *max_ttl_secs > 0 {
                        EffectClass::ExternalMutableRead {
                            ttl_secs: *max_ttl_secs,
                        }
                    } else {
                        EffectClass::UnknownAbstain
                    }
                }
                ExecutionEffect::MutatingSideEffecting {
                    operation_type,
                    idempotency_key,
                } => {
                    if operation_type == "idempotent" && !idempotency_key.is_empty() {
                        EffectClass::IdempotentMutation {
                            idempotency_key: idempotency_key.clone(),
                        }
                    } else {
                        EffectClass::NonIdempotentMutation
                    }
                }
            };
            let mut dag_node = DAGNode::new(&node.id, effect_class);
            for dep in &node.dependencies {
                dag_node = dag_node.with_dep(dep);
            }
            dag.add_node(dag_node);
        }
        dag
    }

    /// Combines all 5 gate checks into one unified `FiveGateEvidence` record.
    pub fn synthesize(
        g1: GateCheckResult,
        g2: GateCheckResult,
        g3: GateCheckResult,
        g4: GateCheckResult,
        g5: GateCheckResult,
    ) -> FiveGateEvidence {
        FiveGateEvidence::evaluate(g1, g2, g3, g4, g5)
    }
}
