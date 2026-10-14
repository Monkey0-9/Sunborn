# Sunborn Testing, Quality & Verification Strategy

# Testing and Verification Strategy

## Test pyramid

### Unit

Pure domain logic, scoring, canonicalization, serialization, state transitions.

### Property

Canonical equivalence, deterministic identity, idempotency, event replay and invariants.

### Contract

HTTP/OpenAPI, JSON schemas, event envelopes and frontend API compatibility.

### Integration

PostgreSQL, Dragonfly, Qdrant, object storage, event system, model/tool adapters.

### End-to-end

Origin request, exact hit, semantic hit, revalidation, partial execution reuse, frontend control path.

### Security

AuthZ, tenant isolation, prompt injection, indirect injection, SSRF/tool abuse, poisoning and semantic collision.

### Performance

Latency, throughput, concurrency, saturation, memory and CPU/GPU behavior.

### Reliability

Dependency outages, timeouts, partial writes, event duplication/reordering, restore and replay.

### Research

Baseline parity, experiment reproducibility, statistical analysis and ablation.

## Gate matrix

Feature: unit + relevant integration + docs.
Data migration: migration CI + rollback/restore.
Cache/decision change: correctness + benchmark.
Security change: security suite + reviewer.
Research change: experiment + evidence.
Release: all mandatory gates.

## Regression principle

A test should be added whenever a bug, security finding, invalidation error or research failure reveals a previously unprotected invariant.

## Evidence retention

Store test report, environment/version metadata and relevant benchmark/security artifact with the Linear issue.
