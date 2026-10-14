# Sunborn CI/CD & Supply-Chain Pipeline

# CI/CD Pipeline

## Pull request pipeline

Checkout → dependency cache → formatting → lint → unit tests → property tests → integration tests → contract tests → frontend build/test → SAST/CodeQL → dependency audit → secret scan → container build → Trivy/container scan → benchmark smoke → artifact provenance → PR review.

## Merge pipeline

Protected main → merge queue → full test suite → package immutable artifacts → publish non-production artifacts → update changelog metadata.

## Environment promotion

Local → CI ephemeral → Staging → Production-like acceptance → Production.

Promotion requires:

* green required checks
* migration compatibility
* benchmark regression review
* security status
* signed/versioned artifacts
* rollback plan

## Deployment strategy

Helm/Kubernetes with immutable images, health/readiness/liveness probes, resource requests/limits, PDB, network policies, secrets, autoscaling and graceful shutdown.

## Supply chain

Pin dependencies where appropriate, generate SBOM, scan dependencies/images, review vulnerable dependencies, sign release artifacts when infrastructure permits.

## Rollback

Versioned application/container/config packages; backward-compatible DB changes where possible; cache/index rebuild process; event replay; documented rollback runbook.

## Quality gates

No merge on required-check failure.
Performance-sensitive changes require benchmark evidence.
Security-sensitive changes require security evidence.
Research changes require experiment linkage.
