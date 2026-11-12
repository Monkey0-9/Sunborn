//! # Sunborn Semantic
//!
//! Non-Exchangeable Conformal Risk Control, online semantic drift tracking,
//! vector similarity search, and linguistic semantic guardrails for the Sunborn platform.
//!
//! ## Quick Start
//!
//! ```rust
//! use sunborn_semantic::cosine_similarity;
//!
//! let v1 = vec![1.0, 0.0, 0.0];
//! let v2 = vec![1.0, 0.0, 0.0];
//! let sim = cosine_similarity(&v1, &v2);
//! assert!((sim - 1.0).abs() < 1e-6);
//! ```

pub mod conformal;
pub mod context_digest;
pub mod decision;
pub mod embedding;
pub mod guardrails;
pub mod microbatch;
pub mod vector_store;

pub use conformal::{
    wilson_score_interval, NonExchangeableConformalCalibrator, RollingWindowConformalEngine,
    ShadowObservation, StreamingOODDetector, TaskStratifiedConformalCalibrator, TaskStratum,
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
    ConformalRiskController, CrossEncoderVerifier, EntailmentScores, StructuralEntailmentScorer,
    StructuralFingerprint,
};
pub use microbatch::MicroBatchedEmbedder;
pub use vector_store::{
    MemoryVectorStore, QdrantVectorStore, ScoredVectorEntry, VectorEntry, VectorStore,
};
