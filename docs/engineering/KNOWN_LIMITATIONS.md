# Sunborn: Register of Known Engineering Limitations & Operational Boundaries

**Document Version:** 1.0.0  
**Verification Target:** Sunborn Platform (Rust Workspace, Frontend, SDK, Infrastructure)  
**Baseline Commit:** `dd25675c5eae200b2518557f966feff74d1556fe`  

---

## 1. Transparency Statement

In alignment with world-class engineering standards (Google, Apple, Meta, OpenAI, Microsoft, NVIDIA), this register explicitly documents the known boundaries, unverified operational zones, and architectural limitations of Sunborn. No capability is claimed beyond what can be independently reproduced from the source code.

---

## 2. Technical Limitations Register

### 1. Invalidation Latency Under High Graph Depth
- **Limitation:** While single-node lookups take $< 1\text{ µs}$ and shallow invalidations take $4.0\text{ µs}$ P50, deep transitive invalidation cascades (depth $> 16$, edges $> 350,000$) take up to **$3,806.4\text{ µs}$ P99**.
- **Impact:** Invalidation bursts involving heavily interconnected source documents may cause momentary latency spikes on the invalidation worker thread.
- **Mitigation:** Traversal depth is hard-capped at 32; invalidation operations run asynchronously off the client request hot path.

### 2. Multi-Pod Graph Consensus Boundary
- **Limitation:** In-memory Roaring Treemaps operate per gateway process. Multi-pod deployments do not maintain a distributed peer-to-peer consensus ring (e.g. Raft) directly inside the Rust gateway.
- **Impact:** Gateways rely on an external outbox broker (Kafka, Dragonfly, or PostgreSQL CDC) to synchronize invalidation events across pods. If the broker lags, pods experience transient invalidation skew until events are drained.
- **Mitigation:** Monotonic watermarks and epoch leases reject stale writes even during broker replay.

### 3. Cryptographic Certificate Signer Identity
- **Limitation:** Out-of-the-box local runs emit certificates verified as `INTEGRITY_VERIFIED_UNAUTHENTICATED`.
- **Impact:** The certificate mathematically proves that execution inputs, outputs, and Merkle hashes have not been tampered with, but does not provide non-repudiation of signer identity unless an external AWS KMS key or Sigstore identity is bound.
- **Mitigation:** Production mode strictly requires KMS key ARNs or Vault tokens (`production_startup_test.rs`).

### 4. Semantic Recall vs Safety Trade-Off
- **Limitation:** Sunborn achieves a verified **0.00% false-hit rate** on adversarial linguistic traps, but achieves **45.83% recall** on legitimate paraphrases (Wilson 95% CI: [27.89%, 64.93%]).
- **Impact:** The system conservatively abstains from caching paraphrases that have minor syntactic divergence, sacrificing token savings in exchange for guaranteed safety.
- **Mitigation:** Planned Stage 1 cross-encoder re-ranking for borderline similarity scores ($s \in [0.80, 0.88]$).

### 5. Live Staging Host Daemon Boundary
- **Limitation:** The evaluation host running this verification program had Docker Desktop and Kubernetes daemons offline (`connectex: No connection could be made`).
- **Impact:** Kubernetes manifests, Helm charts, Dockerfiles, and Kustomize overlays were verified statically via AST parsing, schema linting, and unit assertions, but live multi-node pod eviction and split-brain chaos tests could not be executed on this bare metal host.
- **Mitigation:** Staging cluster validation script provided in [`deploy/verify_live_stack.sh`](file:///C:/Sunborn/deploy/verify_live_stack.sh).

### 6. PII Redaction Coverage
- **Limitation:** PII regex redaction filters cover US SSNs, credit cards, standard UUIDs, and email addresses. Non-standard international identification formats require custom tenant regex definitions.
- **Impact:** Unmatched entity patterns may pass through to upstream LLM providers unless configured in tenant privacy policy rules.
- **Mitigation:** Configurable tenant regex policies in [`crates/sunborn-gateway/src/privacy.rs`](file:///C:/Sunborn/crates/sunborn-gateway/src/privacy.rs).
