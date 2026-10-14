# Sunborn PostgreSQL ERD & Data Ownership Specification

# PostgreSQL Control Plane — ERD and Ownership

## Authoritative entities

Tenant
User
Role
Permission
Application
Policy
API credential metadata
Request
Canonical request metadata
Cache entry
Cache entry version
Decision record
Execution
Execution artifact metadata
Knowledge source
Document
Document version
Chunk
Model
Embedding model/version
Reranker model/version
Tool
Tool version
Invalidation event metadata
Audit event
Experiment
Experiment run
Benchmark run
Release

## Relationship model

Tenant 1:N Application
Tenant 1:N User
User N:M Role
Role N:M Permission
Application 1:N Request
Request 1:N Decision
Decision 0:N CacheEntry
CacheEntry 1:N CacheEntryVersion
CacheEntry N:M KnowledgeDependency
Document 1:N DocumentVersion
DocumentVersion 1:N Chunk
Execution 1:N Artifact
Artifact N:M KnowledgeDependency
Model/Embedding/Reranker/Tool versions participate in compatibility records
InvalidationEvent N:1 source version and references affected artifact state
Experiment 1:N ExperimentRun
ExperimentRun 1:N Benchmark/Artifact references

## Not in PostgreSQL

Large vectors → Qdrant
Exact cache payloads → Dragonfly
Large experiment/replay files → object storage
Durable event stream → Redpanda
Dependency graph → Neo4j only after benchmark evidence

## Tenancy

Tenant ID is mandatory in every tenant-scoped table and API query. Application/service authorization is enforced before data access. Add database-level defense in depth where practical.

## Indexing

Index tenant + lookup identity, status, timestamps, version fields and foreign-key paths used by request/decision/invalidation queries. Query plans must be benchmarked before production.

## Migrations

Expand/contract migrations, compatibility window, migration CI, rollback/recovery procedure and restore rehearsal.
