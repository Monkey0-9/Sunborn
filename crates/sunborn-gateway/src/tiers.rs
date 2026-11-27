//! Tenant tiers and the defaults applied when a tenant is provisioned.
//!
//! Before this module, a tenant's class lived only in an operator's head: the
//! quota endpoint wrote a row when someone remembered to call it, and the model
//! allowlist, retention window, and support level were set per key by hand. That
//! is a pilot procedure, not a tier. This module turns the tier into one
//! authoritative policy so provisioning and upgrades apply the same values.
//!
//! The policy is deliberately code-defined rather than table-defined: it is a
//! handful of product decisions that must ship with the binary, and a tier table
//! that a replica could read stale would make admission depend on cache age. The
//! *applied* values are persisted on the tenant row, so a later change to the
//! defaults never silently re-scopes an existing tenant.
//!
//! `u64::MAX` is not used for "unlimited": the durable columns are `BIGINT`, so
//! an unlimited tier uses `i64::MAX` and stays within the range the quota SQL
//! binds. `0` is *not* unlimited anywhere in this system — a zero limit denies
//! every request — so it is never used to mean "no cap".

use serde::{Deserialize, Serialize};

/// Models a free tenant may call by default. Kept to the low-cost tier the free
/// envelope in `docs/launch/02_FREE_TIER_ARCHITECTURE.md` describes; operators
/// can override with `SUNBORN_TIER_FREE_MODELS` (comma-separated) without a
/// rebuild, but the default is restrictive so an unconfigured deployment fails
/// closed on cost rather than open.
pub const FREE_DEFAULT_MODELS: [&str; 3] = ["gpt-4o-mini", "claude-3-5-haiku", "gemini-1.5-flash"];

/// The largest limit the durable `BIGINT` columns can hold.
pub const UNLIMITED_REQUESTS: u64 = i64::MAX as u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Pilot,
    Standard,
    Enterprise,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Free => "free",
            Tier::Pilot => "pilot",
            Tier::Standard => "standard",
            Tier::Enterprise => "enterprise",
        }
    }

    /// Case-insensitive parse. Unknown names are rejected rather than defaulted:
    /// a typo in a provisioning call must not silently grant an unrestricted
    /// class.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "free" => Some(Tier::Free),
            "pilot" => Some(Tier::Pilot),
            "standard" => Some(Tier::Standard),
            "enterprise" => Some(Tier::Enterprise),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportLevel {
    Community,
    BusinessHours,
    Dedicated,
}

impl SupportLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            SupportLevel::Community => "community",
            SupportLevel::BusinessHours => "business_hours",
            SupportLevel::Dedicated => "dedicated",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TierPolicy {
    pub tier: Tier,
    pub daily_requests: u64,
    pub monthly_requests: u64,
    pub allowed_models: Vec<String>,
    pub retention_days: u32,
    pub support_level: SupportLevel,
    /// How many tenants one inviter may provision per 30-day window. `None`
    /// means no cap (operator-managed classes).
    pub invitation_cap: Option<u32>,
    /// When true, provisioning without `identity_verified: true` is refused.
    pub requires_verified_identity: bool,
}

impl TierPolicy {
    /// The shipped defaults. These mirror the proposal in
    /// `docs/launch/02_FREE_TIER_ARCHITECTURE.md` §4.2 for the free class; the
    /// paid classes carry limits high enough that the quota is a backstop
    /// against runaway automation, not a product boundary.
    pub fn for_tier(tier: Tier) -> Self {
        match tier {
            Tier::Free => Self {
                tier,
                daily_requests: 50,
                monthly_requests: 1_000,
                allowed_models: FREE_DEFAULT_MODELS
                    .iter()
                    .map(|model| (*model).to_string())
                    .collect(),
                retention_days: 7,
                support_level: SupportLevel::Community,
                invitation_cap: Some(5),
                requires_verified_identity: true,
            },
            Tier::Pilot => Self {
                tier,
                daily_requests: 5_000,
                monthly_requests: 100_000,
                allowed_models: vec!["*".to_string()],
                retention_days: 30,
                support_level: SupportLevel::BusinessHours,
                invitation_cap: None,
                requires_verified_identity: false,
            },
            Tier::Standard => Self {
                tier,
                daily_requests: 100_000,
                monthly_requests: 3_000_000,
                allowed_models: vec!["*".to_string()],
                retention_days: 90,
                support_level: SupportLevel::BusinessHours,
                invitation_cap: None,
                requires_verified_identity: false,
            },
            Tier::Enterprise => Self {
                tier,
                daily_requests: UNLIMITED_REQUESTS,
                monthly_requests: UNLIMITED_REQUESTS,
                allowed_models: vec!["*".to_string()],
                retention_days: 365,
                support_level: SupportLevel::Dedicated,
                invitation_cap: None,
                requires_verified_identity: false,
            },
        }
    }

    /// Resolves a tier name to its policy, applying the free-class model
    /// allowlist override when one is configured.
    ///
    /// An empty override is treated as unset rather than as "no models", because
    /// an empty `SUNBORN_TIER_FREE_MODELS=` in a deployment file is a missing
    /// value, not a decision to block every free request.
    pub fn resolve(raw: &str) -> Result<Self, String> {
        let tier = Tier::parse(raw).ok_or_else(|| {
            format!("unknown tier '{raw}'; expected one of free, pilot, standard, enterprise")
        })?;
        let mut policy = Self::for_tier(tier);
        if tier == Tier::Free {
            if let Ok(value) = std::env::var("SUNBORN_TIER_FREE_MODELS") {
                if let Some(models) = parse_model_list(&value) {
                    policy.allowed_models = models;
                }
            }
        }
        Ok(policy)
    }

    /// True when the policy permits `model`. `*` in the policy means any model.
    pub fn allows_model(&self, model: &str) -> bool {
        self.allowed_models.iter().any(|allowed| allowed == "*")
            || self.allowed_models.iter().any(|allowed| allowed == model)
    }

    pub fn is_unrestricted_models(&self) -> bool {
        self.allowed_models.iter().any(|allowed| allowed == "*")
    }

    /// The quota row this tier enrolls the tenant into.
    pub fn quota_limits(&self, tenant_id: &str) -> sunborn_db::TenantQuotaLimits {
        sunborn_db::TenantQuotaLimits::new(
            tenant_id,
            self.tier.as_str(),
            self.daily_requests,
            self.monthly_requests,
        )
    }
}

/// Parses a comma-separated allowlist, dropping blanks. Returns `None` when no
/// usable model names remain, so the caller keeps its default.
fn parse_model_list(value: &str) -> Option<Vec<String>> {
    let models: Vec<String> = value
        .split(',')
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(str::to_string)
        .collect();
    if models.is_empty() {
        None
    } else {
        Some(models)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_names_round_trip_and_typos_are_rejected() {
        for tier in [Tier::Free, Tier::Pilot, Tier::Standard, Tier::Enterprise] {
            assert_eq!(Tier::parse(tier.as_str()), Some(tier));
            assert_eq!(
                Tier::parse(&format!("  {} ", tier.as_str().to_uppercase())),
                Some(tier)
            );
        }
        assert_eq!(Tier::parse("platinum"), None);
        assert!(TierPolicy::resolve("platinum").is_err());
    }

    #[test]
    fn free_tier_is_the_only_identity_gated_tier() {
        assert!(TierPolicy::for_tier(Tier::Free).requires_verified_identity);
        for tier in [Tier::Pilot, Tier::Standard, Tier::Enterprise] {
            assert!(
                !TierPolicy::for_tier(tier).requires_verified_identity,
                "{tier:?} must not silently require identity verification"
            );
        }
    }

    #[test]
    fn free_tier_denies_a_premium_model_that_a_paid_tier_allows() {
        let free = TierPolicy::for_tier(Tier::Free);
        assert!(free.allows_model("gpt-4o-mini"));
        assert!(!free.allows_model("gpt-4o"));

        let standard = TierPolicy::for_tier(Tier::Standard);
        assert!(standard.allows_model("gpt-4o"));
        assert!(standard.is_unrestricted_models());
    }

    #[test]
    fn unlimited_tiers_stay_inside_the_bigint_budget() {
        let enterprise = TierPolicy::for_tier(Tier::Enterprise);
        assert_eq!(enterprise.daily_requests, i64::MAX as u64);
        assert_eq!(enterprise.monthly_requests, i64::MAX as u64);
        assert!(enterprise.daily_requests <= i64::MAX as u64);
    }

    #[test]
    fn quota_limits_carry_the_tier_name() {
        let limits = TierPolicy::for_tier(Tier::Free).quota_limits("tenant-a");
        assert_eq!(limits.tenant_id, "tenant-a");
        assert_eq!(limits.tier, "free");
        assert_eq!(limits.daily_requests, 50);
        assert_eq!(limits.monthly_requests, 1_000);
    }

    #[test]
    fn blank_model_override_is_ignored_rather_than_blocking_everything() {
        assert_eq!(parse_model_list("   "), None);
        assert_eq!(parse_model_list(",,"), None);
        assert_eq!(
            parse_model_list("gpt-4o-mini, , claude-3-5-haiku"),
            Some(vec![
                "gpt-4o-mini".to_string(),
                "claude-3-5-haiku".to_string()
            ])
        );
    }
}
