# Sunborn Developer Quickstart Guide

This guide provides an end-to-end walkthrough for compiling, configuring, running, and testing the Sunborn Autonomic Semantic Cache and Gateway locally.

---

## 1. Prerequisites

- **Rust:** `1.80.0` or higher (`rustup default stable`)
- **Node.js:** `20.x` or higher (`node -v`, `npm -v`)
- **Python:** `3.10+` (`python --version`)
- **PostgreSQL / Dragonfly (Optional):** In-memory mock fallbacks are natively supported for standalone offline development.

---

## 2. Environment Setup

Copy the example environment configuration:

```bash
cp .env.example .env
```

Key environment configuration flags:

```env
# Mode: "development" or "production"
SUNBORN_ENV=development

# Server Port
PORT=8080

# Ingress & CORS
SUNBORN_CORS_ALLOWED_ORIGINS=http://localhost:3000,http://127.0.0.1:3000

# Frontier LLM Provider Credentials
ANTHROPIC_API_KEY=sk-ant-api03-...
GEMINI_API_KEY=AIzaSy...
OPENAI_API_KEY=sk-proj-...
SUNBORN_LLM_ORIGIN_URL=https://api.openai.com/v1

# Dual Caching Configuration (Graph + Vector active by default)
SUNBORN_GRAPH_DEPENDENCY_CACHE_ENABLED=true
SUNBORN_VECTOR_BACKEND=qdrant
SUNBORN_QDRANT_URL=http://127.0.0.1:6333
SUNBORN_CACHE_BACKEND=dragonfly
DRAGONFLY_URL=redis://127.0.0.1:6379

# Backend Storage
SUNBORN_DB_BACKEND=postgres
POSTGRES_URL=postgres://sunborn:sunborn@localhost:5432/sunborn
SUNBORN_DB_STRICT_DURABILITY=false

# Cryptography & Security
SUNBORN_KMS_PROVIDER=local-aead
SUNBORN_MASTER_KEK=000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f
SUNBORN_CERTIFICATE_SIGNING_KEY=a0b1c2d3e4f5a0b1c2d3e4f5a0b1c2d3e4f5a0b1c2d3e4f5a0b1c2d3e4f5a0b1
```

---

## 3. Building the Workspace

Compile the Rust workspace with strict warnings:

```bash
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

Then confirm the environment is ready before starting:

```bash
cargo run -p sunborn-gateway -- config-check
cargo run -p sunborn-gateway -- doctor
```

`config-check` exits 78 on any structural problem and prints the redacted
effective configuration. `doctor` additionally checks durable-storage
writability, port availability, and key-material presence. See
[`INSTALL.md`](INSTALL.md) for the full operator command reference.

---

## 4. Running the Test Suites

Execute all workspace tests:

```bash
# 1. Rust unit, integration, and doc-tests
cargo test --workspace

# 2. Next.js 15 frontend tests (BFF, CSRF, Layout, Simulator)
npm --prefix frontend test

# 3. Python SDK tests
bash scripts/test_python_sdk.sh

# 4. Independent certificate and benchmark reproduction verification
python research/verify_certificate.py
```

---

## 5. Running the Application Locally

### Option A: Complete Full-Stack Launch Script
Run both Gateway and Control Plane together:

```powershell
# Windows PowerShell
.\scripts\start-all.ps1
```

```bash
# Linux / macOS
./scripts/start-all.sh
```

### Option C: Broker-backed development in one command

This is the shortest path to a realistic local setup. It starts Postgres,
Dragonfly (Redis protocol), and Qdrant in Docker, writes a development
`.env.local`, waits for every dependency to report healthy, and then runs the
gateway with that wiring:

```bash
make dev            # or: bash scripts/dev-up.sh
```

Verify the request path end to end (origin miss → exact cache hit) in a second
terminal:

```bash
make smoke
```

Then start the control plane:

```bash
npm --prefix frontend run dev
```

`/ready` may report `503` with `status: degraded` when no LLM provider key is
configured. That is expected: cache, database, and vector store are connected,
but the origin is unavailable until you set a provider key. `make smoke` works
against the built-in mock origin.

`make dev` is safe to re-run: it reuses the Docker volumes and the existing
`.env.local`. Stop the brokers with `make docker-down`. If Docker is not
installed, use Option A (memory-only backends) instead — the gateway runs, but
cache, lineage, and vector state do not survive a restart.

### Option B: Individual Service Launch

**1. Launch Rust Gateway (`http://localhost:8080`):**
```bash
cargo run -p sunborn-gateway
```

**2. Launch Next.js Control Plane (`http://localhost:3000`):**
```bash
cd frontend
npm install
npm run dev
```

Visit [http://localhost:3000](http://localhost:3000) to inspect cache entries, test semantic execution DAGs, and monitor FinOps metrics.

---

## 6. Enterprise Client Walkthrough & Demonstration

To evaluate the complete 6-pillar enterprise guarantees in an isolated sandbox or against a live gateway:

```bash
# Execute the self-contained 6-pillar client evaluation suite
python scripts/client_showcase.py --self-test

# Run the Python SDK client tour in dry-run mode
python sdk/python/examples/enterprise_client_tour.py --dry-run
```

For client integrations in TypeScript / Node.js, see [`sdk/typescript/client_example.ts`](../sdk/typescript/client_example.ts).

---

## 7. Where to go next

| | |
| :--- | :--- |
| Enterprise Systems Architecture & Resilience Dossier | [`ENTERPRISE_ARCHITECTURE_DOSSIER.md`](ENTERPRISE_ARCHITECTURE_DOSSIER.md) |
| What the system actually is, and what it measures | [`README.md`](../README.md) §1–§5 |
| Configuration and the fail-closed production mode | [`../.env.example`](../.env.example), [`README.md`](../README.md) §2 |
| What is verified right now, and what is still blocked | [`STATUS.md`](STATUS.md) |
| Questions, expected behaviours, and known limitations | [`../SUPPORT.md`](../SUPPORT.md) |
| Installing (desktop, source, containers) | [`INSTALL.md`](INSTALL.md) |
| Reporting a vulnerability | [`../SECURITY.md`](../SECURITY.md) |
| Sending a change | [`../CONTRIBUTING.md`](../CONTRIBUTING.md) |

> Running a full evaluation? `bash scripts/verify.sh` executes all 12 gates from the repository root.
> Gate 1 alone (Rust tests and clippy) is enough for a routine change.
