pub mod bitmap_lineage;
pub mod epoch;
pub mod graph;
pub mod outbox;
pub mod watermark;

pub use bitmap_lineage::CompressedLineageGraph;
pub use epoch::{EpochFence, EpochLeaseManager, VectorClock};
pub use graph::{
    DependencyGraph, DocumentVersion, InMemoryGraphStore, KnowledgeGraphStore, KnowledgeSource,
    MockNeo4jCypherStore, Neo4jCypherGraphStore,
};
pub use outbox::{InvalidationEngine, InvalidationEvent};
pub use watermark::{LineageWatermark, ReadView, WatermarkLineageFencer};
