//! # Graphify Multi-Relational In-Memory Knowledge Graph Engine
//!
//! Provides a lock-free, concurrent, directed multi-relational knowledge graph
//! with bidirectional adjacency indexing, content-hash addressing, and numeric ID
//! mapping for SIMD Roaring Treemap interoperability.

use crate::graphify_model::{GraphifyEdge, GraphifyNode, GraphifyNodeType, GraphifyRelation};
use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Concurrent, multi-relational knowledge graph.
pub struct GraphifyKnowledgeGraph {
    /// Node dictionary mapping unique string ID to node data
    nodes: DashMap<String, GraphifyNode>,
    /// Forward adjacency index: source_id -> list of outgoing edges
    outgoing_edges: DashMap<String, Vec<GraphifyEdge>>,
    /// Reverse adjacency index: target_id -> list of incoming edges (used for reverse blast-radius traversal)
    incoming_edges: DashMap<String, Vec<GraphifyEdge>>,
    /// Content hash inverted index: content_hash -> set of node IDs
    hash_to_nodes: DashMap<u64, HashSet<String>>,
    /// Total edge count tracker
    total_edges: AtomicUsize,
    /// Next monotonic numeric ID for bitset mapping
    next_numeric_id: AtomicU64,
}

impl Default for GraphifyKnowledgeGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphifyKnowledgeGraph {
    /// Creates an empty multi-relational knowledge graph.
    pub fn new() -> Self {
        Self {
            nodes: DashMap::new(),
            outgoing_edges: DashMap::new(),
            incoming_edges: DashMap::new(),
            hash_to_nodes: DashMap::new(),
            total_edges: AtomicUsize::new(0),
            next_numeric_id: AtomicU64::new(1),
        }
    }

    /// Allocates a new unique numeric ID for bitset acceleration.
    pub fn next_numeric_id(&self) -> u64 {
        self.next_numeric_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Adds or replaces a node in the graph. Returns the node's numeric ID.
    pub fn add_node(&self, mut node: GraphifyNode) -> u64 {
        if node.numeric_id == 0 {
            node.numeric_id = self.next_numeric_id();
        }
        let num_id = node.numeric_id;
        let node_id = node.id.clone();

        if let Some(hash) = node.content_hash {
            self.hash_to_nodes
                .entry(hash)
                .or_default()
                .insert(node_id.clone());
        }

        self.nodes.insert(node_id, node);
        num_id
    }

    /// Creates and inserts a node with default numeric ID.
    pub fn insert_node(
        &self,
        id: impl Into<String>,
        label: impl Into<String>,
        node_type: GraphifyNodeType,
    ) -> u64 {
        let node = GraphifyNode::new(id, self.next_numeric_id(), label, node_type);
        self.add_node(node)
    }

    /// Retrieves a cloned node by its unique string ID.
    pub fn get_node(&self, id: &str) -> Option<GraphifyNode> {
        self.nodes.get(id).map(|entry| entry.clone())
    }

    /// Retrieves all nodes matching a specific 64-bit content hash.
    pub fn get_nodes_by_hash(&self, hash: u64) -> Vec<GraphifyNode> {
        if let Some(ids) = self.hash_to_nodes.get(&hash) {
            ids.iter().filter_map(|id| self.get_node(id)).collect()
        } else {
            Vec::new()
        }
    }

    /// Adds a typed directional edge between two nodes.
    /// Deduplicates edges with identical (source, target, relation).
    pub fn add_edge(&self, edge: GraphifyEdge) {
        let src = edge.source_id.clone();
        let tgt = edge.target_id.clone();
        let rel = edge.relation;

        // Update forward adjacency
        {
            let mut out = self.outgoing_edges.entry(src.clone()).or_default();
            if let Some(existing) = out
                .iter_mut()
                .find(|e| e.target_id == tgt && e.relation == rel)
            {
                *existing = edge.clone();
            } else {
                out.push(edge.clone());
                self.total_edges.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Update reverse adjacency
        {
            let mut inc = self.incoming_edges.entry(tgt).or_default();
            if let Some(existing) = inc
                .iter_mut()
                .find(|e| e.source_id == src && e.relation == rel)
            {
                *existing = edge;
            } else {
                inc.push(edge);
            }
        }
    }

    /// Convenience helper to link two nodes with an extracted relation.
    pub fn link(
        &self,
        source_id: impl Into<String>,
        target_id: impl Into<String>,
        relation: GraphifyRelation,
    ) {
        let edge = GraphifyEdge::new(source_id, target_id, relation);
        self.add_edge(edge);
    }

    /// Returns all outgoing edges from the given source node.
    pub fn get_outgoing_edges(&self, source_id: &str) -> Vec<GraphifyEdge> {
        self.outgoing_edges
            .get(source_id)
            .map(|list| list.clone())
            .unwrap_or_default()
    }

    /// Returns all incoming edges destined for the given target node.
    pub fn get_incoming_edges(&self, target_id: &str) -> Vec<GraphifyEdge> {
        self.incoming_edges
            .get(target_id)
            .map(|list| list.clone())
            .unwrap_or_default()
    }

    /// Returns the total number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns the total number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.total_edges.load(Ordering::Relaxed)
    }

    /// Returns a list of all node IDs currently present in the graph.
    pub fn all_node_ids(&self) -> Vec<String> {
        self.nodes.iter().map(|entry| entry.key().clone()).collect()
    }

    /// Returns all nodes in the graph.
    pub fn all_nodes(&self) -> Vec<GraphifyNode> {
        self.nodes
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Removes a node and its associated incoming and outgoing edges.
    pub fn remove_node(&self, id: &str) -> Option<GraphifyNode> {
        if let Some((_, node)) = self.nodes.remove(id) {
            if let Some(hash) = node.content_hash {
                if let Some(mut set) = self.hash_to_nodes.get_mut(&hash) {
                    set.remove(id);
                }
            }

            // Remove outgoing edges
            if let Some((_, out)) = self.outgoing_edges.remove(id) {
                self.total_edges.fetch_sub(out.len(), Ordering::Relaxed);
                for edge in out {
                    if let Some(mut inc) = self.incoming_edges.get_mut(&edge.target_id) {
                        inc.retain(|e| !(e.source_id == id && e.relation == edge.relation));
                    }
                }
            }

            // Remove incoming edges
            if let Some((_, inc)) = self.incoming_edges.remove(id) {
                for edge in inc {
                    if let Some(mut out) = self.outgoing_edges.get_mut(&edge.source_id) {
                        let before_len = out.len();
                        out.retain(|e| !(e.target_id == id && e.relation == edge.relation));
                        let removed = before_len - out.len();
                        self.total_edges.fetch_sub(removed, Ordering::Relaxed);
                    }
                }
            }

            Some(node)
        } else {
            None
        }
    }

    /// Clears all nodes and edges from the graph.
    pub fn clear(&self) {
        self.nodes.clear();
        self.outgoing_edges.clear();
        self.incoming_edges.clear();
        self.hash_to_nodes.clear();
        self.total_edges.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_node_and_edge_lifecycle() {
        let graph = GraphifyKnowledgeGraph::new();

        let n1 = graph.insert_node("file::app.rs", "app.rs", GraphifyNodeType::File);
        let n2 = graph.insert_node("func::main", "main()", GraphifyNodeType::Function);
        let n3 = graph.insert_node("table::users", "users", GraphifyNodeType::Table);

        assert_eq!(graph.node_count(), 3);
        assert!(n1 > 0 && n2 > 0 && n3 > 0);

        graph.link("file::app.rs", "func::main", GraphifyRelation::Defines);
        graph.link("func::main", "table::users", GraphifyRelation::ReadsFrom);

        assert_eq!(graph.edge_count(), 2);

        let out = graph.get_outgoing_edges("func::main");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].target_id, "table::users");
        assert_eq!(out[0].relation, GraphifyRelation::ReadsFrom);

        let inc = graph.get_incoming_edges("table::users");
        assert_eq!(inc.len(), 1);
        assert_eq!(inc[0].source_id, "func::main");

        // Edge deduplication test
        graph.link("file::app.rs", "func::main", GraphifyRelation::Defines);
        assert_eq!(graph.edge_count(), 2);

        // Remove node
        let removed = graph.remove_node("func::main");
        assert!(removed.is_some());
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 0);
    }
}
