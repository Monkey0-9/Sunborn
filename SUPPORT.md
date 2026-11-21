# Support

## Read this first

Most questions are answered by an existing document. The current state of the project — what is verified,
what is blocked, and what the release verdict is — lives in **[`docs/STATUS.md`](docs/STATUS.md)**. If a
badge, blog post, or older audit document disagrees with it, `STATUS.md` is correct.

| I want to… | Start here |
| :--- | :--- |
| Run it locally in a few minutes | [`docs/QUICKSTART.md`](docs/QUICKSTART.md) |
| Understand the architecture | [`README.md`](README.md) §6–§9 |
| Know exactly what the numbers mean | [`README.md`](README.md) §4, then the linked artifact |
| Know what the project does *not* claim | [`README.md`](README.md) §5 |
| Configure it | [`.env.example`](.env.example) |
| Integrate from Python | [`sdk/python/README.md`](sdk/python/README.md) |
| Deploy to Kubernetes | [`deploy/`](deploy/), [`docs/runbooks/`](docs/runbooks/) |
| Audit the HTTP surface | [`docs/api/openapi.yaml`](docs/api/openapi.yaml) |
| See the assurance evidence | [`docs/STATUS.md`](docs/STATUS.md) §1 (commands and results), [`docs/audit/DEFECT_REGISTER.md`](docs/audit/DEFECT_REGISTER.md) |
| Report a vulnerability | [`SECURITY.md`](SECURITY.md) — **not** a public issue |
| Contribute a change | [`CONTRIBUTING.md`](CONTRIBUTING.md) |

## Asking a question

Open a GitHub issue and choose the template that fits (`bug_report`, `feature_request`, or
`security_advisory`). For a usage question, a plain issue with a clear title is fine.

A report that gets answered quickly contains:

1. **Version** — the commit SHA or container digest you are running.
2. **Mode** — `SUNBORN_ENV`, plus which backends are configured (memory, Postgres, Dragonfly, Qdrant).
3. **What you ran** — the exact command or HTTP request, with secrets redacted. Redact API keys, prompt
   content, and tenant identifiers before pasting logs.
4. **What happened vs. what you expected** — including the `decision.cache_path` and `decision.action`
   fields if the request went through the gateway, since those say whether reuse was proposed or refused.
5. **What you already tried.**

## Known limitations you might hit

These are expected behaviours, not bugs:

- **Low semantic hit rate on real traffic.** Held-out recall is 20% by design: the guardrail abstains rather
  than risk a wrong reuse. If you expected a higher hit rate, the guardrail is behaving as measured, not
  malfunctioning. See [`README.md`](README.md) §5.
- **The gateway refuses to start in production mode** without PostgreSQL, the cache backend, the vector
  store, and distinct keys. That is the fail-closed design. Use `SUNBORN_ENV=development` for a standalone
  evaluation.
- **Docker-gated checks are skipped** when no daemon is present. The distributed invalidation modes
  (`exact`, `semantic`, `replay`, `outbox`, `finops`, `crash`) need containers.
- **Windows-specific quirks**: scripts are run through Git Bash; `scripts/verify.ps1` is the PowerShell path.

## What support this project does not provide

There is no published SLA, no commercial support contract, and no hosted service operated by this
repository. The project is at **CONDITIONAL GO — controlled pilot**; production use with sensitive data is
gated on an external penetration test and a live multi-replica validation run
([`docs/STATUS.md`](docs/STATUS.md) §3). Any arrangement with a response-time guarantee would have to be
negotiated separately — nothing here should be read as promising one.

## Security issues

Never report a vulnerability in a public issue or discussion. Follow [`SECURITY.md`](SECURITY.md).
