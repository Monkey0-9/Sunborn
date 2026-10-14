# Sunborn Frontend & Control Plane Architecture

# Frontend & Control Plane

## Stack

Next.js App Router + TypeScript + Tailwind/shadcn-style component system; API client generated or contract-driven from backend schemas.

## Surfaces

 1. Executive overview: SLOs, throughput, hit rate, cost savings, incidents.
 2. Request explorer: request trace, canonical identity, cache path, decision and latency.
 3. Cache explorer: exact/semantic/execution entries, status, TTL, versions, provenance and dependencies.
 4. Decision center: similarity/confidence/risk/freshness/policy features and reason codes.
 5. Knowledge/freshness: source → version → dependency → affected cache graph.
 6. Security center: threats, attacks, blocked events, tenant-isolation results and audit.
 7. Model registry: model/embedding/reranker/tool versions and compatibility.
 8. Benchmark center: baselines, experiments, metrics, confidence intervals and comparisons.
 9. Research center: hypotheses, experiment status and evidence links.
10. Operations: services, dependencies, queues, resource usage and deployment state.
11. Release readiness: correctness/security/reliability/performance/reproducibility gates.

## Frontend rules

* Server-side authorization where sensitive data is fetched.
* No secrets in client bundles.
* All mutations go through audited backend APIs.
* Pagination/filtering for large datasets.
* PII redaction in views.
* Error/loading/empty states for every panel.
* Accessibility and keyboard navigation.
* Visualizations must be tied to real telemetry/experiment data, not mock success states.

## Frontend test pipeline

unit/component → contract → integration with API → end-to-end critical flows → accessibility smoke → build.
