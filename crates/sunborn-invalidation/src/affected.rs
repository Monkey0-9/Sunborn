//! # Graphify-Style Causal Blast-Radius Traversal Engine
//!
//! Rust implementation of Graphify's `affected.py` algorithm.
//! Given one or more mutated knowledge assets (files, tables, functions, or documents),
//! this engine performs a multi-hop BFS traversal over dependency relations to compute
//! the exact blast radius of affected components and dependent cache records with full provenance.

use crate::graphify_graph::GraphifyKnowledgeGraph;
use crate::graphify_model::{GraphifyNode, GraphifyNodeType, GraphifyRelation};
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
    /// Roots rejected because tenant ownership could not be established.
    /// Non-empty means the caller must treat the invalidation as refused
    /// (fail closed) rather than as a partial success.
    #[serde(default)]
    pub rejected_roots: Vec<String>,
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
    /// Optional tenant ID to scope traversal and affected assets
    pub tenant_id: Option<String>,
}

/// Fail-closed tenant visibility check.
///
/// A node is visible to a tenant only when it exists in the graph **and**
/// carries an explicit `tenant_id` metadata value equal to the requested
/// tenant. Missing nodes and nodes without tenant metadata are rejected: an
/// absent identity is not evidence of ownership.
fn tenant_visible(node: Option<&GraphifyNode>, required_tenant: &str) -> bool {
    match node {
        Some(node) => {
            node.metadata.get("tenant_id").and_then(|v| v.as_str()) == Some(required_tenant)
        }
        None => false,
    }
}

impl Default for BlastRadiusConfig {
    fn default() -> Self {
        Self {
            max_depth: 16,
            allowed_relations: None,
            include_cache_entries: true,
            tenant_id: None,
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

    let mut rejected_roots: Vec<String> = Vec::new();

    for &root in mutated_roots {
        let root_str = root.to_string();
        // Validate root ownership before traversal. When a tenant scope is
        // requested, a root that is absent or whose tenant metadata is missing
        // or conflicting must not seed traversal: silently skipping it would
        // report success while leaving stale entries reachable.
        if let Some(ref required_tenant) = cfg.tenant_id {
            if !tenant_visible(graph.get_node(&root_str).as_ref(), required_tenant) {
                rejected_roots.push(root_str);
                continue;
            }
        }
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
            let dependent_node = graph.get_node(&dependent_id);
            let is_cache_entry = if let Some(ref node) = dependent_node {
                node.node_type == GraphifyNodeType::CacheEntry
            } else {
                dependent_id.starts_with("cache::")
            };

            // Enforce include_cache_entries filter
            if is_cache_entry && !cfg.include_cache_entries {
                continue;
            }

            // Enforce tenant scoping fail-closed: a node with missing tenant
            // metadata is not assumed to belong to the requested tenant.
            if let Some(ref required_tenant) = cfg.tenant_id {
                if !tenant_visible(dependent_node.as_ref(), required_tenant) {
                    continue;
                }
            }

            if visited.insert(dependent_id.clone()) {
                if next_depth > max_depth_reached {
                    max_depth_reached = next_depth;
                }

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
                let defined_node = graph.get_node(&defined_id);
                let is_cache_entry = if let Some(ref node) = defined_node {
                    node.node_type == GraphifyNodeType::CacheEntry
                } else {
                    defined_id.starts_with("cache::")
                };

                // Enforce include_cache_entries filter
                if is_cache_entry && !cfg.include_cache_entries {
                    continue;
                }

                // Enforce tenant scoping fail-closed (see incoming-edge loop).
                if let Some(ref required_tenant) = cfg.tenant_id {
                    if !tenant_visible(defined_node.as_ref(), required_tenant) {
                        continue;
                    }
                }

                if visited.insert(defined_id.clone()) {
                    if next_depth > max_depth_reached {
                        max_depth_reached = next_depth;
                    }

                    if is_cache_entry {
                        affected_cache_keys.push(defined_id.clone());
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
        rejected_roots,
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

    #[test]
    fn test_include_cache_entries_filtering() {
        let graph = GraphifyKnowledgeGraph::new();
        graph.insert_node("doc::spec", "spec.md", GraphifyNodeType::Document);
        graph.insert_node(
            "cache::spec_summary",
            "spec_summary",
            GraphifyNodeType::CacheEntry,
        );
        graph.link(
            "cache::spec_summary",
            "doc::spec",
            GraphifyRelation::DependsOn,
        );

        // When include_cache_entries is true (default)
        let report_with_cache = compute_affected(&graph, &["doc::spec"], None);
        assert_eq!(report_with_cache.total_affected, 1);
        assert_eq!(
            report_with_cache.affected_cache_keys,
            vec!["cache::spec_summary".to_string()]
        );

        // When include_cache_entries is false
        let cfg = BlastRadiusConfig {
            include_cache_entries: false,
            ..Default::default()
        };
        let report_no_cache = compute_affected(&graph, &["doc::spec"], Some(cfg));
        assert_eq!(report_no_cache.total_affected, 0);
        assert!(report_no_cache.affected_cache_keys.is_empty());
    }

    fn tenant_node(id: &str, n: u64, tenant: &str, kind: GraphifyNodeType) -> GraphifyNode {
        let mut node = GraphifyNode::new(id, n, id, kind);
        node.metadata = serde_json::json!({ "tenant_id": tenant });
        node
    }

    #[test]
    fn test_tenant_scoping() {
        let graph = GraphifyKnowledgeGraph::new();

        graph.add_node(tenant_node(
            "doc::shared_alpha",
            1,
            "tenant-alpha",
            GraphifyNodeType::Document,
        ));
        graph.add_node(tenant_node(
            "doc::shared_beta",
            2,
            "tenant-beta",
            GraphifyNodeType::Document,
        ));
        graph.add_node(tenant_node(
            "cache::entry_a",
            3,
            "tenant-alpha",
            GraphifyNodeType::CacheEntry,
        ));
        graph.add_node(tenant_node(
            "cache::entry_b",
            4,
            "tenant-beta",
            GraphifyNodeType::CacheEntry,
        ));

        graph.link(
            "cache::entry_a",
            "doc::shared_alpha",
            GraphifyRelation::DependsOn,
        );
        graph.link(
            "cache::entry_b",
            "doc::shared_beta",
            GraphifyRelation::DependsOn,
        );

        // Scoped to tenant-alpha only
        let cfg_alpha = BlastRadiusConfig {
            tenant_id: Some("tenant-alpha".to_string()),
            ..Default::default()
        };
        let report_alpha = compute_affected(&graph, &["doc::shared_alpha"], Some(cfg_alpha));
        assert_eq!(
            report_alpha.affected_cache_keys,
            vec!["cache::entry_a".to_string()]
        );
        assert!(report_alpha.rejected_roots.is_empty());

        // Scoped to tenant-beta only
        let cfg_beta = BlastRadiusConfig {
            tenant_id: Some("tenant-beta".to_string()),
            ..Default::default()
        };
        let report_beta = compute_affected(&graph, &["doc::shared_beta"], Some(cfg_beta));
        assert_eq!(
            report_beta.affected_cache_keys,
            vec!["cache::entry_b".to_string()]
        );
        assert!(report_beta.rejected_roots.is_empty());
    }

    #[test]
    fn test_tenant_scoping_fails_closed_on_missing_metadata() {
        let graph = GraphifyKnowledgeGraph::new();

        // Root has no tenant metadata: ownership cannot be established.
        let untagged_doc = GraphifyNode::new(
            "doc::untagged",
            1,
            "untagged.md",
            GraphifyNodeType::Document,
        );
        graph.add_node(untagged_doc);

        // Dependent has no tenant metadata: must not be purged under any scope.
        let untagged_cache = GraphifyNode::new(
            "cache::untagged",
            2,
            "untagged",
            GraphifyNodeType::CacheEntry,
        );
        graph.add_node(untagged_cache);
        graph.link(
            "cache::untagged",
            "doc::untagged",
            GraphifyRelation::DependsOn,
        );

        let cfg = BlastRadiusConfig {
            tenant_id: Some("tenant-alpha".to_string()),
            ..Default::default()
        };
        let report = compute_affected(&graph, &["doc::untagged"], Some(cfg));

        assert_eq!(report.rejected_roots, vec!["doc::untagged".to_string()]);
        assert!(report.affected_cache_keys.is_empty());
        assert_eq!(report.total_affected, 0);
    }

    #[test]
    fn test_tenant_scoping_rejects_cross_tenant_root() {
        let graph = GraphifyKnowledgeGraph::new();
        graph.add_node(tenant_node(
            "doc::beta_only",
            1,
            "tenant-beta",
            GraphifyNodeType::Document,
        ));
        graph.add_node(tenant_node(
            "cache::beta_only",
            2,
            "tenant-beta",
            GraphifyNodeType::CacheEntry,
        ));
        graph.link(
            "cache::beta_only",
            "doc::beta_only",
            GraphifyRelation::DependsOn,
        );

        let cfg = BlastRadiusConfig {
            tenant_id: Some("tenant-alpha".to_string()),
            ..Default::default()
        };
        let report = compute_affected(&graph, &["doc::beta_only"], Some(cfg));

        assert_eq!(report.rejected_roots, vec!["doc::beta_only".to_string()]);
        assert!(report.affected_cache_keys.is_empty());
    }
}
