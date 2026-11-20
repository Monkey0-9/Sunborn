# Free-Tier Architecture

**Owner:** SRE and platform, with Security and FinOps
**Status:** design for private beta; no public free tier is authorized today
**Authority for current status:** [`docs/STATUS.md`](../STATUS.md)

Part of the [launch package](README.md).

---

## 1. Design constraints

The free tier is the front door to the product, which makes it the cheapest place to
damage it. Three constraints are non-negotiable:

1. **Free traffic gets the same correctness path.** A free request runs the same
   guardrails, the same causal check, and the same abstention gate as a paid request.
   Quotas may reject a request; they may never make a cached answer less safe.
2. **Spend is bounded before the request leaves the gateway.** A free tenant cannot
   consume unmetered upstream tokens. The bound is enforced by reservation, not by
   reconciliation after the invoice.
3. **Quota failure is explicit.** Exceeding a quota returns a documented `429` naming
   the scope and reset window; a dependency that cannot enforce the quota returns
   `503`. Neither case degrades into an unguarded or unmetered mode.

The operating rule is the same one in [`GOVERNANCE.md`](../../GOVERNANCE.md): if a
control cannot be evidenced, the tier does not open.

---

## 2. Primitives that exist today

These are implemented in the current tree. Each row names the mechanism and the
limitation that matters for a free tier.

| Primitive | Implemented behavior | Source | Limitation for free tier |
|---|---|---|---|
| Tenant identity | Request tenant is taken from the verified credential, not from a caller header | [`crates/sunborn-gateway/src/middleware/auth.rs`](../../crates/sunborn-gateway/src/middleware/auth.rs) | `tenants.tier` is stored, but tier and tenant status are not admission gates |
| API-key lifecycle | SHA-256 hashed keys, expiry, per-key revocation, tenant-wide revocation | [`crates/sunborn-gateway/src/middleware/auth.rs`](../../crates/sunborn-gateway/src/middleware/auth.rs), [`migrations/0002_durable_identity_and_keys.sql`](../../migrations/0002_durable_identity_and_keys.sql) | Key creation is an operator action, not self-serve |
| Model allowlist | Per-credential `allowed_models`; `*` means all; checked before dispatch | `SecurityContext::allows_model`, [`crates/sunborn-gateway/src/handlers/mod.rs`](../../crates/sunborn-gateway/src/handlers/mod.rs) | The list is per key, not per tier; a tier default must be applied at provisioning |
| Per-tenant rate limit | Token bucket keyed by authenticated tenant; `SUNBORN_RATE_LIMIT_RPS` default 100, `0` disables; standard `x-ratelimit-*` headers and `Retry-After` | [`crates/sunborn-gateway/src/middleware/rate_limit.rs`](../../crates/sunborn-gateway/src/middleware/rate_limit.rs), [`crates/sunborn-gateway/src/quota.rs`](../../crates/sunborn-gateway/src/quota.rs) | Applies only to quota-enrolled tenants. The bucket is cluster-wide when PostgreSQL is active; the in-memory fallback is process-local. It cannot express a monthly or daily cap |
| Login rate limit | 5-attempt bucket refilling at 1/s, keyed per client; kill switch `SUNBORN_FEATURE_LOGIN_RATE_LIMIT` | [`crates/sunborn-gateway/src/middleware/rate_limit.rs`](../../crates/sunborn-gateway/src/middleware/rate_limit.rs) | Protects the login path only; it is not a tenant quota |
| Spend reservation | Check-and-reserve before origin dispatch; released on a cache hit; settled with actual cost | [`crates/sunborn-gateway/src/finops.rs`](../../crates/sunborn-gateway/src/finops.rs), [`crates/sunborn-gateway/src/handlers/mod.rs`](../../crates/sunborn-gateway/src/handlers/mod.rs) | Estimated from prompt word count and a fixed 500-token completion assumption |
| Durable spend ledger | Cluster-wide enforcement with `pg_advisory_xact_lock(hashtextextended(tenant_id, 0))`; reservations and settlements are unique | [`crates/sunborn-db/src/postgres.rs`](../../crates/sunborn-db/src/postgres.rs), [`migrations/0007_distributed_spend_ledger.sql`](../../migrations/0007_distributed_spend_ledger.sql) | Requires PostgreSQL; the in-memory backend is process-local |
| Spend ceiling | Rolling-window ceiling per tenant, default `$100` per 60 s via `SUNBORN_MAX_SPEND_PER_WINDOW_USD`; durable override through `POST /v1/governance/spend-ceiling` (admin only) | [`crates/sunborn-core/src/finops.rs`](../../crates/sunborn-core/src/finops.rs), [`crates/sunborn-gateway/src/handlers/mod.rs`](../../crates/sunborn-gateway/src/handlers/mod.rs) | A velocity ceiling, not a monthly spend budget; the free pool is a request ceiling, not a spend ceiling |
| Request quotas | Opt-in daily/monthly counters per tenant and an aggregate free-pool counter; PostgreSQL transactions with per-tenant and pool advisory locks; in-memory fallback for single-replica development; admin controls at `POST /v1/governance/tenant-quota` and `POST /v1/governance/free-pool` | [`crates/sunborn-gateway/src/quota.rs`](../../crates/sunborn-gateway/src/quota.rs), [`crates/sunborn-db/src/postgres.rs`](../../crates/sunborn-db/src/postgres.rs), [`migrations/0008_free_tier_quotas.sql`](../../migrations/0008_free_tier_quotas.sql) | Enrollment is explicit and operator-driven; there are no tier defaults, and the RPS bucket is cluster-wide only when PostgreSQL is active |
| Enforced request caps | Server-side prompt and completion caps applied before quota and spend; `SUNBORN_MAX_PROMPT_CHARS` default 32,000, `SUNBORN_MAX_COMPLETION_TOKENS` default 1,000, `0` disables either. The spend reservation uses the clamped completion budget. | [`crates/sunborn-gateway/src/limits.rs`](../../crates/sunborn-gateway/src/limits.rs), [`crates/sunborn-gateway/src/handlers/mod.rs`](../../crates/sunborn-gateway/src/handlers/mod.rs) | A global cap, not yet a per-tier cap; paid tiers will need their own limit record |
| Fail-closed spend | Ledger unreachable returns `503`; ceiling exceeded returns `429` | [`crates/sunborn-gateway/src/finops.rs`](../../crates/sunborn-gateway/src/finops.rs), [`crates/sunborn-gateway/src/handlers/mod.rs`](../../crates/sunborn-gateway/src/handlers/mod.rs) | Correct by design, and it means a ledger outage is a free-tier outage |
| Request-size bound | 4 MiB body limit | [`crates/sunborn-gateway/src/lib.rs`](../../crates/sunborn-gateway/src/lib.rs) | Bounds one request, not a tenant's daily volume |
| Cache-first path | Exact hit, then guarded semantic path; abstention routes upstream | [`crates/sunborn-gateway/src/handlers/mod.rs`](../../crates/sunborn-gateway/src/handlers/mod.rs) | Free-tier economics depend on recall, which is measured at 20% on the held-out set |
| Admin quorum | Destructive operations require a distinct second admin key in production | [`crates/sunborn-gateway/src/middleware/auth.rs`](../../crates/sunborn-gateway/src/middleware/auth.rs) | Protects operator actions, not tenant quotas |

Two facts follow from this table and should be stated before any launch material:

- **Quota enforcement exists but is opt-in.** A tenant is counted only after an
  operator writes a quota row and, for the aggregate ceiling, a free-pool row. There
  are still no tier defaults and no token counters. The RPS bucket is cluster-wide
  for enrolled tenants when PostgreSQL is active.
- **The spend estimate is an estimate.** Admission reserves a cost derived from prompt
  length and a fixed 500-token completion assumption; settlement records the actual
  cost. The proposed free-tier completion cap is 1,000 tokens, so the reservation must
  use the effective cap rather than the fixed 500, or it will under-reserve by up to
  half the completion cost. A free tier must also cap the completion length itself,
  not rely on the estimate.

---

## 3. What a free tier adds

A free tier is a policy layer over these primitives. The quota record, counters, and
aggregate pool now exist; the remaining pieces are entry work for private beta.

| Addition | Why it is required | Status | Needed before |
|---|---|---|---|
| Tenant `tier` and quota record | One authoritative place for class, limits, and reset window | Implemented in migration 0008 and the admin quota endpoint; tier defaults are not applied automatically. A replica that has seen a tenant as unenrolled re-checks after at most 60 seconds. | Beta entry |
| Shared rate limiter | A per-process bucket multiplies by replica count; free-tier RPS must be cluster-wide | Implemented in migration 0009 and the rate-limit middleware for quota-enrolled tenants. The in-memory fallback remains process-local. | Beta entry |
| Daily and monthly request counters | A velocity ceiling cannot express "1,000 requests per month" | Implemented; a two-replica race admitted exactly 5 of 30 requests at a 5/day limit. Local evidence, not a full B2 cycle. | B2 |
| Enforced prompt/completion caps | A free tier must clamp client-supplied completion length and bound prompt size | Implemented in the shared admission path for all three protocols; defaults are 32,000 prompt characters and 1,000 completion tokens. Global, not yet per-tier. | B2 |
| Global free-pool counter | Per-tenant ceilings bound each tenant, not the aggregate; the pool needs its own ceiling | Implemented as an opt-in request counter; spend reconciliation pending | B2 |
| Tier defaults at provisioning | Model allowlist, retention, and support level must be applied consistently, not per key by hand | Partial; the quota endpoint writes the record, but allowlist and retention stay manual | B2 |
| Abuse detection and review queue | Invite-only onboarding plus anomaly review, not an automated ban loop | Not implemented | B3 |
| Billing and upgrade path | A free tenant that outgrows the tier needs a self-serve or assisted path without a new tenant | Not implemented | B5 |

Until the remaining pieces exist, a "free tier" is an operator-created tenant with a
quota row, a hand-set allowlist, and a ceiling. That is a pilot, not a tier, and it
should be described that way.

---

## 4. Tenant model

### 4.1 Classes

| Class | Audience | Provisioning | Enforcement today |
|---|---|---|---|
| `PILOT` | Named design partners | Operator-created, per agreement | Existing key, rate-limit, and spend primitives |
| `FREE` | Self-serve, invitation-controlled during beta | Planned self-serve with verification | Not implemented; this document defines it |
| `PAID` | Teams that outgrow the free tier | Planned assisted or self-serve upgrade | Same primitives with higher limits and support terms |

Class changes are a quota change and a billing event, not a new tenant. The tenant ID,
keys, cache namespace, and audit history survive an upgrade. A downgrade applies at the
end of the billing period and never deletes data immediately.

### 4.2 Proposed free-tier envelope

The values below are a **proposal for the beta**, not defaults and not current policy.
They are enforced only after an operator enrolls a tenant with the quota endpoint and,
for the aggregate ceiling, configures the free pool. They become policy when the B2
evidence shows the aggregate stayed under its ceiling.

| Parameter | Proposed starting value | Enforcement point |
|---|---|---|
| Requests per month | 1,000 | Monthly counter |
| Requests per day | 50 | Daily counter |
| Burst rate | 10 requests/minute, bucket of 10 | Shared token bucket |
| Concurrent streams | 1 per tenant | Stream admission |
| Prompt tokens per request | 8,000 | Request validation |
| Completion tokens per request | 1,000 | Clamp on `max_tokens` |
| Tokens per day | 100,000 | Daily counter |
| Model allowlist | Low-cost models only, chosen at beta | Credential `allowed_models` |
| Spend ceiling | `$0.10` per rolling minute and `$0.50` per day | Spend ledger |
| Cache retention | Shortest retention offered; exact TTL by configuration | Cache configuration |
| Log retention | 30 days for request metadata | Deployment configuration |
| Support | Documentation and a monitored address; no response SLA | [`SUPPORT.md`](../../SUPPORT.md) |

The arithmetic that keeps this bounded, using the pricing constants in
[`crates/sunborn-core/src/finops.rs`](../../crates/sunborn-core/src/finops.rs) and an
illustrative 1,000 prompt + 500 completion token request at the fallback
`$2.00`/`$8.00` per million rate:

- one request costs about `$0.006` in upstream tokens;
- 50 requests per day is about `$0.30`, below the `$0.50` daily ceiling;
- 1,000 requests per month is about `$6.00` per tenant;
- 1,000 free tenants would be about `$6,000` per month at full utilization.

Those are arithmetic bounds, not a forecast. They exclude embeddings, storage,
egress, and support. The global pool must be sized from the last line, and B2 must
reconcile it against the ledger rather than against this paragraph.

---

## 5. Admission order and failure semantics

The free tier applies gates in this order. The order matters because it determines
which failure a caller sees and what it costs to serve. Steps marked **(planned)** do
not run today; steps marked **(if enrolled)** run only for tenants with a quota row.

```
1. Authenticate key                  -> 401 invalid, expired, or revoked key
2. Resolve tenant and tier           -> 403 suspended or class not entitled (planned)
3. Check model allowlist             -> 403 model not permitted for this credential
4. Check rate limit                  -> 429 with Retry-After (cluster-wide if enrolled and PostgreSQL is active)
5. Check daily/monthly counters      -> 429 with the reset window (if enrolled)
6. Reserve estimated spend           -> 429 ceiling exceeded, or 503 ledger unavailable
7. Exact cache lookup                -> hit: release reservation, return
8. Guarded semantic path             -> abstain: continue upstream
9. Origin dispatch                   -> settle actual cost, admit result
```

Rules that the implementation must preserve:

- **A cache hit releases its reservation.** The current handler does this on every
  hit path; a free tier must not bill a tenant for origin tokens it did not spend.
- **Tenant status is stored but not enforced today.** The `tenants.status` column is
  written by the database layer and is not consulted by the gateway. Until step 2 is
  implemented, suspending a tenant means revoking its keys.
- **A ledger outage fails closed.** `503`, not a local fallback budget.
- **A quota rejection is not a cache miss.** It never writes an entry and never
  changes the cache.
- **Quota state is tenant-scoped.** Rotating a key does not reset a counter; the
  counter belongs to the tenant, not the credential.
- **Every rejection is counted.** The metrics surface distinguishes rate-limited,
  spend-rejected, and quota-rejected requests; the response detail names the scope
  (`tenant` or `free_pool`) and window.

---

## 6. Cost containment

### 6.1 Layers

| Layer | Bounds | Failure mode if absent |
|---|---|---|
| Per-request caps | Prompt size and completion length | One request can consume a day's budget |
| Per-tenant velocity ceiling | Short bursts | A script can drain a tenant and the pool in minutes |
| Per-tenant daily/monthly budget | Sustained use | A tenant can exceed its intended monthly cost |
| Global free-pool ceiling | Aggregate exposure | Many tenants each inside their limit can still exceed the pool |
| Model allowlist | Cost per token | A free key can call the most expensive model |
| Invitation cap | Number of tenants | Signup velocity outruns the pool |

The first, second, and fifth layers exist in part today. The third and fourth exist as
opt-in controls once a tenant and the pool are enrolled; provisioning and capacity
evidence are the remaining work. The sixth is an operational control during beta: the
invite list is the real global limit until every free tenant is enrolled in the pool,
and that should be said plainly in launch material.

### 6.2 Why cache-first is not a cost control by itself

A hit avoids origin tokens, but the held-out evaluation reuses 24 of 120 legitimate
reuse opportunities (20% recall) while producing zero false hits. Savings therefore
scale with recall, and recall is deliberately conservative. A free tier planned on the
assumption of a high hit rate would be planned on a number the repository does not
support. Size the pool on misses, then treat hits as upside.

Citations: [`research/results/heldout_generalization_run.json`](../../research/results/heldout_generalization_run.json),
[`benchmarks/results/competitive_benchmark_run.json`](../../benchmarks/results/competitive_benchmark_run.json).

### 6.3 Kill switches

| Switch | Effect | Implemented today | Reversible |
|---|---|---|---|
| Revoke a tenant's keys | Tenant stops authenticating immediately | Yes | Yes, by issuing new keys |
| Empty the model allowlist | All model calls for that credential are rejected | Yes | Yes |
| Lower the tenant spend ceiling | New reservations are refused; settled spend is preserved | Yes | Yes |
| Set an enrolled tenant quota to zero | That tenant is refused at admission | Yes | Yes |
| Lower the free-pool ceiling | New requests from enrolled tenants are refused until the ceiling is raised or the window resets | Yes | Yes |
| Suspend tenant status | Tenant is refused before quota checks | No; use key revocation until it is | Yes |
| Disable free signup | No new free tenants; existing tenants continue | No; there is no signup | Yes |
| Disable the free tier entirely | Free class is refused at admission; paid traffic is unaffected | No; requires tier enforcement | Yes |

Kill switches are configuration or operator API calls. None of them changes the
correctness path, and none of them silently routes traffic through a weaker cache.

---

## 7. Abuse controls

The threat is not only a large bill. It is key farming, enumeration, and using the free
tier as a free proxy for a paid workload.

| Abuse | Control | Status |
|---|---|---|
| Key farming by one person | Verified identity at signup; one free tenant per verified identity; invite cap | Planned |
| Quota evasion by key rotation | Counters keyed to tenant, not key; rotation does not reset usage | Implemented for enrolled tenants (tenant-keyed quota rows) |
| Tenant enumeration | Uniform `401` for invalid keys; no key-existence oracle; login limiter | Implemented |
| Model-cost escalation | Per-credential model allowlist checked before dispatch | Implemented |
| Prompt flooding | Shared rate limit, body limit, per-request token caps | Rate and body limits implemented; token caps planned |
| Stream hoarding | Concurrent-stream cap per tenant | Planned |
| Credential stuffing | Login bucket, hashed keys, expiry, revocation | Implemented |
| Account takeover | Admin quorum on destructive actions; key rotation | Implemented |
| Scraping through cache | Response-size and request-volume caps; anomaly review | Planned |
| Correlated signups | Fingerprint and payment-instrument correlation at beta | Planned |

Enforcement is conservative by design: a suspected abusive tenant is suspended and
reviewed, not silently throttled into a different correctness mode. Suspension is
recorded with the reason and the operator who applied it.

---

## 8. Upgrade path

A tenant that outgrows the free tier should not have to start over.

1. **Same identity.** The tenant ID, cache namespace, and audit history are preserved.
2. **Higher limits.** Quotas and spend ceilings are raised on the existing tenant.
3. **Keys unchanged by default.** New keys are available on request; rotation is a
   separate action.
4. **Billing starts at the next period.** Usage before the upgrade is not re-priced.
5. **Support tier changes.** A paid tier may carry response targets; the free tier
   does not. Those targets are contractual only when a contract says so.
6. **Downgrade is safe.** At period end, limits return to the free envelope; data
   above the free retention window is deleted according to the published retention
   policy, not immediately.

The upgrade is not a data migration, and it must not require a cache warm-up that
changes correctness behavior. If it does, the design is wrong.

---

## 9. Invariants and non-goals

**Invariants**

- Free traffic never receives a lower safety bar, a lower freshness bar, or a
  different invalidation path.
- A quota failure is visible to the caller and counted in metrics.
- The free pool can be turned off without affecting paid tenants.
- No free-tier design depends on a hit rate the evaluation does not support.

**Non-goals**

- **No advertising or data sale.** Free traffic is not a data product.
- **No training on customer prompts or completions.** See
  [`GOVERNANCE.md`](../../GOVERNANCE.md) section 7.
- **No "unlimited" marketing.** A limit that is not enforced is not a limit.
- **No separate, weaker cache for free tenants.** The cache is shared code with
  tenant-isolated namespaces.
- **No paid priority that changes correctness.** If request prioritization is added,
  it may change queue position, never the reuse decision or freshness guarantee.

---

## 10. Work required before private beta

| Work | Evidence it is done | Owner |
|---|---|---|
| Tier/quota schema and migration | Migration applies on startup; forward/back test against a live database | Data + Backend |
| Shared rate limiter | Implemented; two-replica rehearsal showing the configured limit is cluster-wide, not doubled | Backend + SRE |
| Daily/monthly counters | Implemented; local two-replica race admitted exactly 5 of 30 requests at a 5/day limit, with durable counters at 5 | Backend |
| Enforced token caps | Request test showing a clamped completion and a rejected oversized prompt | Backend |
| Global pool counter | Implemented; ledger export reconciling aggregate usage to the pool ceiling for a full cycle (B2) | FinOps + SRE |
| Abuse review queue | Red-team report with closed findings (B3) | Security |
| Quota and outage runbooks | Runbook entries for each new failure mode | SRE + Docs |
| Load test at twice beta peak | Report with latency, error rate, and false-hit counts (B1) | SRE + Research |

This list is the free-tier portion of [`ROADMAP.md`](../../ROADMAP.md) "Next". The
schema, counters, and request pool are implemented and unit-tested. The two-replica,
load, abuse, and staging evidence is not, and no launch material should imply it is.
