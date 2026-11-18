//! # Graphify-Style Causal Blast-Radius Traversal Engine
//!
//! Rust implementation of Graphify's `affected.py` algorithm.
//! Given one or more mutated knowledge assets (files, tables, functions, or documents),
//! this engine performs a multi-hop BFS traversal over dependency relations to compute
//! the exact blast radius of affected components and dependent cache records with full provenance.

use crate::graphify_graph::GraphifyKnowledgeGraph;
use crate::graphify_model::{GraphifyNodeType, GraphifyRelation};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

/// Represents an individual entity affected by an upstream mutation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AffectedHit {
    /// Affected node identifier
    pub node_id: String,
    /// Distance (hop count) from the root mutation source
    pub depth: usize,
    /// The relationship traversed that caused this invalidation
    pub via_relation: GraphifyRelation,
    /// Source file where the traversal connection occurred, if recorded
    pub via_file: Option<String>,
    /// Line or symbol location where the traversal connection occurred
    pub via_location: Option<String>,
}

/// Comprehensive report detailing the total blast radius of an invalidation event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectedReport {
    /// Initial root assets that were mutated or triggered the invalidation
    pub mutated_roots: Vec<String>,
    /// Full list of affected nodes across all traversal depths
    pub affected_nodes: Vec<AffectedHit>,
    /// Subset of affected nodes that represent cache entries requiring invalidation
    pub affected_cache_keys: Vec<String>,
    /// Maximum traversal depth reached
    pub max_depth_reached: usize,
    /// Total count of unique affected nodes
    pub total_affected: usize,
}

/// Traversal configuration for computing affected blast radius.
#[derive(Debug, Clone)]
pub struct BlastRadiusConfig {
    /// Maximum search depth (default: 16 hops)
    pub max_depth: usize,
    /// Specific relations allowed to propagate invalidation (None = default relations)
    pub allowed_relations: Option<HashSet<GraphifyRelation>>,
    /// Whether to include cache entries in the traversal
    pub include_cache_entries: bool,
}

impl Default for BlastRadiusConfig {
    fn default() -> Self {
        Self {
            max_depth: 16,
            allowed_relations: None,
            include_cache_entries: true,
        }
    }
}

/// Computes the exact blast radius of affected nodes and cache keys given mutated root assets.
pub fn compute_affected(
    graph: &GraphifyKnowledgeGraph,
    mutated_roots: &[&str],
    config: Option<BlastRadiusConfig>,
) -> AffectedReport {
    let cfg = config.unwrap_or_default();
    let mut visited: HashSet<String> = HashSet::new();
    let mut affected_hits: Vec<AffectedHit> = Vec::new();
    let mut affected_cache_keys: Vec<String> = Vec::new();
    let mut max_depth_reached = 0;

    // Queue holds: (node_id, current_depth)
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();

    for &root in mutated_roots {
        let root_str = root.to_string();
        if visited.insert(root_str.clone()) {
            queue.push_back((root_str, 0));
        }
    }

    while let Some((current_id, depth)) = queue.pop_front() {
        if depth >= cfg.max_depth {
            continue;
        }

        let next_depth = depth + 1;

        // 1. Traverse incoming edges: if someone depends on current_id, that someone is affected!
        // E.g. caller -[Calls]-> callee (current) => caller is affected
        // E.g. reader -[ReadsFrom]-> table (current) => reader is affected
        // E.g. cache_entry -[DependsOn]-> doc (current) => cache_entry is affected
        for edge in graph.get_incoming_edges(&current_id) {
            let rel = edge.relation;
            if let Some(allowed) = &cfg.allowed_relations {
                if !allowed.contains(&rel) {
                    continue;
                }
            } else if !rel.is_affected_by_default() {
                continue;
            }

            let dependent_id = edge.source_id.clone();
            if visited.insert(dependent_id.clone()) {
                if next_depth > max_depth_reached {
                    max_depth_reached = next_depth;
                }

                let is_cache_entry = if let Some(node) = graph.get_node(&dependent_id) {
                    node.node_type == GraphifyNodeType::CacheEntry
                } else {
                    dependent_id.starts_with("cache::")
                };

                if is_cache_entry {
                    affected_cache_keys.push(dependent_id.clone());
                }

                affected_hits.push(AffectedHit {
                    node_id: dependent_id.clone(),
                    depth: next_depth,
                    via_relation: rel,
                    via_file: edge.location.clone(),
                    via_location: edge.location.clone(),
                });

                queue.push_back((dependent_id, next_depth));
            }
        }

        // 2. Container containment (forward): if a container (File / Class / Module) changes,
        // all symbols it defines are also directly affected.
        for edge in graph.get_outgoing_edges(&current_id) {
            if edge.relation == GraphifyRelation::Defines {
                let defined_id = edge.target_id.clone();
                if visited.insert(defined_id.clone()) {
                    if next_depth > max_depth_reached {
                        max_depth_reached = next_depth;
                    }

                    affected_hits.push(AffectedHit {
                        node_id: defined_id.clone(),
                        depth: next_depth,
                        via_relation: GraphifyRelation::Defines,
                        via_file: edge.location.clone(),
                        via_location: edge.location.clone(),
                    });

                    queue.push_back((defined_id, next_depth));
                }
            }
        }
    }

    AffectedReport {
        mutated_roots: mutated_roots.iter().map(|s| s.to_string()).collect(),
        total_affected: affected_hits.len(),
        affected_nodes: affected_hits,
        affected_cache_keys,
        max_depth_reached,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_hop_blast_radius_traversal() {
        let graph = GraphifyKnowledgeGraph::new();

        // Topology:
        // doc::pricing.md <- cache::q1 (cache_entry depends on pricing)
        // func::calc_tax -[ReadsFrom]-> table::tax_rates
        // cache::tax_summary -[DependsOn]-> func::calc_tax
        // cache::tax_summary -[Calls]-> func::calc_tax

        graph.insert_node("table::tax_rates", "tax_rates", GraphifyNodeType::Table);
        graph.insert_node("func::calc_tax", "calc_tax()", GraphifyNodeType::Function);
        graph.insert_node(
            "cache::tax_summary",
            "tax_summary",
            GraphifyNodeType::CacheEntry,
        );

        // Edge 1: func::calc_tax reads table::tax_rates
        graph.link(
            "func::calc_tax",
            "table::tax_rates",
            GraphifyRelation::ReadsFrom,
        );
        // Edge 2: cache::tax_summary depends on func::calc_tax
        graph.link(
            "cache::tax_summary",
            "func::calc_tax",
            GraphifyRelation::DependsOn,
        );

        // Invalidate table::tax_rates
        let report = compute_affected(&graph, &["table::tax_rates"], None);

        assert_eq!(report.mutated_roots, vec!["table::tax_rates"]);
        assert_eq!(report.total_affected, 2);
        assert_eq!(report.max_depth_reached, 2);

        // Depth 1: func::calc_tax via ReadsFrom
        let hit1 = report
            .affected_nodes
            .iter()
            .find(|h| h.node_id == "func::calc_tax")
            .unwrap();
        assert_eq!(hit1.depth, 1);
        assert_eq!(hit1.via_relation, GraphifyRelation::ReadsFrom);

        // Depth 2: cache::tax_summary via DependsOn
        let hit2 = report
            .affected_nodes
            .iter()
            .find(|h| h.node_id == "cache::tax_summary")
            .unwrap();
        assert_eq!(hit2.depth, 2);
        assert_eq!(hit2.via_relation, GraphifyRelation::DependsOn);

        assert_eq!(
            report.affected_cache_keys,
            vec!["cache::tax_summary".to_string()]
        );
    }

    #[test]
    fn test_cycle_resilience() {
        let graph = GraphifyKnowledgeGraph::new();

        // Cyclic dependency: A calls B, B calls A
        graph.insert_node("func::a", "a()", GraphifyNodeType::Function);
        graph.insert_node("func::b", "b()", GraphifyNodeType::Function);
        graph.link("func::a", "func::b", GraphifyRelation::Calls);
        graph.link("func::b", "func::a", GraphifyRelation::Calls);

        let report = compute_affected(&graph, &["func::a"], None);
        // Should terminate without loop, visiting b
        assert_eq!(report.total_affected, 1);
        assert_eq!(report.affected_nodes[0].node_id, "func::b");
    }
}
