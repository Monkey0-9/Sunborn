pub mod conformal;
pub mod decision;
pub mod embedding;
pub mod guardrails;
pub mod microbatch;
pub mod vector_store;

pub use conformal::{StreamingOODDetector, TaskStratifiedConformalCalibrator, TaskStratum};

pub use decision::{
    CascadeStage, DecisionConfig, DecisionEngine, DecisionOutcome, VerificationCascade,
    VerificationCascadeResult,
};
pub use embedding::{
    cosine_similarity, DeterministicEmbeddingProvider, EmbeddingProvider, HttpEmbeddingProvider,
    Vector,
};
pub use guardrails::{
    ConformalRiskController, CrossEncoderVerifier, EntailmentScores, StructuralFingerprint,
};
pub use microbatch::MicroBatchedEmbedder;
pub use vector_store::{
    MemoryVectorStore, QdrantVectorStore, ScoredVectorEntry, VectorEntry, VectorStore,
};
