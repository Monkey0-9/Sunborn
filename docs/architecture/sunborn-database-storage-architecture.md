# Sunborn Database & Storage Architecture

# Database & Storage Architecture

## System-of-record decision

PostgreSQL is authoritative for durable control-plane state and metadata.

## PostgreSQL domains

tenants
users
roles
applications
api_credentials_metadata
policies
cache_entries
cache_entry_versions
request_fingerprints
model_registry
embedding_registry
reranker_registry
tool_registry
knowledge_sources
documents
document_versions
dependency_edges_metadata
execution_runs
execution_artifacts_metadata
decision_records
audit_events
invalidation_state
experiments
experiment_runs_metadata
benchmark_runs_metadata
release_metadata

## Cache payload stores

Dragonfly: exact/ephemeral key-value payloads and coordination primitives.
Qdrant: derived semantic vectors and searchable payload metadata.
Object storage: large raw outputs, benchmark data, replay payloads, reports and experiment artifacts.
Neo4j: derived dependency/lineage graph if benchmarked as superior for target workloads.
Redpanda: durable event transport/replay log, not source of truth.

## Rules

* Never store vectors in PostgreSQL when Qdrant owns the vector index.
* Never store large benchmark files in transactional tables.
* Never make an event log the source of truth.
* Every derived index has rebuild/reconciliation procedure.
* Tenant and security scope must exist on every shared data path.

## Core relational relationships

Tenant 1→N Applications
Tenant 1→N Users/Roles
Application 1→N Requests
Request 1→N Decisions
Decision 0→N Cache Entries
Cache Entry 1→N Versions
Cache Entry N→N Knowledge Dependencies
Document 1→N Document Versions
Document Version N→N Cache Entries
Execution Run 1→N Artifacts
Artifact N→N Dependencies
Model/Embedding/Reranker/Tool registries provide version compatibility state.

## Migration

Versioned migrations, backward-compatible rollout where practical, rollback plan, migration CI and restore test before production.
