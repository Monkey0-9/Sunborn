use sunborn_economics::cost_model::{CostComponents, ProviderPricing};
use sunborn_economics::optimizer::{EconomicOptimizer, OptimizationObjective};

#[test]
fn test_net_economic_benefit_with_overhead_and_risk_penalty() {
    let gpt4o = ProviderPricing::gpt_4o();
    // 2,000 prompt tokens, 500 completion tokens
    let origin_cost = gpt4o.calculate_origin_cost(2000, 500);
    // 2000/1M * 2.50 = 0.005; 500/1M * 10.00 = 0.005 -> total $0.010
    assert!((origin_cost - 0.010).abs() < 0.0001);

    let costs = CostComponents {
        origin_avoided_cost_usd: origin_cost,
        lookup_overhead_usd: 0.000002,
        embedding_cost_usd: 0.000020,
        verification_overhead_usd: 0.000001,
        storage_amortized_usd: 0.000001,
        operations_usd: 0.000001,
        risk_expected_penalty_usd: 0.000100,
    };

    let net_benefit = costs.calculate_net_benefit();
    assert!(net_benefit > 0.009); // Net benefit > 0.9 cents

    let optimizer = EconomicOptimizer::new(OptimizationObjective::LowestCost);
    assert!(optimizer.should_reuse(costs));

    // Strictest correctness should reject non-zero risk penalty
    let strict_optimizer = EconomicOptimizer::new(OptimizationObjective::StrictestCorrectness);
    assert!(!strict_optimizer.should_reuse(costs));

    let zero_risk_costs = CostComponents {
        risk_expected_penalty_usd: 0.0,
        ..costs
    };
    assert!(strict_optimizer.should_reuse(zero_risk_costs));

    // ROI check
    let roi = optimizer.calculate_roi(costs.origin_avoided_cost_usd, costs.total_overhead());
    assert!(roi > 100.0); // > 100x return on infrastructure overhead
}
