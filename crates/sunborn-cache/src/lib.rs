//! # Sunborn Cache
//!
//! Multi-tiered L1 in-memory and L2 DragonflyDB distributed cache engine featuring
//! Greedy Dual-Size Frequency (GDSF) FinOps cost-aware eviction policies and circuit breaking.
//!
//! ## Quick Start
//!
//! ```rust
//! use sunborn_cache::{MemoryExactCache, ExactCacheStore, CacheEntry};
//!
//! # #[tokio::main]
//! # async fn main() {
//! let cache = MemoryExactCache::new();
//! let entry = CacheEntry::new("test-key", "Cached LLM response text", Some(3600));
//!
//! cache.set(entry).await.expect("Set successful");
//! let fetched = cache.get("test-key").await.expect("Get successful");
//! assert!(fetched.is_some());
//! # }
//! ```

pub mod circuit_breaker;
pub mod dragonfly;
pub mod gdsf;
pub mod memory;
pub mod store;
pub mod tiered;

pub use circuit_breaker::{CircuitBreakerCache, CircuitStatus};
pub use dragonfly::DragonflyCacheStore;
pub use gdsf::{CachedObject, FinOpsGdsfCache};
pub use memory::MemoryExactCache;
pub use store::{CacheEntry, ExactCacheStore};
pub use tiered::TieredExactCache;
