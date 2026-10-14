pub mod decision;
pub mod embedding;
pub mod guardrails;
pub mod microbatch;
pub mod vector_store;

pub use decision::{DecisionConfig, DecisionEngine, DecisionOutcome};
pub use embedding::{cosine_similarity, DeterministicEmbeddingProvider, EmbeddingProvider, Vector};
pub use guardrails::{
    ConformalRiskController, CrossEncoderVerifier, EntailmentScores, StructuralFingerprint,
};
pub use microbatch::MicroBatchedEmbedder;
pub use vector_store::{MemoryVectorStore, ScoredVectorEntry, VectorEntry, VectorStore};
