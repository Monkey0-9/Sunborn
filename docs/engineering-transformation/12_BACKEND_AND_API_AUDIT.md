# Backend & Gateway Engine Audit

**Engine:** Axum 0.8 / Tower / Hyper (HTTP/1.1 & HTTP/2)  
**Contract Authority:** OpenAPI 3.1 (`docs/api/openapi.yaml`)  
**Route Count:** 50 Authoritative REST Endpoints  
**Request Multiplexing:** Singleflight Streaming Engine  

---

## 1. Axum 0.8 HTTP Gateway Architecture

The Sunborn gateway (`crates/sunborn-gateway`) operates as an asynchronous, zero-cost reverse proxy.

### 1.1 Ingress Middleware Stack
Every inbound HTTP request traverses a hardened Tower middleware pipeline:
1. **Trace Layer:** Assigns a cryptographically random UUIDv4 correlation ID (`x-request-id`) or validates client-provided trace headers.
2. **Auth Layer:** Validates `Bearer` API keys against the constant-time hash registry (`subtle::ConstantTimeEq`).
3. **Dual-Admin Quorum Gate:** Destructive operations (`/v1/admin/purge`, `/v1/admin/crypto-shred`) require an `X-Admin-Quorum` header containing $M$-of-$N$ Ed25519 signatures.
4. **FinOps Token Bucket:** Evaluates microsecond spend velocity; returns `429 Too Many Requests` if tenant budget is exceeded.

### 1.2 Singleflight Request De-duplication
Under high-concurrency cache misses, multiple identical prompts arriving within milliseconds are coalesced into a single upstream LLM provider call via `crates/sunborn-gateway/src/origin/singleflight.rs`.
- Prevents upstream thundering herds.
- Streaming responses are multiplexed to all waiting followers with zero buffer overrun.
- Verified in `tests/origin/singleflight.rs:test_singleflight_streamer_multiplexing`.
