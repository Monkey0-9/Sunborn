//! # Sunborn Execution
//!
//! Merkle DAG execution planner and intermediate sub-graph reuse engine for multi-step
//! AI agent workflows, guaranteeing deterministic lineage and side-effect contamination boundaries.
//!
//! ## Quick Start
//!
//! ```rust
//! use sunborn_execution::ExecutionPlan;
//!
//! let plan = ExecutionPlan::new();
//! assert_eq!(plan.nodes.len(), 0);
//! ```

pub mod merkle;
pub mod model;
pub mod planner;
pub mod signature;
pub mod store;
pub mod transitive_closure;

pub use merkle::{
    MerkleExecutionEngine, MerkleInclusionProof, MerkleNode, MerkleProofStep, SplicedExecutionPlan,
    ToolPurity,
};
pub use model::{
    ExecutionNode, ExecutionPlan, NodeExecutionStatus, PlanEvaluation, ToolCall, ToolDefinition,
    ToolResult,
};
pub use planner::ExecutionPlanner;
pub use signature::ToolSignatureEngine;
pub use store::{ExecutionCacheStore, MemoryExecutionCacheStore};
pub use transitive_closure::{
    compute_dag_transitive_envelopes, DagNodeSpec, DerivedNodeEnvelope,
    ToolPurity as TransitiveToolPurity,
};
