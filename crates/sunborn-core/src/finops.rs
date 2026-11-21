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

    pub fn sarvam_2b() -> Self {
        Self {
            model: "sarvam-2b".into(),
            input_cost_per_million: 0.15,
            output_cost_per_million: 0.30,
        }
    }

    pub fn krutrim_v1() -> Self {
        Self {
            model: "krutrim-v1".into(),
            input_cost_per_million: 0.20,
            output_cost_per_million: 0.50,
        }
    }

    pub fn for_model(model: &str) -> Self {
        let m = model.to_lowercase();
        if m.contains("sarvam") {
            Self::sarvam_2b()
        } else if m.contains("krutrim") {
            Self::krutrim_v1()
        } else if m.contains("claude") {
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

/// Micro-units representing 1/1,000,000th of 1 USD (1 USD = 1,000,000 micro-units).
/// Eliminates floating-point rounding errors and non-associative accumulation in financial transactions.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct MicroUnits(pub i64);

impl MicroUnits {
    pub const ZERO: Self = Self(0);
    pub const PER_USD: i64 = 1_000_000;

    #[inline]
    pub fn try_from_usd(usd: f64) -> Option<Self> {
        const MAX_USD: f64 = 100_000.0;
        if !usd.is_finite() || !(0.0..=MAX_USD).contains(&usd) {
            return None;
        }
        let units = (usd * Self::PER_USD as f64).round() as i64;
        Some(Self(units))
    }

    #[inline]
    pub fn from_usd(usd: f64) -> Self {
        Self::try_from_usd(usd).unwrap_or(Self::ZERO)
    }

    #[inline]
    pub fn to_usd(self) -> f64 {
        self.0 as f64 / Self::PER_USD as f64
    }
}

impl std::fmt::Display for MicroUnits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} uUSD (${:.6})", self.0, self.to_usd())
    }
}

/// Sovereign Indic micro-paise representing 1/1,000,000th of 1 INR
/// (1 INR = 100 paise = 1,000,000 micro-paise).
/// Ensures zero-loss precision for sub-penny Indic token billing under high concurrency.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct MicroPaise(pub i64);

impl MicroPaise {
    pub const ZERO: Self = Self(0);
    pub const PER_INR: i64 = 1_000_000;
    pub const DEFAULT_INR_PER_USD: f64 = 84.50; // Reference sovereign peg

    #[inline]
    pub fn from_inr(inr: f64) -> Self {
        if !inr.is_finite() || inr < 0.0 {
            return Self::ZERO;
        }
        let units = (inr * Self::PER_INR as f64).round() as i64;
        Self(units.max(0))
    }

    #[inline]
    pub fn to_inr(self) -> f64 {
        self.0 as f64 / Self::PER_INR as f64
    }

    #[inline]
    pub fn from_usd(usd: f64, inr_per_usd: f64) -> Self {
        let rate = if inr_per_usd.is_finite() && inr_per_usd > 0.0 {
            inr_per_usd
        } else {
            Self::DEFAULT_INR_PER_USD
        };
        Self::from_inr(usd * rate)
    }

    #[inline]
    pub fn to_usd(self, inr_per_usd: f64) -> f64 {
        let rate = if inr_per_usd.is_finite() && inr_per_usd > 0.0 {
            inr_per_usd
        } else {
            Self::DEFAULT_INR_PER_USD
        };
        self.to_inr() / rate
    }

    #[inline]
    pub fn format_indian_currency(self) -> String {
        format_inr_currency(self.to_inr())
    }
}

impl std::fmt::Display for MicroPaise {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} uINR ({})", self.0, self.format_indian_currency())
    }
}

/// Format an amount using Indian currency notation (Lakhs and Crores grouping, e.g. ₹1,50,000.00).
pub fn format_inr_currency(amount: f64) -> String {
    if !amount.is_finite() {
        return "₹0.00".to_string();
    }
    let is_negative = amount < 0.0;
    let abs_amount = amount.abs();
    let integer_part = abs_amount.floor() as u64;
    let fractional_part = ((abs_amount - integer_part as f64) * 100.0).round() as u64;

    let int_str = integer_part.to_string();
    let formatted_int = if int_str.len() <= 3 {
        int_str
    } else {
        let (head, tail) = int_str.split_at(int_str.len() - 3);
        let mut parts = Vec::new();
        let mut remaining = head;
        while remaining.len() > 2 {
            let split_idx = remaining.len() - 2;
            parts.push(&remaining[split_idx..]);
            remaining = &remaining[..split_idx];
        }
        if !remaining.is_empty() {
            parts.push(remaining);
        }
        parts.reverse();
        format!("{},{}", parts.join(","), tail)
    };

    let sign = if is_negative { "-" } else { "" };
    format!("{}₹{}.{:02}", sign, formatted_int, fractional_part.min(99))
}

/// Compact Indian denominational formatting (e.g. ₹1.50 Lakh, ₹2.40 Cr, ₹45.00k).
pub fn format_lakhs_crores(amount: f64) -> String {
    if !amount.is_finite() || amount <= 0.0 {
        return "₹0.00".to_string();
    }
    if amount >= 10_000_000.0 {
        format!("₹{:.2} Cr", amount / 10_000_000.0)
    } else if amount >= 100_000.0 {
        format!("₹{:.2} Lakh", amount / 100_000.0)
    } else if amount >= 1_000.0 {
        format!("₹{:.2}k", amount / 1_000.0)
    } else {
        format!("₹{:.2}", amount)
    }
}

/// India GST Statutory Tax Breakdown under SAC 998313 (IT / Cloud Infrastructure Services)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GstBreakdown {
    pub taxable_amount_inr: f64,
    pub cgst_rate_percent: f64,
    pub cgst_amount_inr: f64,
    pub sgst_rate_percent: f64,
    pub sgst_amount_inr: f64,
    pub igst_rate_percent: f64,
    pub igst_amount_inr: f64,
    pub total_tax_inr: f64,
    pub total_amount_inr: f64,
    pub sac_code: String,
    pub is_inter_state: bool,
    pub gstin: Option<String>,
}

impl GstBreakdown {
    pub const SAC_CODE: &'static str = "998313";
    pub const TOTAL_GST_RATE: f64 = 18.0;
    pub const CGST_RATE: f64 = 9.0;
    pub const SGST_RATE: f64 = 9.0;
    pub const IGST_RATE: f64 = 18.0;

    pub fn calculate(taxable_inr: f64, is_inter_state: bool, gstin: Option<&str>) -> Self {
        let taxable = if taxable_inr.is_finite() && taxable_inr > 0.0 {
            taxable_inr
        } else {
            0.0
        };

        let (cgst_r, cgst_a, sgst_r, sgst_a, igst_r, igst_a) = if is_inter_state {
            let igst = (taxable * Self::IGST_RATE / 100.0 * 100.0).round() / 100.0;
            (0.0, 0.0, 0.0, 0.0, Self::IGST_RATE, igst)
        } else {
            let cgst = (taxable * Self::CGST_RATE / 100.0 * 100.0).round() / 100.0;
            let sgst = (taxable * Self::SGST_RATE / 100.0 * 100.0).round() / 100.0;
            (Self::CGST_RATE, cgst, Self::SGST_RATE, sgst, 0.0, 0.0)
        };

        let total_tax = cgst_a + sgst_a + igst_a;
        let total_amount = taxable + total_tax;

        Self {
            taxable_amount_inr: taxable,
            cgst_rate_percent: cgst_r,
            cgst_amount_inr: cgst_a,
            sgst_rate_percent: sgst_r,
            sgst_amount_inr: sgst_a,
            igst_rate_percent: igst_r,
            igst_amount_inr: igst_a,
            total_tax_inr: total_tax,
            total_amount_inr: total_amount,
            sac_code: Self::SAC_CODE.to_string(),
            is_inter_state,
            gstin: gstin.map(|s| s.trim().to_uppercase()),
        }
    }
}

/// Validate standard 15-character Goods and Services Tax Identification Number (GSTIN)
pub fn validate_gstin(gstin: &str) -> bool {
    let s = gstin.trim();
    if s.len() != 15 {
        return false;
    }
    let bytes = s.as_bytes();
    // 1-2: State code (01-38)
    if !bytes[0].is_ascii_digit() || !bytes[1].is_ascii_digit() {
        return false;
    }
    let state_code: u32 = match s[0..2].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    if !(1..=38).contains(&state_code) {
        return false;
    }
    // 3-7: PAN alphabetic (5 uppercase letters)
    if !bytes[2..7].iter().all(|b| b.is_ascii_uppercase()) {
        return false;
    }
    // 8-11: PAN numeric (4 digits)
    if !bytes[7..11].iter().all(|b| b.is_ascii_digit()) {
        return false;
    }
    // 12: PAN entity letter
    if !bytes[11].is_ascii_uppercase() {
        return false;
    }
    // 13: 1-9 or A-Z (entity number)
    if !bytes[12].is_ascii_alphanumeric() {
        return false;
    }
    // 14: Default 'Z'
    if bytes[13] != b'Z' {
        return false;
    }
    // 15: Checksum char (alphanumeric)
    bytes[14].is_ascii_alphanumeric()
}

use crate::sync::{read_lock, write_lock};
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

    /// Length of the rolling spend window.
    pub fn window_duration(&self) -> Duration {
        self.window_duration
    }

    /// Set an explicit custom spend velocity ceiling for a specific tenant
    pub fn set_tenant_ceiling(&self, tenant_id: impl Into<String>, ceiling_usd: f64) {
        let mut ceilings = write_lock(&self.tenant_ceilings);
        ceilings.insert(tenant_id.into(), ceiling_usd);
    }

    /// Resolves the effective spend ceiling for a tenant (custom override or global default)
    pub fn ceiling_for_tenant(&self, tenant_id: &str) -> f64 {
        let ceilings = read_lock(&self.tenant_ceilings);
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
        let ceiling = self.ceiling_for_tenant(tenant_id);

        // Numerical sanitization: reject NaN, infinite, negative, or unrealistic costs
        if !estimated_cost.is_finite() || !(0.0..=100_000.0).contains(&estimated_cost) {
            return Err((0.0, ceiling));
        }

        let now = Instant::now();
        let cutoff = now.checked_sub(self.window_duration).unwrap_or(now);

        let mut res_guard = write_lock(&self.reservations);
        let mut hist_guard = write_lock(&self.spend_history);

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

        let mut res_guard = write_lock(&self.reservations);
        if let Some(res_map) = res_guard.get_mut(tenant_id) {
            res_map.remove(&reservation_id);
        }
        drop(res_guard);

        if !actual_cost.is_finite() || actual_cost < 0.0 {
            tracing::warn!(
                tenant = %tenant_id,
                cost = %actual_cost,
                "Rejected invalid non-finite or negative financial spend settlement"
            );
            let mut hist_guard = write_lock(&self.spend_history);
            let hist_entries = hist_guard.entry(tenant_id.to_string()).or_default();
            hist_entries.retain(|(ts, _)| *ts >= cutoff);
            return hist_entries.iter().map(|(_, c)| *c).sum();
        }

        let valid_cost = actual_cost.min(100_000.0);

        let mut hist_guard = write_lock(&self.spend_history);
        let hist_entries = hist_guard.entry(tenant_id.to_string()).or_default();
        hist_entries.retain(|(ts, _)| *ts >= cutoff);
        hist_entries.push((now, valid_cost));
        hist_entries.iter().map(|(_, c)| *c).sum()
    }

    /// Releases an unused reservation without billing spend (e.g. cache hit, aborted request).
    pub fn release_reservation(&self, tenant_id: &str, reservation_id: uuid::Uuid) {
        let mut res_guard = write_lock(&self.reservations);
        if let Some(res_map) = res_guard.get_mut(tenant_id) {
            res_map.remove(&reservation_id);
        }
    }

    /// Sum of all currently reserved in-flight funds for a tenant
    pub fn current_reserved(&self, tenant_id: &str) -> f64 {
        let now = Instant::now();
        let res_cutoff = now.checked_sub(Duration::from_secs(60)).unwrap_or(now);
        let mut res_guard = write_lock(&self.reservations);
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
        let ceiling = self.ceiling_for_tenant(tenant_id);

        let valid_cost = if cost_usd.is_finite() && cost_usd >= 0.0 {
            cost_usd.min(100_000.0)
        } else {
            return Err((0.0, ceiling));
        };

        let now = Instant::now();
        let cutoff = now.checked_sub(self.window_duration).unwrap_or(now);

        let mut map = write_lock(&self.spend_history);
        let entries = map.entry(tenant_id.to_string()).or_default();

        // Prune expired entries outside rolling window
        entries.retain(|(ts, _)| *ts >= cutoff);
        entries.push((now, valid_cost));

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

        let map = read_lock(&self.spend_history);
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
        let mut map = write_lock(&self.spend_history);
        map.remove(tenant_id);
        let mut res_map = write_lock(&self.reservations);
        res_map.remove(tenant_id);
    }

    /// Set custom spend ceiling in micro-units
    pub fn set_tenant_ceiling_micro_units(
        &self,
        tenant_id: impl Into<String>,
        ceiling: MicroUnits,
    ) {
        self.set_tenant_ceiling(tenant_id, ceiling.to_usd());
    }

    /// Resolves effective spend ceiling in micro-units
    pub fn ceiling_for_tenant_micro_units(&self, tenant_id: &str) -> MicroUnits {
        MicroUnits::from_usd(self.ceiling_for_tenant(tenant_id))
    }

    /// Reserve spend using strongly-typed micro-units
    pub fn reserve_spend_micro_units(
        &self,
        tenant_id: &str,
        estimated_cost: MicroUnits,
    ) -> std::result::Result<uuid::Uuid, (MicroUnits, MicroUnits)> {
        self.reserve_spend(tenant_id, estimated_cost.to_usd())
            .map_err(|(committed, ceiling)| {
                (
                    MicroUnits::from_usd(committed),
                    MicroUnits::from_usd(ceiling),
                )
            })
    }

    /// Settle spend using strongly-typed micro-units
    pub fn settle_spend_micro_units(
        &self,
        tenant_id: &str,
        reservation_id: uuid::Uuid,
        actual_cost: MicroUnits,
    ) -> MicroUnits {
        if actual_cost.0 < 0 {
            tracing::warn!(tenant = %tenant_id, cost = %actual_cost.0, "Rejected negative micro-units settlement");
            return self.current_spend_micro_units(tenant_id);
        }
        let total = self.settle_spend(tenant_id, reservation_id, actual_cost.to_usd());
        MicroUnits::from_usd(total)
    }

    /// Current spend in micro-units
    pub fn current_spend_micro_units(&self, tenant_id: &str) -> MicroUnits {
        MicroUnits::from_usd(self.current_spend(tenant_id))
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

    #[test]
    fn test_spend_velocity_micro_units_accounting() {
        let controller = SpendVelocityController::new(5.0, Duration::from_secs(60));
        let tenant = "tenant-micro-units";

        // Ceiling $5.0 = 5,000,000 micro-units
        assert_eq!(
            controller.ceiling_for_tenant_micro_units(tenant),
            MicroUnits(5_000_000)
        );

        // Set custom ceiling $12.50 = 12,500,000 micro-units
        controller.set_tenant_ceiling_micro_units(tenant, MicroUnits(12_500_000));
        assert_eq!(
            controller.ceiling_for_tenant_micro_units(tenant),
            MicroUnits(12_500_000)
        );

        // Reserve 4,000,000 micro-units ($4.00)
        let r1 = controller
            .reserve_spend_micro_units(tenant, MicroUnits(4_000_000))
            .expect("r1 should succeed");

        // Settle with 3,750,000 micro-units ($3.75)
        let settled = controller.settle_spend_micro_units(tenant, r1, MicroUnits(3_750_000));
        assert_eq!(settled, MicroUnits(3_750_000));
        assert_eq!(
            controller.current_spend_micro_units(tenant),
            MicroUnits(3_750_000)
        );
    }

    #[test]
    fn test_micro_paise_conversion_and_display() {
        // ₹150.50 -> 150,500,000 micro-paise
        let mp = MicroPaise::from_inr(150.50);
        assert_eq!(mp.0, 150_500_000);
        assert_eq!(mp.to_inr(), 150.50);

        // USD conversion at reference peg 84.50
        // $10.00 -> ₹845.00 -> 845,000,000 micro-paise
        let mp_usd = MicroPaise::from_usd(10.0, 84.50);
        assert_eq!(mp_usd.0, 845_000_000);
        assert!((mp_usd.to_usd(84.50) - 10.0).abs() < 1e-6);

        // Display check
        assert_eq!(mp.format_indian_currency(), "₹150.50");
    }

    #[test]
    fn test_format_inr_currency_lakhs_crores() {
        assert_eq!(format_inr_currency(500.0), "₹500.00");
        assert_eq!(format_inr_currency(1200.50), "₹1,200.50");
        assert_eq!(format_inr_currency(150000.0), "₹1,50,000.00");
        assert_eq!(format_inr_currency(12345678.90), "₹1,23,45,678.90");

        assert_eq!(format_lakhs_crores(500.0), "₹500.00");
        assert_eq!(format_lakhs_crores(45000.0), "₹45.00k");
        assert_eq!(format_lakhs_crores(150000.0), "₹1.50 Lakh");
        assert_eq!(format_lakhs_crores(25000000.0), "₹2.50 Cr");
    }

    #[test]
    fn test_indian_gst_tax_engine() {
        // Intra-state (CGST 9% + SGST 9%) on ₹10,000
        let intra = GstBreakdown::calculate(10000.0, false, Some("29ABCDE1234F1Z5"));
        assert_eq!(intra.taxable_amount_inr, 10000.0);
        assert_eq!(intra.cgst_rate_percent, 9.0);
        assert_eq!(intra.cgst_amount_inr, 900.0);
        assert_eq!(intra.sgst_rate_percent, 9.0);
        assert_eq!(intra.sgst_amount_inr, 900.0);
        assert_eq!(intra.igst_amount_inr, 0.0);
        assert_eq!(intra.total_tax_inr, 1800.0);
        assert_eq!(intra.total_amount_inr, 11800.0);
        assert_eq!(intra.sac_code, "998313");

        // Inter-state (IGST 18%) on ₹50,000
        let inter = GstBreakdown::calculate(50000.0, true, Some("27ABCDE1234F1Z5"));
        assert_eq!(inter.cgst_amount_inr, 0.0);
        assert_eq!(inter.sgst_amount_inr, 0.0);
        assert_eq!(inter.igst_rate_percent, 18.0);
        assert_eq!(inter.igst_amount_inr, 9000.0);
        assert_eq!(inter.total_amount_inr, 59000.0);
    }

    #[test]
    fn test_gstin_validation() {
        assert!(validate_gstin("29ABCDE1234F1Z5"));
        assert!(validate_gstin("07AAAAA0000A1Z5"));

        // Invalid length
        assert!(!validate_gstin("29ABCDE1234F1Z"));
        // Invalid state code
        assert!(!validate_gstin("00ABCDE1234F1Z5"));
        assert!(!validate_gstin("99ABCDE1234F1Z5"));
        // Invalid default Z
        assert!(!validate_gstin("29ABCDE1234F1A5"));
    }

    #[test]
    fn test_sovereign_model_pricing() {
        let sarvam = ModelPricing::for_model("sarvam-2b-indic");
        assert_eq!(sarvam.model, "sarvam-2b");
        assert_eq!(sarvam.input_cost_per_million, 0.15);
        assert_eq!(sarvam.output_cost_per_million, 0.30);

        let krutrim = ModelPricing::for_model("krutrim-v1");
        assert_eq!(krutrim.model, "krutrim-v1");
        assert_eq!(krutrim.input_cost_per_million, 0.20);
    }
}
