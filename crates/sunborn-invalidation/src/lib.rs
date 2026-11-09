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

pub mod bitmap_lineage;
pub mod epoch;
pub mod graph;
pub mod graph_cache;
pub mod outbox;
pub mod watermark;

pub use bitmap_lineage::CompressedLineageGraph;
pub use epoch::{EpochFence, EpochLeaseManager, VectorClock};
pub use graph::{
    DependencyGraph, DocumentVersion, InMemoryGraphStore, KnowledgeGraphStore, KnowledgeSource,
    MockNeo4jCypherStore, Neo4jCypherGraphStore,
};
pub use graph_cache::GraphMemoryCache;
pub use outbox::{InvalidationEngine, InvalidationEvent};
pub use watermark::{LineageWatermark, ReadView, WatermarkLineageFencer};
