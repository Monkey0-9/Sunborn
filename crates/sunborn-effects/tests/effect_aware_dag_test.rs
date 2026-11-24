use sunborn_effects::dag_planner::{AgentExecutionDAG, DAGNode, SubgraphPlanner};
use sunborn_effects::effect_class::EffectClass;

#[test]
fn test_effect_aware_dag_prevents_mutating_subgraph_reuse() {
    let mut dag = AgentExecutionDAG::new();
    dag.add_node(DAGNode::new(
        "read_doc",
        EffectClass::ReadOnlyStateful { version: 1 },
    ));
    dag.add_node(DAGNode::new("calc_interest", EffectClass::Pure).with_dep("read_doc"));
    dag.add_node(
        DAGNode::new("execute_payment", EffectClass::NonIdempotentMutation)
            .with_dep("calc_interest"),
    );

    let planner = SubgraphPlanner::new();
    let plan = planner.analyze(&dag);

    assert!(plan.is_reusable("read_doc"));
    assert!(plan.is_reusable("calc_interest"));
    assert!(!plan.is_reusable("execute_payment"));
}

#[test]
fn test_transitive_mutation_taint_propagates_downstream() {
    let mut dag = AgentExecutionDAG::new();
    // Upstream mutating step
    dag.add_node(DAGNode::new(
        "debit_account",
        EffectClass::NonIdempotentMutation,
    ));
    // Downstream calculation marked Pure, but depends on mutating step -> tainted!
    dag.add_node(DAGNode::new("format_receipt", EffectClass::Pure).with_dep("debit_account"));

    let planner = SubgraphPlanner::new();
    let plan = planner.analyze(&dag);

    assert!(
        !plan.is_reusable("format_receipt"),
        "Downstream of mutation must be tainted"
    );
    assert!(!plan.is_reusable("debit_account"));
}

#[test]
fn test_multi_level_pure_dag_is_wholly_reusable() {
    let mut dag = AgentExecutionDAG::new();
    dag.add_node(DAGNode::new("step1", EffectClass::Pure));
    dag.add_node(DAGNode::new("step2", EffectClass::Pure).with_dep("step1"));
    dag.add_node(DAGNode::new("step3", EffectClass::Pure).with_dep("step2"));
    dag.add_node(
        DAGNode::new(
            "step4",
            EffectClass::ExternalMutableRead { ttl_secs: 300 },
        )
        .with_dep("step3"),
    );

    let planner = SubgraphPlanner::new();
    let plan = planner.analyze(&dag);

    assert!(plan.is_reusable("step1"));
    assert!(plan.is_reusable("step2"));
    assert!(plan.is_reusable("step3"));
    assert!(plan.is_reusable("step4"));
}
