//! # Graphify-Style Graph Analytics & God Node Detection
//!
//! Rust implementation of Graphify's structural analysis (`analyze.py`).
//! Identifies bottleneck "god nodes", computes in-degree and out-degree centrality,
//! measures dependency fan-out for FinOps GDSF volatility discounting, and detects
//! cyclic import/dependency structures.

use crate::graphify_graph::GraphifyKnowledgeGraph;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Analytical report for a specific node representing its connectivity and blast-radius potential.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GodNodeReport {
    /// Node identifier
    pub node_id: String,
    /// Entity label
    pub label: String,
    /// In-degree (number of edges directed into this node)
    pub in_degree: usize,
    /// Out-degree (number of edges originating from this node)
    pub out_degree: usize,
    /// Total degree (in_degree + out_degree)
    pub total_degree: usize,
    /// Whether this node exceeds the god-node threshold
    pub is_god_node: bool,
    /// Square root of FanOut for FinOps GDSF volatility discounting: sqrt(in_degree)
    pub fan_out_discount_factor: f32,
}

/// Identifies all "god nodes" in the knowledge graph whose degree exceeds a given threshold.
/// In caching architectures, mutating a god node triggers a massive invalidation cascade.
pub fn detect_god_nodes(
    graph: &GraphifyKnowledgeGraph,
    min_degree_threshold: usize,
) -> Vec<GodNodeReport> {
    let mut reports = Vec::new();

    for node in graph.all_nodes() {
        let out_degree = graph.get_outgoing_edges(&node.id).len();
        let in_degree = graph.get_incoming_edges(&node.id).len();
        let total_degree = in_degree + out_degree;

        if total_degree >= min_degree_threshold {
            reports.push(GodNodeReport {
                node_id: node.id,
                label: node.label,
                in_degree,
                out_degree,
                total_degree,
                is_god_node: true,
                fan_out_discount_factor: (in_degree as f32).sqrt(),
            });
        }
    }

    // Sort descending by total degree
    reports.sort_by_key(|b| std::cmp::Reverse(b.total_degree));
    reports
}

/// Computes connectivity and fan-out metrics for a single node.
pub fn compute_node_metrics(
    graph: &GraphifyKnowledgeGraph,
    node_id: &str,
) -> Option<GodNodeReport> {
    let node = graph.get_node(node_id)?;
    let out_degree = graph.get_outgoing_edges(node_id).len();
    let in_degree = graph.get_incoming_edges(node_id).len();
    let total_degree = in_degree + out_degree;

    Some(GodNodeReport {
        node_id: node.id,
        label: node.label,
        in_degree,
        out_degree,
        total_degree,
        is_god_node: total_degree >= 10,
        fan_out_discount_factor: (in_degree as f32).sqrt(),
    })
}

/// Detects simple dependency cycles within the knowledge graph using iterative DFS.
/// Returned cycles are canonicalized with the lexicographically smallest node at the start and end.
pub fn find_dependency_cycles(graph: &GraphifyKnowledgeGraph) -> Vec<Vec<String>> {
    let mut raw_cycles = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut path: Vec<String> = Vec::new();
    let mut path_set: HashSet<String> = HashSet::new();

    let mut node_ids = graph.all_node_ids();
    node_ids.sort();

    for node_id in node_ids {
        if !visited.contains(&node_id) {
            dfs_cycle_find(
                graph,
                &node_id,
                &mut visited,
                &mut path,
                &mut path_set,
                &mut raw_cycles,
            );
        }
    }

    // Canonicalize cycles: rotate so that the minimum element is at index 0 and end
    let mut canonical_cycles = Vec::new();
    for mut cycle in raw_cycles {
        if cycle.len() >= 2 && cycle.first() == cycle.last() {
            cycle.pop(); // remove duplicate tail
            if let Some((min_idx, _)) = cycle.iter().enumerate().min_by_key(|(_, x)| *x) {
                cycle.rotate_left(min_idx);
            }
            cycle.push(cycle[0].clone()); // re-add duplicate tail
        }
        if !canonical_cycles.contains(&cycle) {
            canonical_cycles.push(cycle);
        }
    }

    canonical_cycles
}

fn dfs_cycle_find(
    graph: &GraphifyKnowledgeGraph,
    current: &str,
    visited: &mut HashSet<String>,
    path: &mut Vec<String>,
    path_set: &mut HashSet<String>,
    cycles: &mut Vec<Vec<String>>,
) {
    visited.insert(current.to_string());
    path.push(current.to_string());
    path_set.insert(current.to_string());

    for edge in graph.get_outgoing_edges(current) {
        let neighbor = &edge.target_id;
        if path_set.contains(neighbor) {
            // Cycle detected: extract subpath from neighbor to end of path
            if let Some(pos) = path.iter().position(|x| x == neighbor) {
                let mut cycle = path[pos..].to_vec();
                cycle.push(neighbor.clone());
                cycles.push(cycle);
            }
        } else if !visited.contains(neighbor) {
            dfs_cycle_find(graph, neighbor, visited, path, path_set, cycles);
        }
    }

    path.pop();
    path_set.remove(current);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphify_model::{GraphifyNodeType, GraphifyRelation};

    #[test]
    fn test_god_node_detection() {
        let graph = GraphifyKnowledgeGraph::new();

        // Central hub: doc::core_spec.md
        graph.insert_node("doc::core_spec", "core_spec.md", GraphifyNodeType::Document);

        for i in 0..15 {
            let caller = format!("func::worker_{i}");
            graph.insert_node(&caller, &caller, GraphifyNodeType::Function);
            graph.link(&caller, "doc::core_spec", GraphifyRelation::References);
        }

        let god_nodes = detect_god_nodes(&graph, 10);
        assert_eq!(god_nodes.len(), 1);
        assert_eq!(god_nodes[0].node_id, "doc::core_spec");
        assert_eq!(god_nodes[0].in_degree, 15);
        assert!(god_nodes[0].is_god_node);
        assert!((god_nodes[0].fan_out_discount_factor - (15.0f32).sqrt()).abs() < 1e-4);
    }

    #[test]
    fn test_cycle_detection() {
        let graph = GraphifyKnowledgeGraph::new();

        graph.insert_node("a", "Node A", GraphifyNodeType::Function);
        graph.insert_node("b", "Node B", GraphifyNodeType::Function);
        graph.insert_node("c", "Node C", GraphifyNodeType::Function);

        graph.link("a", "b", GraphifyRelation::Calls);
        graph.link("b", "c", GraphifyRelation::Calls);
        graph.link("c", "a", GraphifyRelation::Calls);

        let cycles = find_dependency_cycles(&graph);
        assert!(!cycles.is_empty());
        assert_eq!(cycles[0], vec!["a", "b", "c", "a"]);
    }
}
