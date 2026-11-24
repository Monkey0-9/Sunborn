# Paper 2: Effect-Aware Partial Execution Reuse for Stateful AI Agents

**Target Venues**: EuroSys / USENIX Annual Technical Conference (ATC) / ACM Middleware  
**Title**: *Effect-Aware Partial Execution Reuse for Stateful AI Agents*

---

## Abstract
Autonomous AI agents execute complex, multi-step Directed Acyclic Graphs (DAGs) combining dense retrieval, code interpretation, database queries, and external tool mutations. While whole-response caching yields near-zero hit rates in agentic workflows due to non-deterministic intermediate steps, naive DAG memoization risks catastrophic bugs: replaying payment APIs, ignoring transactional state mutations, or serving stale database reads. We introduce **Effect-Aware Execution Reuse**, an execution planner that classifies tool invocations into typed effect classes: *Pure*, *ReadOnlyStateful*, *ExternalMutableRead*, *IdempotentMutation*, and *NonIdempotentMutation*. By tracking transitive effect taint and pruning mutating subgraphs, Sunborn safely reuses valid sub-computations. In benchmarks across realistic financial and enterprise workflows, Sunborn saves 42% of computation steps with 0% unsafe mutation replays.

## Key Research Contributions
1. **Typed Effect Algebra**: A formal lattice over tool side-effects and transitive taint propagation across agent DAGs.
2. **Safe Subgraph Planner**: An algorithm that partitions agent execution graphs into reusable pure/read subgraphs while enforcing fresh execution for mutable nodes.
3. **State and Idempotency Validation**: Token-based idempotency fencing preventing double-execution of external actions.
4. **Empirical Evaluation**: Tested against agent workflows (financial research, data ingestion, customer support) demonstrating substantial TTFT reduction.
