# MODEL-LEVEL & DOMAIN ARCHITECTURE AUDIT

**Audit Standard**: IEEE 1016-2009 (Software Design Descriptions) / ISO/IEC 25010  
**Scope**: Domain models, entities, state machines, and type-system invariants across Sunborn crates  

---

## 🏛️ Core Domain Models & Struct Census

### 1. `sunborn_core::model::RequestEnvelope`
- **Definition**: Represents canonical incoming prompt envelope stripped of transport quirks.
- **Fields**:
  - `tenant_id: TenantId` (Strongly-typed string wrapper preventing cross-tenant substitution)
  - `application_id: ApplicationId`
  - `model: String` (Model identifier, e.g., `gpt-4o`, `claude-3-5-sonnet`)
  - `messages: Vec<ChatMessage>` (Structured multi-turn prompt history)
  - `temperature: f64`
  - `top_p: Option<f64>`
  - `seed: Option<u64>`
  - `tools: Vec<ToolDefinition>`
- **Invariants**: `tenant_id` and `model` must be non-empty ASCII; `messages` must contain at least 1 message; `temperature` bounded in $[0.0, 2.0]$.
- **Audit Verdict**: ✅ **PASSED**. Strongly typed and validated at ingress boundary.

### 2. `sunborn_core::certificate::ReuseCertificate`
- **Definition**: Cryptographic proof-carrying authorization token bound to exact-match cached entries.
- **Fields**:
  - `tenant_id: TenantId`
  - `application_id: ApplicationId`
  - `model_name: String`
  - `input_hash: String` (Hex-encoded SHA-256 canonical envelope digest)
  - `output_hash: String` (Hex-encoded SHA-256 payload digest)
  - `merkle_root: String` (RFC 6962 root of upstream source document hashes)
  - `valid_from: DateTime<Utc>`
  - `valid_until: DateTime<Utc>`
  - `monotonic_epoch_lease: u64`
  - `signature: String` (HMAC-SHA256 signature generated with `certificate_signing_key`)
- **State Invariants**:
  - Expiration enforcement: `valid_until > Utc::now()`.
  - Signature integrity: $\text{HMAC}(\text{signing\_key}, \text{canonical\_fields}) == \text{signature}$.
  - Monotonicity: `current_source_epoch <= monotonic_epoch_lease`.
- **Audit Verdict**: ✅ **PASSED**. Enforced on exact cache read path in `crates/sunborn-gateway/src/handlers.rs`.

### 3. `sunborn_semantic::context_digest::ContextDigest`
- **Definition**: 32-byte cryptographic binding linking cached vectors to their specific knowledge context.
- **Fields**: `bytes: [u8; 32]`
- **Purpose**: Prevents semantic cross-contamination when identical questions are asked under different legal agreements, system prompts, or tenant domains.
- **Audit Verdict**: ✅ **PASSED**. Enforced as mandatory filter condition during Qdrant HNSW vector queries.

### 4. `sunborn_execution::model::DAGNode`
- **Definition**: Fundamental unit of work within an agent execution graph.
- **Fields**:
  - `id: NodeId`
  - `tool_name: String`
  - `input_parameters: serde_json::Value`
  - `dependencies: Vec<NodeId>`
  - `is_mutating: bool` (Flag indicating side-effectful mutations)
  - `ttl_seconds: Option<u64>`
- **State Machine**:
  - `Pending` $\to$ `Executing` $\to$ `Completed` $\to$ `Memoized`
  - If `is_mutating == true`, state transitions directly to `Completed` without passing through `Memoized`.
- **Audit Verdict**: ✅ **PASSED**. Cycle detection and transitive side-effect contamination verified.

---

## 🔒 Type-System Safety & Invariant Guarantees

Sunborn effectively leverages Rust's type system to eliminate whole classes of distributed systems defects:
1. **Newtype Pattern**: `TenantId`, `ApplicationId`, `NodeId`, `RecordId` are distinct types, preventing argument transposition bugs.
2. **Zeroize on Drop**: Cryptographic secrets and decrypted plaintext buffers implement the `Zeroize` trait, zeroing memory upon drop to prevent cold-boot extraction.
3. **Send + Sync Bounds**: All state shared across Tokio worker threads enforces strict concurrency bounds, eliminating data races at compile time.
