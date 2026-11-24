# Sunborn Documentation (`docs/`)

Technical specifications, architectural designs, launch packages, audit registers, compliance mappings, and operational runbooks for Sunborn.

## Directory Structure

- **[`adr/`](./adr/)**: Architecture Decision Records detailing key technical and design decisions.
- **[`api/`](./api/)**: OpenAPI 3.1 contract specification (`openapi.yaml`) for gateway and control-plane endpoints.
- **[`audit/`](./audit/)**: Architectural audit registers, defect ledgers (`DEFECT_REGISTER.md`), and GitHub issue queues.
- **[`compliance/`](./compliance/)**: Regulatory and standards self-assessments (ISO 27001/27002, India DPDP Act 2023, CERT-In, RBI data localization).
- **[`frontend/`](./frontend/)**: Frontend WCAG 2.2 AA specialist accessibility audits and conformance matrices.
- **[`integrations/`](./integrations/)**: AI agent and pipeline integration guides (LangChain, LlamaIndex, crewAI).
- **[`launch/`](./launch/)**: Launch readiness package (launch plan, free-tier architecture, capacity & SLOs, trust center, prelaunch checklist).
- **[`runbooks/`](./runbooks/)**: Production runbooks for operators (pilot onboarding, operations, disaster recovery, troubleshooting, incident playbooks).
- **[`security/`](./security/)**: Threat model, STRIDE security assessments, penetration testing scope, and authorization matrix.

## Authoritative Documentation

- **[`STATUS.md`](./STATUS.md)**: Authoritative single source of truth for release verdicts, verified gates, and test counts.
- **[`ENTERPRISE_ARCHITECTURE_DOSSIER.md`](./ENTERPRISE_ARCHITECTURE_DOSSIER.md)**: End-to-end enterprise system architecture, data plane vs control plane boundaries, and fail-closed invariants.
- **[`21_QUALITY_ATTRIBUTES_VERIFICATION.md`](./21_QUALITY_ATTRIBUTES_VERIFICATION.md)**: Formal attestation and empirical verification across all 21 system quality attributes.
- **[`INSTALL.md`](./INSTALL.md)**: Complete deployment and installation guide across desktop, local in-memory, Docker, and Kubernetes.
- **[`QUICKSTART.md`](./QUICKSTART.md)**: 60-second quickstart guide for sending cached chat completions.
