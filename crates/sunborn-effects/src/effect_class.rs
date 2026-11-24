use serde::{Deserialize, Serialize};

/// Classification of computational effects and side-effect guarantees for execution nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EffectClass {
    /// Pure deterministic transformation without external dependencies or state.
    /// Reusable whenever inputs match.
    Pure,
    /// Read-only idempotent operation bound to versioned authoritative state.
    /// Reusable when input matches and state version is verified fresh.
    ReadOnlyStateful { version: u64 },
    /// Time-sensitive external read (e.g. stock price, current time).
    /// Reusable strictly within its temporal validity window.
    ExternalMutableRead { ttl_secs: u64 },
    /// Mutating operation that is mathematically idempotent under a unique key.
    /// Requires verified idempotency token and state check.
    IdempotentMutation { idempotency_key: String },
    /// Mutating side-effecting operation (e.g. payment execution, database delete).
    /// NEVER blindly replayed or reused.
    NonIdempotentMutation,
    /// Unclassified or unknown external tool. Fails safe by abstaining from reuse.
    #[default]
    UnknownAbstain,
}

impl EffectClass {
    /// Indicates whether the effect class is inherently eligible for memoization.
    pub fn is_inherently_reusable(&self) -> bool {
        matches!(
            self,
            EffectClass::Pure
                | EffectClass::ReadOnlyStateful { .. }
                | EffectClass::ExternalMutableRead { .. }
        )
    }

    /// Indicates whether this node mutates external state.
    pub fn is_mutating(&self) -> bool {
        matches!(
            self,
            EffectClass::IdempotentMutation { .. } | EffectClass::NonIdempotentMutation
        )
    }
}
