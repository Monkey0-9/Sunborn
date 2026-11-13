# 04 — Repository Inventory

**Source:** `git ls-files` (382 tracked files at `0e412f8`) + on-disk census.

---

## 1. Tracked files by top-level area

| Area | Files |
|---|---:|
| `crates/` | 111 |
| `docs/` | 103 |
| `frontend/` | 48 |
| `deploy/` | 34 |
| `research/` | 20 |
| `sdk/` | 12 |
| `scripts/` | 11 |
| `.github/` | 9 |
| `benchmarks/` | 7 |
| `migrations/` | 3 |
| `apps/` | 3 |
| `.cargo/` | 2 |
| `fuzz/` | 2 |
| `.devcontainer/` | 2 |
| root files | 15 |
| **Total** | **382** |

## 2. Rust workspace

| Crate | src LOC | tests LOC | `#[test]` count | `tests/` dir |
|---|---:|---:|---:|---|
| `sunborn-core` | 2,148 | 0 | 13 | no |
| `sunborn-identity` | 2,347 | 247 | 26 | yes |
| `sunborn-cache` | 1,384 | 0 | 9 | no |
| `sunborn-semantic` | 3,313 | 0 | 20 | no |
| `sunborn-invalidation` | 2,368 | 123 | 21 | yes |
| `sunborn-execution` | 1,692 | 0 | 15 | no |
| `sunborn-db` | 1,917 | 0 | 5 | no |
| `sunborn-gateway` | 7,519 | 8,837 | 142 | yes (23 files) |
| `apps/sunborn-rebuild` | 153 | 0 | 0 | no |
| `fuzz/` | 33 | — | — | — |
| **Total** | **~21,724** | **~9,207** | **251** | 25 binaries |

Key modules:
- `sunborn-gateway/src/`: `handlers.rs` (3003), `state.rs` (831), `middleware/auth.rs` (509), `origin/provider.rs` (491), `routing/radix.rs` (428), `origin/singleflight.rs` (300), `learning/*`, `middleware/rate_limit.rs`, `shadow.rs`, `lib.rs`, `main.rs`.
- `sunborn-semantic/src/`: `guardrails.rs` (897), `vector_store.rs` (653), `decision.rs` (588), `conformal.rs` (492), `embedding.rs` (333), `context_digest.rs`, `microbatch.rs`.
- `sunborn-invalidation/src/`: `graph.rs` (644), `outbox.rs` (464), `graph_cache.rs` (353), `cluster_sync.rs` (323), `epoch.rs` (242), `bitmap_lineage.rs` (157), `watermark.rs` (148).
- `sunborn-identity/src/`: `crypto_shred.rs` (1609), `identity.rs`, `canonical.rs`, `privacy.rs`.
- `sunborn-execution/src/`: `planner.rs` (609), `merkle.rs` (397), `transitive_closure.rs`, `store.rs`, `model.rs`, `signature.rs`.
- `sunborn-db/src/`: `postgres.rs` (1242), `write_behind.rs` (311), `repository.rs`, `models.rs`.
- `sunborn-cache/src/`: `memory.rs` (329), `gdsf.rs` (287), `dragonfly.rs` (277), `circuit_breaker.rs` (230), `tiered.rs` (148).

## 3. Frontend

- 34 source files / ~11,055 LOC; 7 test files / ~1,142 LOC.
- Routes: `/`, `/explorer`, `/knowledge`, `/execution`, `/finops`, `/governance`, `/learning`, `/security`, `/ceremony`, `/login`, error/404.
- BFF routes: `/api/proxy/[...path]`, `/api/auth`, `/api/health`.
- `src/lib/standalone-simulator.ts` (698 LOC) is a fake backend (see 15/41).

## 4. SDK

`sdk/python`: `sunborn/` package (real client + async client + models + exceptions), legacy duplicate `sunborn_agent.py`, tests (`test_sunborn_agent.py` 21, `tests/test_sdk_package.py` 5), generated `dist/`, `uv.lock`.

## 5. Docs (103 files)

`docs/master` (21), `docs/architecture` (30), `docs/verification` (15), `docs/engineering` (12), `docs/research` (7), `docs/audit` (7), `docs/runbooks` (3), `docs/operations` (2), `docs/adr` (1), `docs/api` (1), `docs/integrations` (1), `docs/security` (1), `docs/release` (1), root `QUICKSTART.md`. Four overlapping/contradictory suites (see 40).

## 6. Deploy / CI / scripts

- `deploy/`: `docker-compose.yml`, `docker-compose.prod.yml`, 2 Dockerfiles, Helm chart, raw k8s, multi-region sketches, `verify_live_stack.sh`.
- `.github/workflows`: `ci.yml`, `security.yml`, `release.yml` + templates/CODEOWNERS/branch_protection.md.
- `scripts/`: `verify.sh`, `verify.ps1`, `check_openapi_drift.py`, `check_k8s_manifests.py`, `audit_licenses.py`, `generate_sbom.py`, `ci_load_test.py`, `verify_doc_paths.py`, `start-all.sh/.ps1`, `test_python_sdk.sh`.

## 7. Untracked / on-disk but not source (junk & caches)

`target/`, `frontend/.next/`, `frontend/node_modules/`, `.mypy_cache/`, `.ruff_cache/`, `.trunk/`, `.vscode/`, `__pycache__/`, `frontend/tsconfig.tsbuildinfo`, `frontend/*.png`, `sdk/python/dist/`, `sdk/python/build/`, `sdk/python/sunborn.egg-info/`.

`.gitignore` gaps and committed artifacts are enumerated in 30/39 and 40.
