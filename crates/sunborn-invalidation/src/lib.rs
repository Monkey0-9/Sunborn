//! # Sunborn Invalidation
//!
//! Transitive dependency graph invalidation, Roaring Bitmap SIMD lineage propagation,
//! and transactional outbox event streams for the Sunborn platform.
//!
//! ## Quick Start
//!
//! ```rust
//! use sunborn_invalidation::CompressedLineageGraph;
//!
//! let graph = CompressedLineageGraph::new();
//! graph.link_document(1001, 42);
//! let affected = graph.invalidate_document(1001);
//! assert!(affected.contains(42));
//! ```

pub mod affected;
pub mod bitmap_lineage;
pub mod c_accelerator;
pub mod cluster_sync;
pub mod clustering;
pub mod epoch;
pub mod graph;
pub mod graph_analytics;
pub mod graph_cache;
pub mod graphify_graph;
pub mod graphify_model;
pub mod orchestrator;
pub mod outbox;
pub mod query;
pub mod watermark;

pub use affected::{compute_affected, AffectedHit, AffectedReport, BlastRadiusConfig};
pub use bitmap_lineage::CompressedLineageGraph;
pub use c_accelerator::{
    c_bitset_intersection_popcount, c_bitset_popcount, c_bitset_union_popcount, c_fast_hash_64,
};
pub use cluster_sync::{
    create_cluster_transport, ClusterBroadcastTransport, ClusterInvalidationMessage,
    DistributedGraphSyncEngine, InMemoryClusterTransport, RedisClusterTransport,
    CLUSTER_INVALIDATION_CHANNEL,
};
pub use clustering::{detect_communities, ClusteringReport, CommunityInfo};
pub use epoch::{EpochFence, EpochLeaseManager, EpochPersistenceBackend, VectorClock};
pub use graph::{
    DependencyGraph, DocumentVersion, InMemoryGraphStore, KnowledgeGraphStore, KnowledgeSource,
    MockNeo4jCypherStore, Neo4jCypherGraphStore,
};
pub use graph_analytics::{
    compute_node_metrics, detect_god_nodes, find_dependency_cycles, GodNodeReport,
};
pub use graph_cache::{deterministic_hash_64, GraphCacheStats, GraphMemoryCache};
pub use graphify_graph::GraphifyKnowledgeGraph;
pub use graphify_model::{
    GraphifyConfidence, GraphifyEdge, GraphifyNode, GraphifyNodeType, GraphifyRelation,
};
pub use orchestrator::{InvalidationOrchestrator, PurgeReport};
pub use outbox::{InvalidationEngine, InvalidationEvent};
pub use query::{
    explain_node, extract_subgraph, find_shortest_path, NodeExplanation, Subgraph, TraversalMode,
};
pub use watermark::{LineageWatermark, ReadView, WatermarkLineageFencer};
