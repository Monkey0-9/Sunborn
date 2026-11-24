use serde::{Deserialize, Serialize};

/// Real commercial LLM provider pricing catalog per 1M tokens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderPricing {
    pub model_name: String,
    pub prompt_usd_per_1m: f64,
    pub completion_usd_per_1m: f64,
}

impl ProviderPricing {
    pub fn gpt_4o() -> Self {
        Self {
            model_name: "gpt-4o".into(),
            prompt_usd_per_1m: 2.50,
            completion_usd_per_1m: 10.00,
        }
    }

    pub fn claude_3_5_sonnet() -> Self {
        Self {
            model_name: "claude-3-5-sonnet-20241022".into(),
            prompt_usd_per_1m: 3.00,
            completion_usd_per_1m: 15.00,
        }
    }

    pub fn gemini_1_5_pro() -> Self {
        Self {
            model_name: "gemini-1.5-pro".into(),
            prompt_usd_per_1m: 1.25,
            completion_usd_per_1m: 5.00,
        }
    }

    pub fn sarvam_2b() -> Self {
        Self {
            model_name: "sarvam-2b".into(),
            prompt_usd_per_1m: 0.10,
            completion_usd_per_1m: 0.20,
        }
    }

    pub fn calculate_origin_cost(&self, prompt_tokens: u32, completion_tokens: u32) -> f64 {
        let prompt_cost = (prompt_tokens as f64 / 1_000_000.0) * self.prompt_usd_per_1m;
        let comp_cost = (completion_tokens as f64 / 1_000_000.0) * self.completion_usd_per_1m;
        prompt_cost + comp_cost
    }
}

/// Detailed multi-dimensional cost components of an execution reuse decision.
/// V_reuse = C_origin_avoided - C_lookup - C_verification - C_storage - C_operations - C_risk - C_embedding
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CostComponents {
    pub origin_avoided_cost_usd: f64,
    pub lookup_overhead_usd: f64,
    pub embedding_cost_usd: f64,
    pub verification_overhead_usd: f64,
    pub storage_amortized_usd: f64,
    pub operations_usd: f64,
    pub risk_expected_penalty_usd: f64,
}

impl CostComponents {
    /// Computes net economic benefit (V_reuse).
    pub fn calculate_net_benefit(&self) -> f64 {
        self.origin_avoided_cost_usd
            - self.lookup_overhead_usd
            - self.embedding_cost_usd
            - self.verification_overhead_usd
            - self.storage_amortized_usd
            - self.operations_usd
            - self.risk_expected_penalty_usd
    }

    /// Computes total infrastructure cache overhead cost.
    pub fn total_overhead(&self) -> f64 {
        self.lookup_overhead_usd
            + self.embedding_cost_usd
            + self.verification_overhead_usd
            + self.storage_amortized_usd
            + self.operations_usd
    }
}
