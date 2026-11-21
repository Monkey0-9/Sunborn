# Installing Sunborn

Three supported paths. Each one ends with the same proof: the binary loads your
configuration, answers `/health`, and reports its own build identity.

If you only read one line: after installing, run

```bash
sunborn-gateway doctor
```

`doctor` verifies configuration, durable-storage writability, port availability,
and key-material presence, then tells you the single next command to run.

---

## 1. Local desktop (single user, BYOK)

The intended path for an individual developer: a loopback-bound gateway with an
encrypted local key vault.

### Windows

```powershell
powershell -ExecutionPolicy Bypass -File scripts/install-sunborn.ps1 -LaunchAfterInstall
```

Installs to `%LOCALAPPDATA%\Sunborn`, creates Desktop and Start Menu shortcuts,
registers an uninstaller in Add/Remove Programs, then runs the smoke test below.

### Linux / macOS

```bash
bash scripts/install-sunborn.sh
```

Installs to `~/.local/bin/sunborn-gateway`, with data under
`~/.local/share/sunborn` and a desktop entry when `~/.local/share/applications`
exists.

### What the installer proves before it says "complete"

1. `sunborn-gateway config-check` succeeds.
2. The binary is started on a private port (`18081` by default) and must answer
   `/health` within 10 seconds.
3. The process is stopped again.

If step 2 fails, the installer prints the log tail and the exact `doctor`
command to diagnose it. "Installed" therefore means "runs", not "files copied".

Start it afterwards:

```bash
SUNBORN_SERVER__HOST=127.0.0.1 sunborn-gateway
```

The local cockpit is served at <http://127.0.0.1:8080/>.

---

## 2. From source (development and CI)

```bash
cargo build --workspace
cargo run -p sunborn-gateway
```

Defaults are safe for development: `127.0.0.1:8080`, in-memory cache, vector, and
database backends. Copy `.env.example` to `.env` for a configured profile.

Verify the tree itself:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

---

## 3. Container / Kubernetes (production)

```bash
docker compose -f deploy/docker-compose.yml up -d
```

Kubernetes, plain manifests:

```bash
kubectl apply -k deploy/k8s
```

Helm (fails closed without real secrets):

```bash
helm install sunborn deploy/helm/sunborn \
  --set secrets.existingSecret=sunborn-secrets
python scripts/check_k8s_manifests.py
```

Production startup is strict by design. It refuses to start if any of the
following is true, rather than silently degrading:

- `SUNBORN_CACHE_BACKEND` is not `dragonfly`
- `SUNBORN_DB_BACKEND` is not `postgres`
- `SUNBORN_VECTOR_BACKEND` is not `qdrant`
- `SUNBORN_MASTER_KEK` is missing, weak, or a known placeholder
- `SUNBORN_ADMIN_KEY` is missing, default, or shorter than 16 characters
- The LLM origin provider is still the mock
- Execution or learning storage is ephemeral

### Scaling beyond one node

Two pieces of the Kubernetes overlay exist only for multi-replica operation:

**PgBouncer (`deploy/k8s/pgbouncer.yaml`).** Each gateway pod opens its own
server-side connection pool. With the HPA at its ceiling those pools sum to more
postgres backends than a managed instance's `max_connections` allows, and the
surplus queues inside the database where nothing can shed it. The pooler runs in
`transaction` mode — which matches how the gateway uses connections (one
statement or explicit transaction per request) — so 5,000 client connections
share 50 backends. Point the gateway's `DATABASE_URL` at
`sunborn-pgbouncer:6432`. The auth file is not in this repository; provision
the Secret `sunborn-pgbouncer-auth` (key `userlist.txt`) from your secret
manager, following the same rule as `sunborn-gateway-secrets`.

Session-pooling mode would break the gateway: it relies on releasing a
connection between requests, and holding one per client reproduces the
connection explosion the pooler exists to prevent.

**Qdrant collection tuning (`SUNBORN_QDRANT_*`).** `ensure_collection` now sends
an explicit HNSW graph (`m=16`, `ef_construct=128`) and `on_disk_payload: true`,
so RAM holds vectors rather than payload and query latency does not drift as the
corpus grows. Scalar int8 quantization is available with
`SUNBORN_QDRANT_QUANTIZATION=true` and cuts the resident vector footprint by
roughly 75%; it is **off by default** because its recall impact has not been
measured on this corpus. Enable it only after running the semantic evaluation
and comparing results.

```bash
SUNBORN_QDRANT_HNSW_M=16 \
SUNBORN_QDRANT_HNSW_EF_CONSTRUCT=128 \
SUNBORN_QDRANT_QUANTIZATION=false \
  sunborn-gateway
```

Cost ceilings are per process. `SUNBORN_MAX_SPEND_PER_WINDOW_USD` (default
$100 per rolling 60 s) plus the per-tenant ceilings bound one replica; a
fleet-wide cap is therefore approximately `replicas × window cap`, not a single
global number. If you need a hard global daily budget, enforce it at the edge or
in the billing layer — the gateway does not aggregate spend across pods.

---

## Operator commands

| Command | Purpose | Exit codes |
|---|---|---|
| `sunborn-gateway` | Start the gateway | 0 clean shutdown, 75 drain deadline exceeded, 1 fatal startup error |
| `sunborn-gateway config-check` | Load + validate, print redacted effective config | 0 valid, 78 invalid |
| `sunborn-gateway doctor` | Full local pre-flight with PASS/FAIL/WARN | 0 ready, 1 blocking problem, 78 invalid config |
| `sunborn-gateway healthcheck` | Probe a running instance's `/health` | 0 healthy, 1 not healthy |
| `sunborn-gateway version` | Print build identity | 0 |

`config-check` and `doctor` never print secret values; key material is reported
only as `set` or `unset`.

---

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Exit **78** with "configuration validation failed" | One or more structural problems; all are listed | Fix every bullet in the message, then re-run `config-check` |
| `doctor`: "`<addr>` is in use by another process" | Something that is not a healthy Sunborn gateway owns the port | `PORT=<other>` or stop the owner |
| `doctor`: "a healthy Sunborn gateway is already listening" | You already have an instance running | Use it, or stop it first |
| Production refuses to start: "forbids ephemeral in-memory cache backend" | `SUNBORN_ENV=production` with memory backends | Provide Dragonfly/Postgres/Qdrant, or run in development |
| Installer smoke test failed | The environment cannot bind or start the binary | Read the printed log tail, then run `doctor` |
| `SUNBORN_MASTER_KEK` rejected as weak | Placeholder or short key | Generate 32 random bytes, hex-encode, set it |

Generate a valid key:

```bash
openssl rand -hex 32
```

---

## Verifying an installation against the repository's own gates

```bash
python scripts/verify_public_launch.py   # documentation, claims, OpenAPI drift, SBOM, manifests
python scripts/check_openapi_drift.py    # gateway routes vs docs/api/openapi.yaml
python scripts/check_k8s_manifests.py    # no deployable placeholder credentials
```
