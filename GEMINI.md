# Workspace Security & Engineering Governance

This document establishes mandatory operational rules, security contracts, and engineering standards across this environment for Gemini CLI and Antigravity agents.

---

## 1. Security & Operational Permissions Baseline

### 1.1 Non-Root Execution & Privilege Escalation
* **Non-Root Execution**: Agents operate strictly as non-root user `unknownuser` (UID 1000). Running commands as `root` (UID 0) is forbidden.
* **No Privilege Escalation**: Agents must never invoke `sudo`, modify `/etc/sudoers`, manipulate SUID binaries, or attempt container breakouts.

### 1.2 Destructive Operations Approval Gate
The following destructive actions **REQUIRE EXPLICIT HUMAN APPROVAL** before execution:
* **Filesystem Deletions**: Executing `rm -rf` on non-scratch or non-temporary directories, mass file removals, or raw block operations (`dd`, `mkfs`, `fdisk`).
* **Database Destruction**: Executing `DROP DATABASE`, `DROP TABLE`, `TRUNCATE`, or bulk un-indexed deletions.
* **Git History Destruction**: Force-pushing (`git push -f`, `git push --force`), dropping stashes, or resetting commits on shared branches (`main`, `master`, `release`).

### 1.3 Production Writes & Deployments Approval Gate
* Direct deployments to production clusters, production database mutations, or package publications (`npm publish`, `cargo publish`, `twine upload`) must never occur autonomously.
* All production releases require human sign-off with clear changelogs and verified rollback procedures.

### 1.4 Secrets, SSH Keys & Sensitive File Protection
* **Isolation**: Files matching `~/.ssh/**`, `**/.env*`, `**/*.pem`, `**/*.key`, `**/*secret*`, `**/*credential*` are isolated and ignored via `.geminiignore`.
* **Zero-Leakage Policy**: Agents must never print, echo, transmit, or commit private keys, passwords, bearer tokens, or API secrets.
* **Environment Protection**: Avoid executing raw `env` or `printenv` without redaction. Sensitive tokens in memory or subprocesses must remain masked.

### 1.5 Least-Privilege Filesystem & Network Access
* File modifications are strictly confined to the active project workspace.
* Modification of root system directories (`/etc`, `/usr`, `/var`, `/sys`, `/proc`) is prohibited.
* Bypassing local firewall or opening unauthorized listening ports on `0.0.0.0` is prohibited.

### 1.6 Supply-Chain Security & Dependency Auditing
* Always use deterministic lockfiles (`package-lock.json`, `uv.lock`, `.skill-lock.json`).
* Scan dependencies for vulnerabilities and use Semgrep static analysis before introducing new third-party libraries.

### 1.7 Audit Logging & Traceability
* Every executed tool, shell command, and configuration change is logged with full context in CLI history and audit logs.

---

## 2. Engineering Standards & Workflows

### 2.1 Architecture & Systems Design
* Use the `/architect` command and Sequential Thinking MCP to evaluate trade-offs (latency, throughput, operational complexity, failure modes) before implementing non-trivial subsystems.
* Query `context7` MCP for authoritative library and framework documentation.

### 2.2 Test-Driven Development (TDD)
* Follow the strict Red-Green-Refactor loop via `/tdd`:
  1. **RED**: Write a minimal failing test that captures the requirement or bug.
  2. **GREEN**: Implement the minimal code required to pass the test.
  3. **REFACTOR**: Eliminate duplication and verify edge cases and types.

### 2.3 UI/UX & Frontend Engineering
* Adhere to intentional visual design via `/hero-ui` and the `frontend-design` playbook.
* Ensure responsive layouts (375px to 1440px), WCAG accessibility compliance, `tabular-nums` for numeric displays, micro-interactions, and full interactive states (Loading, Error, Empty, Focus-Visible).
* Verify rendering and accessibility trees with headless Playwright.

### 2.4 Database Engineering & Migrations
* Use Expand-and-Contract patterns for zero-downtime database migrations.
* Validate queries with `EXPLAIN (ANALYZE, BUFFERS)` and ensure foreign-key indexes exist to avoid table locks and sequential scans.

### 2.5 DevSecOps & Code Review
* Run `/sec-audit` on all git diffs to check against the OWASP Top 10, IDOR, authorization flaws, and secrets.
* Request peer reviews before merging to protected branches.
