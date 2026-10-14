# Sunborn Technology & Tooling Blueprint

## Core product stack

Frontend: Next.js + TypeScript + Tailwind/shadcn-style UI
Backend: Rust + Axum
CLI: Rust
Research/benchmark automation: Python
API contract: OpenAPI/JSON Schema
Database: PostgreSQL
Exact cache: DragonflyDB
Semantic vector index: Qdrant
Large artifacts: S3/MinIO-compatible object storage
Knowledge graph: Neo4j only if validated by benchmark
Event backbone: Redpanda/Kafka-compatible log, subject to benchmark
Local model/runtime: Ollama
Container: Docker
Orchestration: Kubernetes + Helm
Infrastructure as code: Terraform where cloud resources are introduced

## Quality/security stack

GitHub Actions
CodeQL/SAST
cargo-audit
cargo-deny
Trivy/container scanning
secret scanning
dependency automation
SBOM/provenance generation

## Observability

OpenTelemetry
Prometheus
Grafana
Jaeger

## ResearchOps

DVC
MLflow
Jupyter for exploration, not as the source of truth
SciPy/statsmodels for statistical analysis
Zotero for literature
Overleaf/LaTeX for publication

## Tool selection rule

Technology is provisional until:

1. workload is specified
2. alternatives are identified
3. failure behavior is understood
4. operational cost is estimated
5. benchmark evidence exists
6. an ADR records the decision

## What each tool owns

Linear = planning/dependencies/evidence
GitHub = code/PR/release history
DVC = data versioning
MLflow = experiment runs
PostgreSQL = authoritative application metadata
Dragonfly/Qdrant/Neo4j = derived/accelerator state
Redpanda = durable event transport/replay
Object storage = large immutable artifacts
Telemetry stack = operational evidence

## Non-goals

No unnecessary microservice explosion.
No frontend as the system of record.
No unrestricted chain-of-thought persistence.
No technology added only for resume value.
No benchmark tuned only to favor the proposed method.
