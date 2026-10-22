pub mod conformal;
pub mod context_digest;
pub mod decision;
pub mod embedding;
pub mod guardrails;
pub mod microbatch;
pub mod vector_store;

pub use conformal::{
    NonExchangeableConformalCalibrator, RollingWindowConformalEngine, ShadowObservation,
    StreamingOODDetector, TaskStratifiedConformalCalibrator, TaskStratum,
};
pub use context_digest::ContextDigest;

pub use decision::{
    CascadeStage, DecisionConfig, DecisionEngine, DecisionOutcome, VerificationCascade,
    VerificationCascadeResult,
};
pub use embedding::{
    cosine_similarity, DeterministicEmbeddingProvider, EmbeddingProvider, HttpEmbeddingProvider,
    LshRandomProjectionEmbeddingProvider, Vector,
};
pub use guardrails::{
    ConformalRiskController, CrossEncoderVerifier, EntailmentScores, StructuralFingerprint,
};
pub use microbatch::MicroBatchedEmbedder;
pub use vector_store::{
    MemoryVectorStore, QdrantVectorStore, ScoredVectorEntry, VectorEntry, VectorStore,
};
