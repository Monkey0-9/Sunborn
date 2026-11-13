//! # Sunborn DB
//!
//! Authoritative PostgreSQL persistence, asynchronous write-behind buffers, and audit log persistence
//! for the Sunborn platform.
//!
//! ## Quick Start
//!
//! ```rust
//! use sunborn_db::{MemoryDatabaseStore, DatabaseStore, DurableCacheRecord};
//! use sunborn_core::{TenantId, ApplicationId};
//!
//! # #[tokio::main]
//! # async fn main() {
//! let db = MemoryDatabaseStore::new();
//! let tenant = TenantId("tenant-1".to_string());
//! let app = ApplicationId("app-1".to_string());
//!
//! let record = DurableCacheRecord {
//!     key: "test-key".to_string(),
//!     tenant_id: tenant.0.clone(),
//!     application_id: app.0.clone(),
//!     model: "gpt-4o".to_string(),
//!     policy_version: "v1".to_string(),
//!     hash: "hash-1".to_string(),
//!     prompt_canonical: "Explain quantum computing".to_string(),
//!     content: "Quantum computing uses qubits...".to_string(),
//!     hit_count: 0,
//!     created_at: chrono::Utc::now(),
//!     expires_at: Some(chrono::Utc::now() + chrono::Duration::seconds(3600)),
//!     last_accessed_at: chrono::Utc::now(),
//!     certificate_json: None,
//! };
//!
//! db.insert_cache_entry(record).await.expect("Saved successfully");
//! let fetched = db.get_cache_entry("test-key").await.expect("Fetched successfully");
//! assert!(fetched.is_some());
//! # }
//! ```

pub mod models;
pub mod postgres;
pub mod repository;
pub mod write_behind;

pub use models::{
    AuditRecord, DurableApiCredentialRecord, DurableCacheRecord, NewApiCredentialRecord,
    OutboxRecord, TenantRecord,
};
pub use postgres::PostgresDatabaseStore;
pub use repository::{DatabaseStore, MemoryDatabaseStore};
pub use write_behind::{WriteBehindFlusher, WriteBehindTask};
