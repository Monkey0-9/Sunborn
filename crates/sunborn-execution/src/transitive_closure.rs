// ============================================================================
// crates/sunborn-execution/src/transitive_closure.rs
// Transitive DAG Purity, TTL, and Lineage Envelope Propagation (Tier C3)
// Prevents pure child nodes from outliving or ignoring mutable ancestors.
// ============================================================================
use std::collections::{BTreeSet, HashMap, VecDeque};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum ToolPurity {
    PureDeterministic = 0,
    ReadIdempotent = 1,
    SideEffectingMutator = 2,
}

#[derive(Debug, Clone)]
pub struct DagNodeSpec {
    pub node_id: String,
    pub intrinsic_purity: ToolPurity,
    pub intrinsic_ttl_ms: Option<u64>,
    pub intrinsic_lineage_keys: BTreeSet<String>,
    pub parents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DerivedNodeEnvelope {
    pub effective_purity: ToolPurity,
    pub effective_ttl_ms: Option<u64>,
    pub effective_lineage_keys: BTreeSet<String>,
    pub cacheable: bool,
}

pub fn compute_dag_transitive_envelopes(
    nodes: &[DagNodeSpec],
) -> Result<HashMap<String, DerivedNodeEnvelope>, &'static str> {
    let mut spec_map: HashMap<String, &DagNodeSpec> = HashMap::with_capacity(nodes.len());
    let mut in_degree: HashMap<String, usize> = HashMap::with_capacity(nodes.len());
    let mut children_map: HashMap<String, Vec<String>> = HashMap::with_capacity(nodes.len());

    for node in nodes {
        spec_map.insert(node.node_id.clone(), node);
        in_degree.insert(node.node_id.clone(), node.parents.len());
        for parent in &node.parents {
            children_map
                .entry(parent.clone())
                .or_default()
                .push(node.node_id.clone());
        }
    }

    let mut queue: VecDeque<String> = in_degree
        .iter()
        .filter_map(|(k, &deg)| if deg == 0 { Some(k.clone()) } else { None })
        .collect();

    let mut derived: HashMap<String, DerivedNodeEnvelope> = HashMap::with_capacity(nodes.len());
    let mut visited_count = 0;

    while let Some(curr_id) = queue.pop_front() {
        visited_count += 1;
        let spec = spec_map.get(&curr_id).ok_or("Missing node specification")?;

        let mut eff_purity = spec.intrinsic_purity;
        let mut eff_ttl = spec.intrinsic_ttl_ms;
        let mut eff_lineage = spec.intrinsic_lineage_keys.clone();

        for parent_id in &spec.parents {
            let parent_env = derived
                .get(parent_id)
                .ok_or("Topological ordering invariant violated")?;

            // 1. Purity degrades monotonically to the most side-effecting ancestor
            eff_purity = eff_purity.max(parent_env.effective_purity);

            // 2. Effective TTL is bounded by the shortest-lived ancestor
            eff_ttl = match (eff_ttl, parent_env.effective_ttl_ms) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            };

            // 3. Lineage set is the full union of all upstream knowledge dependencies
            eff_lineage.extend(parent_env.effective_lineage_keys.iter().cloned());
        }

        derived.insert(
            curr_id.clone(),
            DerivedNodeEnvelope {
                effective_purity: eff_purity,
                effective_ttl_ms: eff_ttl,
                effective_lineage_keys: eff_lineage,
                cacheable: eff_purity != ToolPurity::SideEffectingMutator,
            },
        );

        if let Some(children) = children_map.get(&curr_id) {
            for child_id in children {
                if let Some(deg) = in_degree.get_mut(child_id) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(child_id.clone());
                    }
                }
            }
        }
    }

    if visited_count != nodes.len() {
        return Err("Cycle detected in execution DAG");
    }
    Ok(derived)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transitive_lineage_and_ttl_inheritance() {
        // Node 1: sql_query -> ReadIdempotent with TTL 30s and table:bonds:42 lineage
        let mut lineage1 = BTreeSet::new();
        lineage1.insert("table:bonds:42".to_string());
        let n1 = DagNodeSpec {
            node_id: "node_1".to_string(),
            intrinsic_purity: ToolPurity::ReadIdempotent,
            intrinsic_ttl_ms: Some(30_000),
            intrinsic_lineage_keys: lineage1,
            parents: vec![],
        };

        // Node 2: pure deterministic calculation depending on Node 1
        let n2 = DagNodeSpec {
            node_id: "node_2".to_string(),
            intrinsic_purity: ToolPurity::PureDeterministic,
            intrinsic_ttl_ms: None,
            intrinsic_lineage_keys: BTreeSet::new(),
            parents: vec!["node_1".to_string()],
        };

        let envelopes = compute_dag_transitive_envelopes(&[n1, n2]).unwrap();

        let env2 = &envelopes["node_2"];
        assert_eq!(
            env2.effective_purity,
            ToolPurity::ReadIdempotent,
            "Child must inherit ReadIdempotent from parent"
        );
        assert_eq!(
            env2.effective_ttl_ms,
            Some(30_000),
            "Child must inherit TTL from parent"
        );
        assert!(
            env2.effective_lineage_keys.contains("table:bonds:42"),
            "Child must inherit lineage keys from parent"
        );
        assert!(env2.cacheable);
    }

    #[test]
    fn test_side_effect_mutator_contaminates_downstream() {
        // Node 1: mutator (e.g. transfer_funds)
        let n1 = DagNodeSpec {
            node_id: "transfer".to_string(),
            intrinsic_purity: ToolPurity::SideEffectingMutator,
            intrinsic_ttl_ms: None,
            intrinsic_lineage_keys: BTreeSet::new(),
            parents: vec![],
        };

        // Node 2: pure deterministic log formatter
        let n2 = DagNodeSpec {
            node_id: "format_receipt".to_string(),
            intrinsic_purity: ToolPurity::PureDeterministic,
            intrinsic_ttl_ms: None,
            intrinsic_lineage_keys: BTreeSet::new(),
            parents: vec!["transfer".to_string()],
        };

        let envelopes = compute_dag_transitive_envelopes(&[n1, n2]).unwrap();

        let env2 = &envelopes["format_receipt"];
        assert_eq!(env2.effective_purity, ToolPurity::SideEffectingMutator);
        assert!(
            !env2.cacheable,
            "Downstream node in causal cone of mutator must not be cacheable"
        );
    }

    #[test]
    fn test_cycle_detection_in_dag() {
        let n1 = DagNodeSpec {
            node_id: "a".to_string(),
            intrinsic_purity: ToolPurity::PureDeterministic,
            intrinsic_ttl_ms: None,
            intrinsic_lineage_keys: BTreeSet::new(),
            parents: vec!["b".to_string()],
        };
        let n2 = DagNodeSpec {
            node_id: "b".to_string(),
            intrinsic_purity: ToolPurity::PureDeterministic,
            intrinsic_ttl_ms: None,
            intrinsic_lineage_keys: BTreeSet::new(),
            parents: vec!["a".to_string()],
        };

        let res = compute_dag_transitive_envelopes(&[n1, n2]);
        assert_eq!(res.unwrap_err(), "Cycle detected in execution DAG");
    }
}
