//! # Graphify-Style Query & Shortest-Path Lineage Engine
//!
//! Rust implementation of Graphify's path finding, subgraph extraction, and node explanation (`query.py`).
//! Enables AI agents and cache controllers to:
//! - Discover the shortest dependency path between any two assets (`find_shortest_path`).
//! - Extract a budget-bounded contextual subgraph (BFS / DFS) for prompt augmentation.
//! - Explain exactly why a cache entry depends on an upstream asset (`explain_node`).

use crate::graph_analytics::compute_node_metrics;
use crate::graphify_graph::GraphifyKnowledgeGraph;
use crate::graphify_model::{GraphifyEdge, GraphifyNode};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// Traversal strategy for extracting a contextual subgraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraversalMode {
    /// Breadth-First Search: captures immediate surrounding neighborhood
    Bfs,
    /// Depth-First Search: traces deep causal execution chains
    Dfs,
}

/// A localized subgraph bounded by a node budget.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subgraph {
    /// Root node where the traversal began
    pub root_id: String,
    /// All nodes contained in this subgraph
    pub nodes: Vec<GraphifyNode>,
    /// All edges connecting nodes within this subgraph
    pub edges: Vec<GraphifyEdge>,
    /// Total node count
    pub total_nodes: usize,
    /// Total edge count
    pub total_edges: usize,
}

/// Structural explanation of a node's dependencies and community context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeExplanation {
    /// Target node metadata
    pub node: GraphifyNode,
    /// Incoming edges (who depends on this node)
    pub incoming_dependencies: Vec<GraphifyEdge>,
    /// Outgoing edges (what this node depends on)
    pub outgoing_dependencies: Vec<GraphifyEdge>,
    /// Community cluster ID
    pub community_id: Option<u32>,
    /// Whether this node is classified as a god node / bottleneck
    pub is_god_node: bool,
    /// Human-readable explanation string
    pub lineage_summary: String,
}

/// Finds the shortest directed path between two nodes using Breadth-First Search.
pub fn find_shortest_path(
    graph: &GraphifyKnowledgeGraph,
    from_id: &str,
    to_id: &str,
) -> Option<Vec<GraphifyEdge>> {
    if from_id == to_id {
        return Some(Vec::new());
    }

    if graph.get_node(from_id).is_none() || graph.get_node(to_id).is_none() {
        return None;
    }

    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    // Maps child_id -> (parent_id, edge_traversed)
    let mut parent_map: HashMap<String, (String, GraphifyEdge)> = HashMap::new();

    visited.insert(from_id.to_string());
    queue.push_back(from_id.to_string());

    let mut found = false;

    while let Some(current) = queue.pop_front() {
        if current == to_id {
            found = true;
            break;
        }

        for edge in graph.get_outgoing_edges(&current) {
            let next_node = &edge.target_id;
            if visited.insert(next_node.clone()) {
                parent_map.insert(next_node.clone(), (current.clone(), edge.clone()));
                if next_node == to_id {
                    found = true;
                    break;
                }
                queue.push_back(next_node.clone());
            }
        }

        if found {
            break;
        }
    }

    if !found {
        return None;
    }

    // Reconstruct path from to_id back to from_id
    let mut path = Vec::new();
    let mut curr = to_id.to_string();

    while let Some((parent, edge)) = parent_map.get(&curr) {
        path.push(edge.clone());
        curr = parent.clone();
        if curr == from_id {
            break;
        }
    }

    path.reverse();
    Some(path)
}

/// Extracts a localized subgraph starting from root_id, capped at budget_nodes.
pub fn extract_subgraph(
    graph: &GraphifyKnowledgeGraph,
    start_id: &str,
    budget_nodes: usize,
    mode: TraversalMode,
) -> Subgraph {
    let mut selected_node_ids = Vec::new();
    let mut visited = HashSet::new();

    if let Some(root_node) = graph.get_node(start_id) {
        visited.insert(start_id.to_string());
        selected_node_ids.push(root_node.id.clone());

        match mode {
            TraversalMode::Bfs => {
                let mut queue = VecDeque::new();
                queue.push_back(start_id.to_string());

                while let Some(curr) = queue.pop_front() {
                    if selected_node_ids.len() >= budget_nodes {
                        break;
                    }

                    for edge in graph.get_outgoing_edges(&curr) {
                        if selected_node_ids.len() >= budget_nodes {
                            break;
                        }
                        if visited.insert(edge.target_id.clone()) {
                            selected_node_ids.push(edge.target_id.clone());
                            queue.push_back(edge.target_id);
                        }
                    }
                }
            }
            TraversalMode::Dfs => {
                let mut stack = Vec::new();
                stack.push(start_id.to_string());

                while let Some(curr) = stack.pop() {
                    if selected_node_ids.len() >= budget_nodes {
                        break;
                    }

                    for edge in graph.get_outgoing_edges(&curr) {
                        if selected_node_ids.len() >= budget_nodes {
                            break;
                        }
                        if visited.insert(edge.target_id.clone()) {
                            selected_node_ids.push(edge.target_id.clone());
                            stack.push(edge.target_id);
                        }
                    }
                }
            }
        }
    }

    let node_id_set: HashSet<&String> = selected_node_ids.iter().collect();
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for id in &selected_node_ids {
        if let Some(node) = graph.get_node(id) {
            nodes.push(node);
        }
        for edge in graph.get_outgoing_edges(id) {
            if node_id_set.contains(&edge.target_id) {
                edges.push(edge);
            }
        }
    }

    Subgraph {
        root_id: start_id.to_string(),
        total_nodes: nodes.len(),
        total_edges: edges.len(),
        nodes,
        edges,
    }
}

/// Generates an explanation report of a node's incoming and outgoing lineage.
pub fn explain_node(graph: &GraphifyKnowledgeGraph, node_id: &str) -> Option<NodeExplanation> {
    let node = graph.get_node(node_id)?;
    let incoming = graph.get_incoming_edges(node_id);
    let outgoing = graph.get_outgoing_edges(node_id);
    let metrics = compute_node_metrics(graph, node_id);
    let is_god_node = metrics.as_ref().map(|m| m.is_god_node).unwrap_or(false);

    let summary = format!(
        "Node '{}' [{:?}]: has {} incoming dependencies and {} outgoing references (Community: {:?}, GodNode: {})",
        node.label,
        node.node_type,
        incoming.len(),
        outgoing.len(),
        node.community_id,
        is_god_node
    );

    Some(NodeExplanation {
        node: node.clone(),
        incoming_dependencies: incoming,
        outgoing_dependencies: outgoing,
        community_id: node.community_id,
        is_god_node,
        lineage_summary: summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphify_model::{GraphifyNodeType, GraphifyRelation};

    #[test]
    fn test_shortest_path_bfs() {
        let graph = GraphifyKnowledgeGraph::new();

        // A -> B -> C -> D
        // A -> X -> D (shorter: 2 edges)
        graph.insert_node("A", "Node A", GraphifyNodeType::Function);
        graph.insert_node("B", "Node B", GraphifyNodeType::Function);
        graph.insert_node("C", "Node C", GraphifyNodeType::Function);
        graph.insert_node("D", "Node D", GraphifyNodeType::Table);
        graph.insert_node("X", "Node X", GraphifyNodeType::Function);

        graph.link("A", "B", GraphifyRelation::Calls);
        graph.link("B", "C", GraphifyRelation::Calls);
        graph.link("C", "D", GraphifyRelation::ReadsFrom);

        graph.link("A", "X", GraphifyRelation::Calls);
        graph.link("X", "D", GraphifyRelation::ReadsFrom);

        let path = find_shortest_path(&graph, "A", "D").expect("path should exist");
        assert_eq!(path.len(), 2);
        assert_eq!(path[0].source_id, "A");
        assert_eq!(path[0].target_id, "X");
        assert_eq!(path[1].source_id, "X");
        assert_eq!(path[1].target_id, "D");
    }

    #[test]
    fn test_subgraph_budget_bounding() {
        let graph = GraphifyKnowledgeGraph::new();

        graph.insert_node("root", "Root", GraphifyNodeType::Module);
        for i in 0..10 {
            let child = format!("child_{i}");
            graph.insert_node(&child, &child, GraphifyNodeType::Function);
            graph.link("root", &child, GraphifyRelation::Defines);
        }

        let subgraph = extract_subgraph(&graph, "root", 4, TraversalMode::Bfs);
        assert_eq!(subgraph.nodes.len(), 4);
    }
}
