# Multi-region deployment sketches

These manifests are design sketches only. They are not approved or proven production deployment
artifacts. Both gateway workloads deliberately use a non-pullable image placeholder and the
unsupported `local-aead` provider. Production readiness rejects that provider. The ingress uses an
`.invalid` hostname and routes to the US East service only; it does not implement health-based
global failover.

Before using this directory for a real deployment, the owning team must provide and independently
review all of the following:

- A verified immutable image digest and signed build provenance for the exact candidate.
- A real cloud KMS integration using workload identity, least-privilege policies, key versions,
  rotation, remote-operation audit records, bounded retries/timeouts, and failure tests.
- Per-region secrets and identities, database topology, cache/vector replication, residency rules,
  and an explicit consistency/authority model.
- Health-checked traffic management with split-brain protection, failover and failback policy.
- A staging exercise taking the primary offline, measuring RTO, validating RPO/data integrity,
  tenant isolation, duplicate handling, and recovery.

Until that evidence exists, do not describe this directory as a functioning multi-region deployment.
