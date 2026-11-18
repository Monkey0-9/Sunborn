//! # Graphify-Style Community Clustering Engine
//!
//! Rust implementation of Graphify's community detection (`cluster.py`).
//! Partitions the multi-relational knowledge graph into cohesive functional communities
//! using a deterministic Label Propagation Algorithm (LPA).
//!
//! Enables:
//! - Subgraph isolation and community-level cache invalidation (`invalidate_community`).
//! - Community cohesion scoring and modularity analysis.
//! - Context grouping for LLM agent prompt augmentation.

use crate::graphify_graph::GraphifyKnowledgeGraph;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Metrics summarizing a detected functional community.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityInfo {
    /// Community unique numeric identifier
    pub id: u32,
    /// List of node identifiers belonging to this community
    pub member_node_ids: Vec<String>,
    /// Number of internal edges connecting nodes inside this community
    pub internal_edges: usize,
    /// Number of external edges connecting to nodes outside this community
    pub external_edges: usize,
    /// Cohesion score: internal_edges / (internal_edges + external_edges)
    pub cohesion_score: f32,
}

/// Comprehensive report containing all partitioned communities and global clustering stats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusteringReport {
    /// Total number of distinct communities detected
    pub total_communities: usize,
    /// Community metadata indexed by community ID
    pub communities: HashMap<u32, CommunityInfo>,
    /// Ratio of internal edges across the entire network
    pub global_modularity: f32,
}

/// Executes deterministic Label Propagation community clustering over the knowledge graph.
pub fn detect_communities(
    graph: &GraphifyKnowledgeGraph,
    max_iterations: usize,
) -> ClusteringReport {
    let mut node_ids = graph.all_node_ids();
    if node_ids.is_empty() {
        return ClusteringReport {
            total_communities: 0,
            communities: HashMap::new(),
            global_modularity: 0.0,
        };
    }

    // Sort node IDs to guarantee 100% deterministic traversal across runs
    node_ids.sort();

    // 1. Initial state: each node is in its own community
    let mut labels: HashMap<String, u32> = HashMap::with_capacity(node_ids.len());
    for (idx, id) in node_ids.iter().enumerate() {
        labels.insert(id.clone(), idx as u32);
    }

    // 2. Asynchronous Deterministic Label Propagation iterations (in sorted node order)
    for _ in 0..max_iterations {
        let mut changed = false;

        for node_id in &node_ids {
            let mut label_weights: HashMap<u32, f32> = HashMap::new();

            // Consider outgoing neighbors
            for edge in graph.get_outgoing_edges(node_id) {
                if let Some(&target_label) = labels.get(&edge.target_id) {
                    *label_weights.entry(target_label).or_default() += edge.weight;
                }
            }

            // Consider incoming neighbors
            for edge in graph.get_incoming_edges(node_id) {
                if let Some(&source_label) = labels.get(&edge.source_id) {
                    *label_weights.entry(source_label).or_default() += edge.weight;
                }
            }

            if label_weights.is_empty() {
                continue;
            }

            let current_label = *labels.get(node_id).unwrap();
            let mut best_label = current_label;
            let best_weight = *label_weights.get(&current_label).unwrap_or(&0.0);

            let mut sorted_candidates: Vec<(u32, f32)> = label_weights.into_iter().collect();
            sorted_candidates.sort_by(|a, b| {
                b.1.partial_cmp(&a.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.0.cmp(&b.0))
            });

            if let Some((cand_label, cand_weight)) = sorted_candidates.first() {
                if *cand_weight > best_weight
                    || (*cand_weight == best_weight && *cand_label < best_label)
                {
                    best_label = *cand_label;
                }
            }

            if best_label != current_label {
                labels.insert(node_id.clone(), best_label);
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }

    // 3. Compact labels into 0..K IDs
    let mut label_remap: HashMap<u32, u32> = HashMap::new();
    let mut next_id = 0u32;
    for label in labels.values() {
        if !label_remap.contains_key(label) {
            label_remap.insert(*label, next_id);
            next_id += 1;
        }
    }

    // 4. Update nodes in graph and construct report
    let mut community_members: HashMap<u32, Vec<String>> = HashMap::new();
    for (node_id, old_label) in labels {
        let compact_id = *label_remap.get(&old_label).unwrap();
        community_members
            .entry(compact_id)
            .or_default()
            .push(node_id.clone());

        if let Some(mut node) = graph.get_node(&node_id) {
            node.community_id = Some(compact_id);
            graph.add_node(node);
        }
    }

    // 5. Calculate internal/external edge metrics per community
    let mut communities_info = HashMap::new();
    let mut total_internal = 0;
    let mut total_edges_counted = 0;

    for (cid, members) in community_members {
        let member_set: std::collections::HashSet<&String> = members.iter().collect();
        let mut internal = 0;
        let mut external = 0;

        for member in &members {
            for edge in graph.get_outgoing_edges(member) {
                if member_set.contains(&edge.target_id) {
                    internal += 1;
                } else {
                    external += 1;
                }
            }
        }

        total_internal += internal;
        total_edges_counted += internal + external;

        let cohesion = if internal + external > 0 {
            internal as f32 / (internal + external) as f32
        } else {
            1.0
        };

        communities_info.insert(
            cid,
            CommunityInfo {
                id: cid,
                member_node_ids: members,
                internal_edges: internal,
                external_edges: external,
                cohesion_score: cohesion,
            },
        );
    }

    let global_modularity = if total_edges_counted > 0 {
        total_internal as f32 / total_edges_counted as f32
    } else {
        1.0
    };

    ClusteringReport {
        total_communities: communities_info.len(),
        communities: communities_info,
        global_modularity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphify_model::{GraphifyNodeType, GraphifyRelation};

    #[test]
    fn test_community_detection_partitions_clusters() {
        let graph = GraphifyKnowledgeGraph::new();

        // Cluster 1: Auth Module (auth_a, auth_b, auth_c)
        graph.insert_node("auth::a", "auth_a", GraphifyNodeType::Function);
        graph.insert_node("auth::b", "auth_b", GraphifyNodeType::Function);
        graph.insert_node("auth::c", "auth_c", GraphifyNodeType::Function);
        graph.link("auth::a", "auth::b", GraphifyRelation::Calls);
        graph.link("auth::b", "auth::c", GraphifyRelation::Calls);
        graph.link("auth::c", "auth::a", GraphifyRelation::Calls);

        // Cluster 2: Billing Module (bill_x, bill_y)
        graph.insert_node("bill::x", "bill_x", GraphifyNodeType::Function);
        graph.insert_node("bill::y", "bill_y", GraphifyNodeType::Function);
        graph.link("bill::x", "bill::y", GraphifyRelation::Calls);
        graph.link("bill::y", "bill::x", GraphifyRelation::Calls);

        // Run community detection
        let report = detect_communities(&graph, 10);

        assert_eq!(report.total_communities, 2);

        // Assert nodes in same module share same community_id
        let node_a = graph.get_node("auth::a").unwrap();
        let node_b = graph.get_node("auth::b").unwrap();
        let node_x = graph.get_node("bill::x").unwrap();

        assert_eq!(node_a.community_id, node_b.community_id);
        assert_ne!(node_a.community_id, node_x.community_id);
    }
}
