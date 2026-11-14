//! # Sunborn Core
//!
//! Foundational domain models, FinOps economic utility scoring, tenancy identifiers,
//! error taxonomies, and configuration primitives for the Sunborn platform.
//!
//! ## Quick Start
//!
//! ```rust
//! use sunborn_core::{TenantId, ApplicationId, RequestEnvelope};
//!
//! let req = RequestEnvelope::new("tenant-alpha", "app-1", "gpt-4o", "Explain quantum computing");
//! assert_eq!(req.tenant_id, TenantId("tenant-alpha".to_string()));
//! assert_eq!(req.application_id, ApplicationId("app-1".to_string()));
//! assert_eq!(req.model, "gpt-4o");
//! ```

pub mod certificate;
pub mod cluster;
pub mod config;
pub mod error;
pub mod finops;
pub mod model;
pub mod telemetry;

pub use certificate::{
    ReuseCertificate, SafetyEvidenceRecord, SignatureAlgorithm, SourceVersionSnapshot,
    ValidityWindow,
};
pub use cluster::{CausalityRelation, ClusterSyncDigest, VectorClock};
pub use config::{allow_test_fixtures, is_production_mode, AppConfig};
pub use error::{ErrorCode, Result, SunbornError};
pub use finops::{FinOpsEngine, ModelPricing, SpendVelocityController};
pub use model::{
    ApplicationId, CachePath, CorrectnessTier, DecisionAction, DecisionRecord, RequestEnvelope,
    RequestId, ResponseEnvelope, TenantId, ToolDefinition,
};
pub use telemetry::init_tracing;
