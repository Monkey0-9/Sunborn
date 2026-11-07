use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    pub model: String,
    pub input_cost_per_million: f64,
    pub output_cost_per_million: f64,
}

impl ModelPricing {
    pub fn gpt4o() -> Self {
        Self {
            model: "gpt-4o".into(),
            input_cost_per_million: 2.50,
            output_cost_per_million: 10.00,
        }
    }

    pub fn claude35_sonnet() -> Self {
        Self {
            model: "claude-3-5-sonnet".into(),
            input_cost_per_million: 3.00,
            output_cost_per_million: 15.00,
        }
    }

    pub fn for_model(model: &str) -> Self {
        let m = model.to_lowercase();
        if m.contains("claude") {
            Self::claude35_sonnet()
        } else if m.contains("gpt-4") || m.contains("o1") || m.contains("o3") {
            Self::gpt4o()
        } else {
            Self {
                model: model.to_string(),
                input_cost_per_million: 2.00,
                output_cost_per_million: 8.00,
            }
        }
    }
}

pub struct FinOpsEngine;

impl FinOpsEngine {
    // Cloud RAM storage cost estimate: ~$0.01 per GB per hour -> ~1e-11 per byte per hour
    pub const RAM_COST_PER_BYTE_HOUR: f64 = 0.00000000001;
    // Embedding compute cost (local ONNX/API amortized: ~$0.000005 per call)
    pub const LOCAL_EMBED_COST_PER_CALL: f64 = 0.000005;
    // Cache lookup and indexing overhead: ~$0.000001 per query
    pub const CACHE_LOOKUP_OVERHEAD_PER_CALL: f64 = 0.000001;

    /// Calculate gross token cost avoided by serving from cache
    pub fn calculate_token_cost_avoided(
        pricing: &ModelPricing,
        prompt_tokens: u32,
        completion_tokens: u32,
    ) -> f64 {
        let input_cost = (prompt_tokens as f64 / 1_000_000.0) * pricing.input_cost_per_million;
        let output_cost =
            (completion_tokens as f64 / 1_000_000.0) * pricing.output_cost_per_million;
        input_cost + output_cost
    }

    /// Calculate net savings per hit
    pub fn calculate_net_savings(
        pricing: &ModelPricing,
        prompt_tokens: u32,
        completion_tokens: u32,
    ) -> f64 {
        let gross = Self::calculate_token_cost_avoided(pricing, prompt_tokens, completion_tokens);
        (gross - Self::LOCAL_EMBED_COST_PER_CALL - Self::CACHE_LOOKUP_OVERHEAD_PER_CALL).max(0.0)
    }

    /// Calculate break-even hit rate for an entry of given size and TTL
    pub fn break_even_hit_rate(
        pricing: &ModelPricing,
        prompt_tokens: u32,
        completion_tokens: u32,
        entry_bytes: usize,
        ttl_hours: f64,
    ) -> f64 {
        let gross_per_hit =
            Self::calculate_token_cost_avoided(pricing, prompt_tokens, completion_tokens);
        let storage_cost = entry_bytes as f64 * Self::RAM_COST_PER_BYTE_HOUR * ttl_hours;
        let total_overhead = storage_cost + Self::LOCAL_EMBED_COST_PER_CALL;

        if gross_per_hit <= 0.0 {
            1.0
        } else {
            (total_overhead / gross_per_hit).clamp(0.01, 1.0)
        }
    }

    /// Determines whether admitting an entry is economically justified
    pub fn should_admit_economically(
        pricing: &ModelPricing,
        prompt_tokens: u32,
        completion_tokens: u32,
        entry_bytes: usize,
        expected_hits: u32,
    ) -> bool {
        let gross_total =
            Self::calculate_token_cost_avoided(pricing, prompt_tokens, completion_tokens)
                * (expected_hits as f64);
        let storage_cost = entry_bytes as f64 * Self::RAM_COST_PER_BYTE_HOUR * 24.0; // 24 hour window
        gross_total > (storage_cost + Self::LOCAL_EMBED_COST_PER_CALL)
    }

    /// Pre-registered Net-Utility Optimization Model (Innovation 7):
    /// U = C_origin_saved + V_latency - C_verification - C_storage - C_invalidation - C_recompute - E[loss]
    pub fn calculate_net_utility(params: &NetUtilityParameters) -> f64 {
        let gross_origin_saved = params.c_origin_saved;
        let latency_value = params.v_latency;
        let verification_cost = params.c_verification;
        let storage_cost = params.c_storage;
        let invalidation_cost = params.c_invalidation;
        let recompute_cost = params.c_recompute;
        let expected_loss = params.expected_incorrect_reuse_loss;

        gross_origin_saved + latency_value
            - verification_cost
            - storage_cost
            - invalidation_cost
            - recompute_cost
            - expected_loss
    }
}

/// Parameters for Net-Utility Optimization with hard correctness constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetUtilityParameters {
    pub c_origin_saved: f64,
    pub v_latency: f64,
    pub c_verification: f64,
    pub c_storage: f64,
    pub c_invalidation: f64,
    pub c_recompute: f64,
    pub expected_incorrect_reuse_loss: f64,
}

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

type SpendHistoryMap = Arc<RwLock<HashMap<String, Vec<(Instant, f64)>>>>;
type TenantReservations = HashMap<uuid::Uuid, (Instant, f64)>;
type ReservationMap = Arc<RwLock<HashMap<String, TenantReservations>>>;

/// Spend velocity controller enforcing tenant-level rolling window budget ceilings
/// and preventing unbounded origin expenditure under cascading LLM traffic.
#[derive(Clone)]
pub struct SpendVelocityController {
    max_spend_per_window_usd: f64,
    window_duration: Duration,
    spend_history: SpendHistoryMap,
    tenant_ceilings: Arc<RwLock<HashMap<String, f64>>>,
    reservations: ReservationMap,
}

impl Default for SpendVelocityController {
    fn default() -> Self {
        let default_max = match std::env::var("SUNBORN_MAX_SPEND_PER_WINDOW_USD") {
            Ok(val) => val.parse::<f64>().unwrap_or(100.0),
            Err(_) => 100.0,
        };
        Self::new(default_max, Duration::from_secs(60)) // Default $100/minute ceiling
    }
}

impl SpendVelocityController {
    pub fn new(max_spend_per_window_usd: f64, window_duration: Duration) -> Self {
        Self {
            max_spend_per_window_usd,
            window_duration,
            spend_history: Arc::new(RwLock::new(HashMap::new())),
            tenant_ceilings: Arc::new(RwLock::new(HashMap::new())),
            reservations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Global fallback ceiling per rolling window in USD
    pub fn max_spend_per_window_usd(&self) -> f64 {
        self.max_spend_per_window_usd
    }

    /// Set an explicit custom spend velocity ceiling for a specific tenant
    pub fn set_tenant_ceiling(&self, tenant_id: impl Into<String>, ceiling_usd: f64) {
        let mut ceilings = self.tenant_ceilings.write().unwrap();
        ceilings.insert(tenant_id.into(), ceiling_usd);
    }

    /// Resolves the effective spend ceiling for a tenant (custom override or global default)
    pub fn ceiling_for_tenant(&self, tenant_id: &str) -> f64 {
        let ceilings = self.tenant_ceilings.read().unwrap();
        ceilings
            .get(tenant_id)
            .copied()
            .unwrap_or(self.max_spend_per_window_usd)
    }

    /// Atomic pre-authorization reservation: checks if current spend + active reservations + estimated cost
    /// respects the tenant's ceiling. Returns Ok(reservation_id) on success, or Err((total_committed, ceiling)).
    pub fn reserve_spend(
        &self,
        tenant_id: &str,
        estimated_cost: f64,
    ) -> std::result::Result<uuid::Uuid, (f64, f64)> {
        let now = Instant::now();
        let cutoff = now.checked_sub(self.window_duration).unwrap_or(now);
        let ceiling = self.ceiling_for_tenant(tenant_id);

        let mut res_guard = self.reservations.write().unwrap();
        let mut hist_guard = self.spend_history.write().unwrap();

        // 1. Calculate active spend
        let hist_entries = hist_guard.entry(tenant_id.to_string()).or_default();
        hist_entries.retain(|(ts, _)| *ts >= cutoff);
        let current_spend: f64 = hist_entries.iter().map(|(_, c)| *c).sum();

        // 2. Calculate active in-flight reservations (prune reservations older than 60s)
        let res_map = res_guard.entry(tenant_id.to_string()).or_default();
        let res_cutoff = now.checked_sub(Duration::from_secs(60)).unwrap_or(now);
        res_map.retain(|_, (ts, _)| *ts >= res_cutoff);
        let current_res: f64 = res_map.values().map(|(_, c)| *c).sum();

        let committed_before = current_spend + current_res;
        if committed_before >= ceiling {
            return Err((committed_before, ceiling));
        }

        let total_committed = committed_before + estimated_cost;
        if committed_before > 0.0 && total_committed > ceiling {
            return Err((total_committed, ceiling));
        }

        let res_id = uuid::Uuid::new_v4();
        res_map.insert(res_id, (now, estimated_cost));
        Ok(res_id)
    }

    /// Settles an in-flight reservation with the actual final cost. Releases the reservation
    /// and atomically commits the actual spend to the rolling window history.
    pub fn settle_spend(
        &self,
        tenant_id: &str,
        reservation_id: uuid::Uuid,
        actual_cost: f64,
    ) -> f64 {
        let now = Instant::now();
        let cutoff = now.checked_sub(self.window_duration).unwrap_or(now);

        let mut res_guard = self.reservations.write().unwrap();
        if let Some(res_map) = res_guard.get_mut(tenant_id) {
            res_map.remove(&reservation_id);
        }
        drop(res_guard);

        let mut hist_guard = self.spend_history.write().unwrap();
        let hist_entries = hist_guard.entry(tenant_id.to_string()).or_default();
        hist_entries.retain(|(ts, _)| *ts >= cutoff);
        hist_entries.push((now, actual_cost));
        hist_entries.iter().map(|(_, c)| *c).sum()
    }

    /// Releases an unused reservation without billing spend (e.g. cache hit, aborted request).
    pub fn release_reservation(&self, tenant_id: &str, reservation_id: uuid::Uuid) {
        let mut res_guard = self.reservations.write().unwrap();
        if let Some(res_map) = res_guard.get_mut(tenant_id) {
            res_map.remove(&reservation_id);
        }
    }

    /// Sum of all currently reserved in-flight funds for a tenant
    pub fn current_reserved(&self, tenant_id: &str) -> f64 {
        let now = Instant::now();
        let res_cutoff = now.checked_sub(Duration::from_secs(60)).unwrap_or(now);
        let mut res_guard = self.reservations.write().unwrap();
        if let Some(res_map) = res_guard.get_mut(tenant_id) {
            res_map.retain(|_, (ts, _)| *ts >= res_cutoff);
            res_map.values().map(|(_, c)| *c).sum()
        } else {
            0.0
        }
    }

    /// Total commitment (current window spend + active reservations)
    pub fn current_total_commitment(&self, tenant_id: &str) -> f64 {
        self.current_spend(tenant_id) + self.current_reserved(tenant_id)
    }

    /// Records an expenditure in USD and checks if the tenant has exceeded their spend velocity ceiling.
    /// Returns Ok(current_window_spend) or Err((total_window_spend, max_allowed)).
    pub fn record_and_check(
        &self,
        tenant_id: &str,
        cost_usd: f64,
    ) -> std::result::Result<f64, (f64, f64)> {
        let now = Instant::now();
        let cutoff = now.checked_sub(self.window_duration).unwrap_or(now);
        let ceiling = self.ceiling_for_tenant(tenant_id);

        let mut map = self.spend_history.write().unwrap();
        let entries = map.entry(tenant_id.to_string()).or_default();

        // Prune expired entries outside rolling window
        entries.retain(|(ts, _)| *ts >= cutoff);
        entries.push((now, cost_usd));

        let total_window_spend: f64 = entries.iter().map(|(_, c)| *c).sum();

        if total_window_spend > ceiling {
            Err((total_window_spend, ceiling))
        } else {
            Ok(total_window_spend)
        }
    }

    /// Current spend in the active rolling window
    pub fn current_spend(&self, tenant_id: &str) -> f64 {
        let now = Instant::now();
        let cutoff = now.checked_sub(self.window_duration).unwrap_or(now);

        let map = self.spend_history.read().unwrap();
        if let Some(entries) = map.get(tenant_id) {
            entries
                .iter()
                .filter(|(ts, _)| *ts >= cutoff)
                .map(|(_, c)| *c)
                .sum()
        } else {
            0.0
        }
    }

    pub fn reset_tenant(&self, tenant_id: &str) {
        let mut map = self.spend_history.write().unwrap();
        map.remove(tenant_id);
        let mut res_map = self.reservations.write().unwrap();
        res_map.remove(tenant_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_finops_token_avoided_calculation() {
        let pricing = ModelPricing::gpt4o();
        // 500 prompt tokens, 500 completion tokens
        let cost = FinOpsEngine::calculate_token_cost_avoided(&pricing, 500, 500);
        // (500 / 1e6)*2.5 + (500 / 1e6)*10.0 = 0.00125 + 0.005 = 0.00625
        assert!((cost - 0.00625).abs() < 1e-6);
    }

    #[test]
    fn test_finops_break_even_low_for_llm_queries() {
        let pricing = ModelPricing::gpt4o();
        let break_even = FinOpsEngine::break_even_hit_rate(&pricing, 1000, 800, 4096, 24.0);
        // Break-even hit rate should be very low (< 0.10) for expensive LLMs
        assert!(
            break_even < 0.10,
            "Break-even hit rate for GPT-4o should be well under 10%"
        );
    }

    #[test]
    fn test_finops_net_utility_calculation_with_hard_loss_penalty() {
        let params = NetUtilityParameters {
            c_origin_saved: 0.00625,
            v_latency: 0.00100,
            c_verification: 0.00010,
            c_storage: 0.00001,
            c_invalidation: 0.00005,
            c_recompute: 0.0,
            expected_incorrect_reuse_loss: 0.0,
        };
        let utility = FinOpsEngine::calculate_net_utility(&params);
        assert!(utility > 0.0, "Net utility must be positive when safe");

        // High loss penalty on false hit drives net utility negative
        let penalized_params = NetUtilityParameters {
            expected_incorrect_reuse_loss: 0.10,
            ..params
        };
        let penalized_utility = FinOpsEngine::calculate_net_utility(&penalized_params);
        assert!(
            penalized_utility < 0.0,
            "High incorrect-reuse loss must drive utility negative"
        );
    }

    #[test]
    fn test_spend_velocity_controller_rate_limiting() {
        let controller = SpendVelocityController::new(10.0, Duration::from_secs(60));
        let tenant = "tenant-finops-test";

        // Under limit
        assert!(controller.record_and_check(tenant, 4.0).is_ok());
        assert!(controller.record_and_check(tenant, 5.0).is_ok());
        assert_eq!(controller.current_spend(tenant), 9.0);

        // Exceed limit ($9 + $2 = $11 > $10)
        let res = controller.record_and_check(tenant, 2.0);
        assert!(res.is_err());
        let (total, max) = res.unwrap_err();
        assert_eq!(total, 11.0);
        assert_eq!(max, 10.0);

        // Reset tenant
        controller.reset_tenant(tenant);
        assert_eq!(controller.current_spend(tenant), 0.0);
    }

    #[test]
    fn test_spend_velocity_controller_custom_tenant_ceiling() {
        let controller = SpendVelocityController::new(100.0, Duration::from_secs(60));
        let tenant_vip = "tenant-vip";
        let tenant_standard = "tenant-standard";

        // Set custom low ceiling for standard tenant ($5.0)
        controller.set_tenant_ceiling(tenant_standard, 5.0);

        // Standard tenant exceeds $5.0 ceiling with $6.0 spend
        let res = controller.record_and_check(tenant_standard, 6.0);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), (6.0, 5.0));

        // VIP tenant uses default $100.0 ceiling and succeeds with $50.0 spend
        let res_vip = controller.record_and_check(tenant_vip, 50.0);
        assert!(res_vip.is_ok());
        assert_eq!(res_vip.unwrap(), 50.0);
    }

    #[test]
    fn test_spend_velocity_controller_atomic_reservation_and_settlement() {
        let controller = SpendVelocityController::new(10.0, Duration::from_secs(60));
        let tenant = "tenant-reserve-test";

        // Reserve 4.0 -> success
        let r1 = controller
            .reserve_spend(tenant, 4.0)
            .expect("r1 should succeed");
        assert_eq!(controller.current_reserved(tenant), 4.0);
        assert_eq!(controller.current_spend(tenant), 0.0);
        assert_eq!(controller.current_total_commitment(tenant), 4.0);

        // Reserve another 5.0 -> success (4 + 5 = 9 <= 10)
        let r2 = controller
            .reserve_spend(tenant, 5.0)
            .expect("r2 should succeed");
        assert_eq!(controller.current_reserved(tenant), 9.0);

        // Attempting to reserve 2.0 fails (9 + 2 = 11 > 10)
        let r3_err = controller.reserve_spend(tenant, 2.0);
        assert!(r3_err.is_err());
        let (committed, ceiling) = r3_err.unwrap_err();
        assert_eq!(committed, 11.0);
        assert_eq!(ceiling, 10.0);

        // Release r2 (e.g. cache hit) -> reserved drops back to 4.0
        controller.release_reservation(tenant, r2);
        assert_eq!(controller.current_reserved(tenant), 4.0);

        // Settle r1 with actual cost 3.5
        let total_spend = controller.settle_spend(tenant, r1, 3.5);
        assert_eq!(total_spend, 3.5);
        assert_eq!(controller.current_reserved(tenant), 0.0);
        assert_eq!(controller.current_spend(tenant), 3.5);
        assert_eq!(controller.current_total_commitment(tenant), 3.5);
    }
}
