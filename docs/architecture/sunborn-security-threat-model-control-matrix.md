# Sunborn Security Threat Model & Control Matrix

# Security Threat Model

## Assets

User/request data, cached answers, retrieved knowledge, tool results, credentials, tenant metadata, model/provider metadata, audit events and experiment artifacts.

## Threats

Prompt injection
Indirect prompt injection
Semantic cache poisoning
Embedding/semantic collision
Cross-tenant access
SSRF/tool abuse
Secret leakage
PII exposure
Malicious documents
Event tampering/replay
Unauthorized administration
Dependency supply-chain compromise
Stale sensitive information

## Controls

AuthN/AuthZ
RBAC/ABAC
Tenant isolation
Input validation
URL/tool allowlists
Secret/PII classification
Security-aware cache identity
Metadata filtering before semantic retrieval
Provenance and dependency validation
Signed/versioned artifacts where practical
Audit logs
Encryption in transit/at rest where supported
SAST/CodeQL
Dependency scanning
Secret scanning
Container scanning
Adversarial regression suite
Chaos/failure testing

## Security decision rule

Similarity never overrides security policy.

## Attack/evidence workflow

Threat → fixture → exploit measurement → mitigation → regression test → retest → close.

## Required release evidence

No unresolved critical/high security issue; tenant-isolation suite passes; secret/PII handling validated; supply-chain scans reviewed; security runbook exists.
