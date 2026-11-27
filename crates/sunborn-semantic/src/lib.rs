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
pub mod grounding;
pub mod guardrails;
pub mod microbatch;
pub mod paraphrase_cluster;
pub mod vector_store;

pub use conformal::{
    wilson_score_interval, CalibrationManifest, NonExchangeableConformalCalibrator,
    RollingWindowConformalEngine, ShadowObservation, StratumCalibration, StreamingOODDetector,
    TaskStratifiedConformalCalibrator, TaskStratum,
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
pub use grounding::{
    EvidenceBundle, GroundedClaim, GroundingAttestation, GroundingOutcome, SourceEvidence,
    SourceGroundingVerifier,
};
pub use guardrails::{
    ConformalRiskController, CrossEncoderVerifier, EntailmentScores, StructuralEntailmentScorer,
    StructuralFingerprint,
};
pub use microbatch::MicroBatchedEmbedder;
pub use paraphrase_cluster::{ClusterMember, ParaphraseCluster, ParaphraseClusterManager};
pub use vector_store::{
    embedding_model_parts, MemoryVectorStore, QdrantVectorStore, ScoredVectorEntry, VectorEntry,
    VectorStore,
};
