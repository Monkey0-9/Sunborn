# 18 — Integration Audit

End-to-end connectivity verification across boundaries.

---

## 1. Boundary matrix

| Boundary | Wired? | Real call? | Verdict |
|---|---|---|---|
| Frontend browser → Next BFF | yes | yes | WORKING |
| BFF → Gateway | yes | yes when up | WORKING; **fake backend on failure in dev** (F-002) |
| Frontend auth → gateway | yes | yes when up | **BROKEN fallback** (F-001) |
| Gateway → origin LLM | opt-in | yes (reqwest) | WORKING when `SUNBORN_LLM_ORIGIN_URL` set; default Mock |
| Gateway → embeddings | opt-in | yes (reqwest) | WORKING when set; default LSH; silent fallback |
| Gateway → Postgres | opt-in | yes (sqlx) | WORKING; silent memory fallback |
| Gateway → Redis/Dragonfly | opt-in | yes (redis) | WORKING; silent fallback |
| Gateway → Qdrant | opt-in | yes (reqwest REST) | WORKING; delete bug F-006 |
| Gateway → Neo4j | **no env path** | no | UNWIRED |
| Gateway ↔ cluster sync | not referenced | no | UNWIRED |
| Gateway → KMS/Vault | opt-in | yes | WORKING when credentials set |
| Gateway → durable outbox | no | no | DEAD CODE |
| Gateway → write-behind | no | no | UNWIRED |
| Cache → vector invalidation | yes | in-process | WORKING (lineage bug F-017) |
| Attestation issue → verify | yes | in-process | WORKING but self-attesting (F-004) |
| Python SDK → gateway | yes | yes | WORKING (9 endpoints resolve) |
| Frontend ← tests | mocked | no | MOCK-ONLY E2E |

## 2. The fake-backend integration path

`frontend/src/app/api/proxy/[...path]/route.ts:163-190`:
- In production or `STRICT_GATEWAY=true`, or for privileged routes → fail closed (502). Good.
- Otherwise → `handleSimulatedRoute()` serves fabricated responses with `x-sunborn-simulated: true`.

Because `npm run dev` is the documented local experience, a developer interacting with the console is **not** talking to the gateway. The header `x-sunborn-simulated` is set but the UI does not surface it prominently; several pages render simulated values as live.

## 3. Real end-to-end tests

- Rust: `integration_test.rs` uses `create_router` + `tower::ServiceExt::oneshot` (in-process HTTP). Two tests bind a real TCP listener + reqwest client (`adversarial_security_test.rs`). Startup tests spin mock TCP/HTTP servers.
- **No test exercises live Postgres, Redis, Qdrant, Neo4j, Vault, or AWS KMS.**
- Frontend Playwright E2E intercepts every gateway call with `page.route(...)`.

## 4. Data-flow correctness

- OpenAI/Anthropic translation both funnel to `execute_request_inner` — verified.
- FinOps reservation released on every reviewed early-exit — verified.
- On cache hit, the returned body is the stored payload; headers claimed in docs are absent.
- Invalidation of a document invalidates direct records; transitive only via unwired Neo4j.

## 5. Verdict

Interfaces exist and are wired when explicitly configured, but the **default** and **failure** paths substitute simulation, and no integration test touches real infrastructure. Integration score: **4/10**.
