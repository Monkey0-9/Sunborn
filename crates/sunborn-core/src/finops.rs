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
}
