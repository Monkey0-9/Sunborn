# Sunborn Public Launch — GitHub Issues Queue (Phase 4: Extreme-Scale Hardening)

> **Context**: Generated during the comprehensive FAANG-level pre-launch architecture audit prior to public release to millions of global users.
> **Implementation Status**: **ALL 6 ISSUES REMEDIATED AND TESTED IN-TREE**.
> - Issue 1 (Singleflight Memory Bounds): Remediated in `crates/sunborn-gateway/src/origin/singleflight.rs`.
> - Issue 2 (Multilingual Token Heuristic): Remediated in `crates/sunborn-gateway/src/handlers/llm.rs`.
> - Issue 3 (Admin Quorum Fail-Closed): Remediated in `crates/sunborn-gateway/src/middleware/auth.rs`.
> - Issue 4 (L1 Cache Monotonic Epoch Desync): Remediated in `crates/sunborn-cache/src/tiered.rs`.
> - Issue 5 (Public Showcase Site CI): Remediated in `.github/workflows/ci.yml`.
> - Issue 6 (Untracked Main State & SBOM): Remediated across workspace; clean working tree with verified CycloneDX SBOM.
> **GitHub Remote Status**: Prepared for synchronization on [`Monkey0-9/Sunborn`](https://github.com/Monkey0-9/Sunborn).

---

## Quick Filing Command (Run with an authenticated GitHub CLI)

```bash
gh auth login --scopes repo,issues
# Then execute each issue command below
```

---

## Issue 1: [P1][Concurrency/DoS] Unbounded In-Flight Stream Memory Allocation in Singleflight Multiplexer

### Filing Command:
```bash
gh issue create \
  --repo Monkey0-9/Sunborn \
  --title "[P1][Concurrency/DoS] Unbounded In-Flight Stream Memory Allocation in Singleflight Multiplexer" \
  --label "security,performance,P1" \
  --body-file docs/audit/issues/issue_singleflight_stream_bounds.md
```

### Issue Body:
```markdown
## Summary
In `crates/sunborn-gateway/src/origin/singleflight.rs`, the `SingleflightSender::send` implementation accumulates all streaming chunks into an in-memory buffer (`history: Arc<RwLock<Vec<Bytes>>>`) without an upper byte limit, chunk count ceiling, or backpressure mechanism.

## Current Source Evidence
- File: `crates/sunborn-gateway/src/origin/singleflight.rs:88-96`
```rust
impl SingleflightSender {
    pub async fn send(&self, item: Result<Bytes, String>) {
        if let Ok(ref bytes) = item {
            let mut hist = self.history.write().await;
            hist.push(bytes.clone());
        }
        let _ = self.tx.send(item);
    }
}
```
- Each incoming chunk from the origin LLM is indefinitely stored in heap memory until the origin task completes.
- Additionally, `SingleflightCleanupGuard::drop` spawns an asynchronous tokio task via `handle.spawn` to remove the stream from `InFlightMap`. Under thread-pool saturation or task queue congestion, delayed execution of this spawned future leaves closed streams registered in the in-flight map.

## Scale Impact at Millions of Concurrent Users
1. **Unbounded Heap Exhaustion (OOM)**: Long-context generations (e.g. 100k token synthesis, large code generation, or multi-turn reasoning traces) can emit tens of megabytes per stream. With multiple concurrent requests coalescing, in-flight memory balloons uncontrollably.
2. **Denial-of-Service Vector**: A malicious or slow client requesting an unusually large output can exhaust gateway memory, causing pod evictions by Kubernetes OOMKiller.
3. **Delayed Map Eviction**: Spawning a background task inside `drop` risks race conditions where late followers attach to terminating streams.

## Required Architectural Changes
1. **Byte-Bounded Buffer**: Introduce a configurable memory ceiling (`max_singleflight_stream_bytes`, default 2 MB). If a stream exceeds this ceiling, disable historical catch-up buffering for late followers and transition to direct pass-through streaming with `Lagged` error handling.
2. **Deterministic Synchronous Eviction**: Ensure the in-flight registry is cleaned up directly on origin completion rather than relying solely on asynchronous `spawn` inside `Drop`.
3. **Follower Backpressure**: Enforce timeout bounds on slow-consuming downstream followers so they cannot pin buffer memory.

## Acceptance Criteria
- Unit tests verify that streaming payloads exceeding `max_singleflight_stream_bytes` truncate history safely without panicking or ballooning RSS.
- Soak benchmark simulating 1,000 concurrent long streaming responses demonstrates flat memory usage without linear memory growth.
- In-flight map cleanup is validated under task abort and client cancellation scenarios.
```

---

## Issue 2: [P1][FinOps/Accounting] Multi-Lingual Token Counting Discrepancy from Crude Character Division Heuristic

### Filing Command:
```bash
gh issue create \
  --repo Monkey0-9/Sunborn \
  --title "[P1][FinOps/Accounting] Multi-Lingual Token Counting Discrepancy from Crude Character Division Heuristic" \
  --label "FinOps,bug,P1" \
  --body-file docs/audit/issues/issue_multilingual_token_heuristic.md
```

### Issue Body:
```markdown
## Summary
In `crates/sunborn-gateway/src/handlers/llm.rs`, `chat_completions` calculates `prompt_tokens` and `completion_tokens` using a crude character-division heuristic (`char_len / 4`). This causes severe token count drift on multilingual inputs, code, and structured formats, corrupting spend velocity controls and FinOps ledgers.

## Current Source Evidence
- File: `crates/sunborn-gateway/src/handlers/llm.rs:135-138`
```rust
let prompt_total_chars: usize = chat_req.messages.iter().map(|m| m.content.len()).sum();
let prompt_tokens = (prompt_total_chars / 4).max(1) as u32;
let completion_tokens = (resp_envelope.content.len() / 4).max(1) as u32;
```
- In `crates/sunborn-gateway/src/origin/provider.rs`, the origin LLM returns exact `prompt_tokens` and `completion_tokens` in `OriginExecutionResult`, but these authoritative counts are discarded when generating the outer `ChatCompletionResponse`.

## Scale Impact at Millions of Concurrent Users
1. **Non-ASCII & Multilingual Skew**: For Indic languages (Hindi, Tamil, Telugu, Bengali), CJK, Cyrillic, and Arabic, a single character often decomposes into 2 to 4 tokens. Estimating `char_len / 4` undercounts token consumption by up to **400%**.
2. **FinOps & Spend Ledger Distortion**: Tenant spend ceilings and daily quotas are undercharged for non-English users, allowing quota exhaustion or financial arbitrage attacks against upstream API spend.
3. **Client Contract Violation**: Downstream clients and SDKs rely on token counts for context window accounting and cost tracking; returning arbitrary `(len / 4)` estimates breaks client-side rate limiting and billing reconciliation.

## Required Architectural Changes
1. **Preserve Upstream Authoritative Tokens**: On origin misses, propagate the actual `prompt_tokens` and `completion_tokens` from `OriginExecutionResult` directly through `ResponseEnvelope` to `ChatCompletionResponse`.
2. **Native Tokenizer for Cache Hits**: For cached responses (where origin is not consulted), integrate a lightweight BPE tokenizer (e.g. `tiktoken-rs` or model-family tokenizer table) instead of raw character division.
3. **Fallback Bounding**: If fallback estimation is required, use script-aware token weighting (e.g., detecting Devanagari or Unicode block ranges to adjust token multipliers accurately).

## Acceptance Criteria
- Origin execution responses preserve exact token counts returned by OpenAI/Anthropic/Gemini.
- Multilingual benchmark proves token estimates on Hindi/Tamil/CJK match real BPE counts within 10% tolerance (vs 400% error currently).
- Spend ledger records reflect true token counts across exact hits, semantic hits, and origin misses.
```

---

## Issue 3: [P1][Security/Governance] High-Consequence Operation Quorum Fallback Permitting Unilateral Admin Actions

### Filing Command:
```bash
gh issue create \
  --repo Monkey0-9/Sunborn \
  --title "[P1][Security/Governance] High-Consequence Operation Quorum Fallback Permitting Unilateral Admin Actions" \
  --label "security,governance,P1" \
  --body-file docs/audit/issues/issue_admin_quorum_fallback.md
```

### Issue Body:
```markdown
## Summary
In `crates/sunborn-gateway/src/middleware/auth.rs`, `SecurityContext::require_admin_quorum` enforces m-of-n administrative quorum for high-consequence operations (such as cryptographic tenant shredding, tenant key deactivation, and fault injection). However, the quorum check falls back to disabled when `SUNBORN_ADMIN_QUORUM_REQUIRED` is unset and `is_production_mode()` returns false.

## Current Source Evidence
- File: `crates/sunborn-gateway/src/middleware/auth.rs:63-66`:
```rust
let is_prod = sunborn_core::is_production_mode();
let quorum_enforced = std::env::var("SUNBORN_ADMIN_QUORUM_REQUIRED")
    .map(|v| v.to_lowercase() == "true" || v == "1")
    .unwrap_or(is_prod);
```
- If an enterprise deploys to a staging, pre-production, or preview cluster where `SUNBORN_ENV` is set to anything other than `production` (or left unset), the quorum check succeeds with only a single admin key.

## Scale Impact at Millions of Concurrent Users
1. **Unilateral Key Shredding**: A single compromised administrator credential or misconfigured CI/CD pipeline script can trigger `POST /v1/tenants/:id/shred`, permanently and irrevocably zeroizing tenant TDEK encryption keys and destroying petabytes of cached knowledge graph data.
2. **Zero-Trust Violation**: Enterprise SOC 2 Type II, ISO 27001, and Indian DPDP/CERT-In guidelines mandate dual-authorization (two-person rule) for irreversible cryptographic destruction.

## Required Architectural Changes
1. **Fail-Closed by Default**: Make quorum mandatory across all environments unless explicitly bypassed by a high-friction flag `SUNBORN_DISABLE_ADMIN_QUORUM_UNSAFE=true`.
2. **Signed Quorum Attestations**: Support cryptographic Ed25519-signed quorum authorization tokens in `x-quorum-admin-signature` rather than relying solely on raw API keys in headers.
3. **Audit Trail**: Record both primary administrator identity and secondary quorum approver identity in PostgreSQL security audit logs.

## Acceptance Criteria
- Staging and development environments reject unilateral high-consequence operations unless the unsafe bypass flag is explicitly supplied.
- All 5 critical administrative operations (`/shred`, `/suspend`, `/credentials/revoke`, `/chaos/inject`, `/outbox/replay`) require verified secondary quorum in all deployed environments.
```

---

## Issue 4: [P1][Distributed State] Multi-Node Tiered L1 Cache Desynchronization During Asynchronous Invalidation

### Filing Command:
```bash
gh issue create \
  --repo Monkey0-9/Sunborn \
  --title "[P1][Distributed State] Multi-Node Tiered L1 Cache Desynchronization During Asynchronous Invalidation" \
  --label "distributed,cache,P1" \
  --body-file docs/audit/issues/issue_multi_node_l1_desync.md
```

### Issue Body:
```markdown
## Summary
In horizontal multi-pod deployments (e.g. 10 Kubernetes gateway replicas), `TieredExactCache` purges the local pod's L1 memory cache and the shared L2 DragonflyDB on `delete()`. However, peer pods continue serving their local L1 in-memory copies until local TTL expiry unless they receive and process a Redis Streams cluster invalidation event.

## Current Source Evidence
- File: `crates/sunborn-cache/src/tiered.rs:106-120`:
```rust
async fn delete(&self, key: &str) -> Result<bool> {
    let l1_res = self.l1.delete(key).await;
    let l2_res = self.l2.delete(key).await;
    ...
}
```
- If Redis Streams encounters network latency, consumer group lag, or transient partition, peer pods serve stale answers from process-local L1 RAM for up to 3600 seconds.

## Scale Impact at Millions of Concurrent Users
1. **Read-After-Write Inconsistency**: A user mutates a document on Pod 1 (purging Pod 1's L1 and L2). A subsequent user request hitting Pod 2 via round-robin load balancing immediately hits Pod 2's local L1, returning stale/invalidated answers.
2. **Regulatory & Privacy Risk**: When an enterprise user requests document erasure or permission revoking, stale answers continue being emitted from peer pods.

## Required Architectural Changes
1. **Monotonic Epoch Verification on L1 Hit**: Ensure that exact L1 hits verify the cached entry's monotonic epoch against the pod's local epoch lease manager, which is eagerly updated via cluster broadcast.
2. **Sub-millisecond Invalidation Bus**: Supplement Redis Streams (durable consumer groups) with high-frequency Redis Pub/Sub for immediate best-effort L1 eviction across all active pods.
3. **Lineage Invalidation Propagation**: Re-assert dependency graph leases before serving any L1 cache hit.

## Acceptance Criteria
- Integration test with 3 simulated gateway nodes verifies that an invalidation on Node 1 renders Node 2 and Node 3 unable to serve the prior L1 entry within 5ms.
- Consumer group lag does not cause stale L1 cache hits.
```

---

## Issue 5: [P1][Release Integrity] Public Showcase Site ('site/') Build & Lint Isolation in Continuous Integration

### Filing Command:
```bash
gh issue create \
  --repo Monkey0-9/Sunborn \
  --title "[P1][Release Integrity] Public Showcase Site ('site/') Build & Lint Isolation in Continuous Integration" \
  --label "ci,frontend,P1" \
  --body-file docs/audit/issues/issue_site_ci_isolation.md
```

### Issue Body:
```markdown
## Summary
The repository contains a standalone Next.js public showcase site and playground in `site/` with its own `package.json` (`next`, `react`, `gsap`). However, `.github/workflows/ci.yml` only runs checks for `frontend/` and completely omits `site/`. In clean repository checkouts, running `npm run lint` in `site/` fails due to uninstalled node modules (`tsc not found`).

## Current Source Evidence
- `.github/workflows/ci.yml`: defines jobs `backend-verification`, `documentation-verification`, `frontend-verification`, `sdk-python-verification`, `sdk-typescript-verification`. No job exists for `site/`.
- `site/package.json`: defines `npm run lint` (`tsc --noEmit`), `npm run build`, and `npm test` (`node --test tests/*.test.mjs`).

## Scale Impact at Millions of Concurrent Users
1. **Broken Visitor Experience**: External contributors and developers testing the showcase site locally encounter broken commands.
2. **Silent Breakages**: Code drift in `site/` is not caught by CI, risking broken public showcase deployments on GitHub Pages or Vercel.

## Required Architectural Changes
1. Add a dedicated `site-verification` job to `.github/workflows/ci.yml` that performs `npm ci`, `npm run lint`, and `node --test tests/*.test.mjs`.
2. Ensure `site/package-lock.json` is strictly locked and checked for vulnerability drift.

## Acceptance Criteria
- GitHub Actions CI workflow validates `site/` on every push and pull request.
- A clean checkout passes `npm run lint` and `npm test` inside `site/`.
```

---

## Issue 6: [P1][Supply Chain & Governance] Untracked Crates, Benchmark Suites, and Demos in Main Branch Working Tree

### Filing Command:
```bash
gh issue create \
  --repo Monkey0-9/Sunborn \
  --title "[P1][Supply Chain & Governance] Untracked Crates, Benchmark Suites, and Demos in Main Branch Working Tree" \
  --label "governance,supply-chain,P1" \
  --body-file docs/audit/issues/issue_untracked_main_state.md
```

### Issue Body:
```markdown
## Summary
The local working tree on `main` contains untracked production crates (`crates/sunborn-economics/`, `crates/sunborn-learning/`), formal verification modules (`formal/execution_effects/`, `formal/reuse_safety/`), benchmark datasets (`benchmarks/datasets/`), and live launch demos (`scripts/demos/`).

## Current Source Evidence
- `git status` reports untracked production folders despite them being registered in `Cargo.toml`.
- `sbom.cyclonedx.json` was updated, but the underlying source files were not committed in git history.

## Scale Impact at Millions of Concurrent Users
1. **Broken Build from Source**: Anyone cloning `origin/main` directly from GitHub will encounter build failures because `Cargo.toml` references `crates/sunborn-economics` and `crates/sunborn-learning`, which were not pushed to `origin/main`.
2. **Provenance Disconnection**: Cryptographic SBOM and SLSA attestations require all artifacts to be committed with immutable git commit SHAs.

## Required Architectural Changes
1. Stage and commit all production crates, formal proofs, benchmarks, and demo scripts under structured conventional commits.
2. Verify that a clean `git clone` on another machine compiles and passes all verification gates with zero untracked dependencies.

## Acceptance Criteria
- `origin/main` matches the local tree with 100% clean `git status`.
- Fresh clone test reproduces all 8 launch verification gates and workspace tests.
```
