# Contributing to Sunborn

Thanks for taking the time to contribute. This project is an execution gateway, so a small change can affect
correctness, cost accounting, or reuse safety for every tenant on the instance. The rules below exist to keep
that manageable.

Two principles outrank everything else here:

1. **Every claim must trace to an artifact.** If a number appears in a document, a JSON file, a test, or a
   command in this repository must produce it.
2. **[`docs/STATUS.md`](docs/STATUS.md) is the single source of truth** for what is verified, what is
   blocked, and what the release verdict is. Badges, README prose, and older audit documents yield to it.

## Getting set up

| Tool | Version | Needed for |
| :--- | :--- | :--- |
| Rust | 1.80+ (`rust-toolchain` respected) | all crates |
| Node.js | 20+ | `frontend/` |
| Python | 3.10+ | SDK, research harnesses, doc checks |
| PyYAML | 6.0+ | manifest pre-flight gates ([`requirements-dev.txt`](requirements-dev.txt)) |
| Docker | any recent | optional: `scripts/test_distributed_invalidation.sh` (the six `DIST_MODE` runs) |
| Helm | 3.x | optional: chart lint gate |

```bash
git clone https://github.com/Monkey0-9/Sunborn.git
cd Sunborn
cargo build --workspace --all-targets
npm install --prefix frontend
python -m pip install -r requirements-dev.txt
python -m pip install -e "sdk/python[dev]"
```

No external services are required for local development: `SUNBORN_ENV=development` runs the gateway fully
in memory with mock origins. See [`.env.example`](.env.example) and [`docs/QUICKSTART.md`](docs/QUICKSTART.md).

## Before you open a pull request

Run the narrow checks first, then the full gate if your change is broad:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
npm test --prefix frontend
python scripts/verify_doc_paths.py --strict
```

Full pipeline, all 12 gates (needs Docker for some):

```bash
bash scripts/verify.sh          # or: powershell -ExecutionPolicy Bypass -File scripts/verify.ps1
```

A pull request is green when those pass. If a gate cannot run in your environment (for example, no Docker
daemon), say which ones you skipped in the PR description — do not report a skipped gate as passing.

## How to make a change that will be accepted

**Bug fixes** need a regression test that fails before the fix and passes after. Where the defect was a
silent failure mode (an error swallowed into an empty set, an acknowledgement never sent), the test must
assert the failure is now visible, not merely that the happy path works.

**Behaviour changes** need to touch, in the same pull request:

- the implementation,
- the tests,
- [`docs/api/openapi.yaml`](docs/api/openapi.yaml) if the HTTP surface changed (gate 5 asserts zero drift),
- the router access table if a route was added or its access model changed (the build fails otherwise),
- [`docs/STATUS.md`](docs/STATUS.md) if a verified check or its result changed.

**New claims or benchmarks** need the artifact that produces them committed next to the tool that generates
it. Hand-typed numbers in a document, a printed confusion matrix, or a percentage copied from a previous run
will be treated as a defect: the 2026-11-17 audit withdrew a release over exactly that
([AUD-04](docs/engineering-audit-2026-11-17/REPORT.md)).

We would rather merge a smaller change with an honest limitation than a larger one with an unverified
superlative.

## Code style

- `rustfmt` decides formatting; `clippy` with `-D warnings` decides lints. Both are enforced, so run them
  rather than arguing with them.
- No `unsafe` outside the reviewed FFI wrappers around the C kernels in `crates/*/c`; those wrappers must
  keep their parity tests against the pure-Rust reference.
- Errors are values (`thiserror` / `anyhow` at the boundary). Do not panic in request paths, and do not
  convert a backend error into an empty result.
- Log with `tracing` and structured fields; never log prompts, API keys, or tenant secrets.
- Public functions get doc comments that state the invariant they preserve, not just what they do.

## Commits and pull requests

Commit subjects follow the repository's existing conventional style, scoped to the area touched:

```
feat(gateway): cluster-wide spend ceiling via advisory-locked ledger
fix(invalidation): abort instead of reporting purged on graph backend failure
docs(status): record the 320-pair held-out result and its interval
perf(semantic): clone only top-K winners in vector search
```

One logical change per commit. In the PR description, explain **why** the change is needed, what evidence
supports it, and what you could not verify. The checklist in
[`.github/PULL_REQUEST_TEMPLATE.md`](.github/PULL_REQUEST_TEMPLATE.md) covers the architectural invariants —
read it before writing code, not after.

## Documentation rules

- Backticked repository paths must exist; `scripts/verify_doc_paths.py --strict` enforces it.
- Do not reference `file:///` paths or absolute directories from your machine.
- Mark historical documents as historical rather than editing their conclusions. If new evidence contradicts
  an old document, correct the current document and add a pointer, the way
  [`RELEASE_NOTES.md`](RELEASE_NOTES.md) records its own rescission.
- Do not add team-size, funding, adoption, or comparative-superiority claims. Nothing in the repository can
  substantiate them, and unverifiable claims undermine the ones that are checkable.

## Reporting a security issue

Not here. See [`SECURITY.md`](SECURITY.md).

## Code of conduct

Participation is covered by [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).
