//! End-to-end integration tests for Graphify-Style Graphical Caching & Blast-Radius Invalidation.

use sunborn_cache::CacheEntry;
use sunborn_invalidation::{
    detect_communities, detect_god_nodes, find_shortest_path, GraphMemoryCache,
    GraphifyKnowledgeGraph, GraphifyNodeType, GraphifyRelation,
};

#[tokio::test]
async fn test_graphify_multi_relational_blast_radius_invalidation() {
    let graph_cache = GraphMemoryCache::new_default();
    let graph = graph_cache.graphify_graph();

    // 1. Build knowledge graph topology:
    // table::users (Table)
    //   <-[ReadsFrom]- func::authenticate (Function)
    //   <-[Calls]- prompt::login_flow (PromptTemplate)
    //   <-[DependsOn]- cache::user_session_42 (CacheEntry)
    //
    // Separate independent branch:
    // doc::terms (Document)
    //   <-[DependsOn]- cache::terms_summary (CacheEntry)

    graph.insert_node("table::users", "users_table", GraphifyNodeType::Table);
    graph.insert_node(
        "func::authenticate",
        "authenticate()",
        GraphifyNodeType::Function,
    );
    graph.insert_node(
        "prompt::login_flow",
        "login_flow",
        GraphifyNodeType::PromptTemplate,
    );
    graph.insert_node(
        "cache::user_session_42",
        "user_session_42",
        GraphifyNodeType::CacheEntry,
    );

    graph.insert_node(
        "doc::terms",
        "terms_of_service.md",
        GraphifyNodeType::Document,
    );
    graph.insert_node(
        "cache::terms_summary",
        "terms_summary",
        GraphifyNodeType::CacheEntry,
    );

    // Link dependency edges
    graph.link(
        "func::authenticate",
        "table::users",
        GraphifyRelation::ReadsFrom,
    );
    graph.link(
        "prompt::login_flow",
        "func::authenticate",
        GraphifyRelation::Calls,
    );
    graph.link(
        "cache::user_session_42",
        "prompt::login_flow",
        GraphifyRelation::DependsOn,
    );

    graph.link(
        "cache::terms_summary",
        "doc::terms",
        GraphifyRelation::DependsOn,
    );

    // Populate L1 cache with both entries
    let entry1 = CacheEntry::new("cache::user_session_42", "token_xyz_encrypted", Some(300));
    let entry2 = CacheEntry::new("cache::terms_summary", "terms_summary_cached", Some(300));

    graph_cache.cache().set(entry1).await.unwrap();
    graph_cache.cache().set(entry2).await.unwrap();

    // Verify both are initially present in L1 cache
    assert!(graph_cache
        .cache()
        .get("cache::user_session_42")
        .await
        .unwrap()
        .is_some());
    assert!(graph_cache
        .cache()
        .get("cache::terms_summary")
        .await
        .unwrap()
        .is_some());

    // 2. Trigger mutation on table::users
    let report = graph_cache
        .invalidate_with_graphify_blast_radius(&["table::users"])
        .await
        .unwrap();

    assert_eq!(report.mutated_roots, vec!["table::users"]);
    assert_eq!(report.max_depth_reached, 3);
    assert_eq!(report.total_affected, 3);

    // Verify affected path provenance
    let hit_func = report
        .affected_nodes
        .iter()
        .find(|h| h.node_id == "func::authenticate")
        .unwrap();
    assert_eq!(hit_func.depth, 1);
    assert_eq!(hit_func.via_relation, GraphifyRelation::ReadsFrom);

    let hit_prompt = report
        .affected_nodes
        .iter()
        .find(|h| h.node_id == "prompt::login_flow")
        .unwrap();
    assert_eq!(hit_prompt.depth, 2);
    assert_eq!(hit_prompt.via_relation, GraphifyRelation::Calls);

    let hit_cache = report
        .affected_nodes
        .iter()
        .find(|h| h.node_id == "cache::user_session_42")
        .unwrap();
    assert_eq!(hit_cache.depth, 3);
    assert_eq!(hit_cache.via_relation, GraphifyRelation::DependsOn);

    // 3. Verify L1 cache state:
    // cache::user_session_42 MUST be purged!
    assert!(graph_cache
        .cache()
        .get("cache::user_session_42")
        .await
        .unwrap()
        .is_none());
    // cache::terms_summary MUST remain intact!
    assert!(graph_cache
        .cache()
        .get("cache::terms_summary")
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn test_graphify_community_clustering_and_invalidation() {
    let graph_cache = GraphMemoryCache::new_default();
    let graph = graph_cache.graphify_graph();

    // Module 1: Billing Community
    graph.insert_node("table::invoices", "invoices", GraphifyNodeType::Table);
    graph.insert_node(
        "func::bill_customer",
        "bill_customer()",
        GraphifyNodeType::Function,
    );
    graph.insert_node(
        "cache::bill_report",
        "bill_report",
        GraphifyNodeType::CacheEntry,
    );
    graph.link(
        "func::bill_customer",
        "table::invoices",
        GraphifyRelation::WritesTo,
    );
    graph.link(
        "cache::bill_report",
        "func::bill_customer",
        GraphifyRelation::DependsOn,
    );

    // Module 2: Analytics Community
    graph.insert_node("table::metrics", "metrics", GraphifyNodeType::Table);
    graph.insert_node("func::aggregate", "aggregate()", GraphifyNodeType::Function);
    graph.insert_node(
        "cache::metric_summary",
        "metric_summary",
        GraphifyNodeType::CacheEntry,
    );
    graph.link(
        "func::aggregate",
        "table::metrics",
        GraphifyRelation::ReadsFrom,
    );
    graph.link(
        "cache::metric_summary",
        "func::aggregate",
        GraphifyRelation::DependsOn,
    );

    // Seed cache
    let bill_entry = CacheEntry::new("cache::bill_report", "invoice_data", Some(300));
    let metric_entry = CacheEntry::new("cache::metric_summary", "metrics_data", Some(300));
    graph_cache.cache().set(bill_entry).await.unwrap();
    graph_cache.cache().set(metric_entry).await.unwrap();

    // Detect communities
    let report = detect_communities(graph, 10);
    assert_eq!(report.total_communities, 2);

    let bill_node = graph.get_node("cache::bill_report").unwrap();
    let metric_node = graph.get_node("cache::metric_summary").unwrap();
    let bill_comm = bill_node.community_id.unwrap();
    let metric_comm = metric_node.community_id.unwrap();
    assert_ne!(bill_comm, metric_comm);

    // Invalidate entire billing community
    let purged_count = graph_cache.invalidate_community(bill_comm).await.unwrap();
    assert_eq!(purged_count, 1);

    // Assert bill_report is purged while metric_summary remains
    assert!(graph_cache
        .cache()
        .get("cache::bill_report")
        .await
        .unwrap()
        .is_none());
    assert!(graph_cache
        .cache()
        .get("cache::metric_summary")
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn test_graphify_lineage_explanation_and_shortest_path() {
    let graph = GraphifyKnowledgeGraph::new();

    graph.insert_node("src::doc", "base_doc.md", GraphifyNodeType::Document);
    graph.insert_node(
        "mid::transformer",
        "transformer()",
        GraphifyNodeType::Function,
    );
    graph.insert_node(
        "cache::final_output",
        "final_output",
        GraphifyNodeType::CacheEntry,
    );

    graph.link("mid::transformer", "src::doc", GraphifyRelation::References);
    graph.link(
        "cache::final_output",
        "mid::transformer",
        GraphifyRelation::DependsOn,
    );

    // Shortest path from doc to final output
    let path = find_shortest_path(&graph, "mid::transformer", "src::doc").unwrap();
    assert_eq!(path.len(), 1);
    assert_eq!(path[0].relation, GraphifyRelation::References);

    // Detect god node on central document
    for i in 0..12 {
        let caller = format!("worker_{i}");
        graph.insert_node(&caller, &caller, GraphifyNodeType::Function);
        graph.link(&caller, "src::doc", GraphifyRelation::References);
    }

    let god_nodes = detect_god_nodes(&graph, 10);
    assert_eq!(god_nodes.len(), 1);
    assert_eq!(god_nodes[0].node_id, "src::doc");
    assert!(god_nodes[0].total_degree >= 12);
}
