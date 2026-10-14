# Sunborn Reference Architecture

## 1. Architectural principles

* Correctness comes from authoritative knowledge/state, not cache presence.
* Cache failure must never become application failure.
* Security scope is part of identity and filtering.
* Derived systems must be rebuildable.
* Start modular; extract services only after measured boundaries.
* Every state transition is observable and auditable where policy requires.

## 2. Logical layers

### Experience layer

Next.js control plane, CLI, SDKs and external clients.

### Edge layer

TLS termination, gateway, authentication, authorization, quotas, rate limiting, validation and request IDs.

### Intelligence layer

Canonical request engine, cache identity, semantic retrieval, reranking, risk/confidence/freshness evaluation, admission and execution reuse.

### Data layer

PostgreSQL, Dragonfly, Qdrant, object storage, optional Neo4j.

### Event layer

Redpanda/Kafka-compatible event log and consumers.

### Origin layer

RAG retrieval, LLM/model providers and authorized tools.

### Platform layer

OpenTelemetry, Prometheus, Grafana, Jaeger, CI/CD, containers, Kubernetes and backup/recovery.

## 3. End-to-end request

Client → Edge → AuthN/Z → Tenant → Security → Canonical → Identity → Exact → Semantic → Rerank → Decision → Safe Hit / Revalidate / Partial Reuse / Origin → Validation → Admission → Persistence/Lineage → Response → Telemetry.

## 4. Cache miss/origin

Origin execution returns validated structured result + provenance + dependencies + versions. Admission decides whether each artifact is cacheable and for how long.

## 5. Invalidation

Source change → durable event → idempotent consumer → dependency lookup → affected artifact set → invalidate/revalidate → telemetry.

## 6. Frontend boundary

Frontend reads through APIs; it cannot mutate cache correctness state directly. Sensitive administrative actions require authorization, audit and confirmation.

## 7. Service extraction triggers

Extract only for measurable performance, scaling, fault isolation, security or deployment reasons.
