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
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

---

## 4. Running the Test Suites

Execute all workspace tests:

```bash
# 1. Rust unit, integration, and doc-tests
cargo test --workspace

# 2. Next.js 15 frontend tests (BFF, CSRF, Layout, Simulator)
npm --prefix frontend test

# 3. Python SDK tests
python -m unittest discover -s sdk/python

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
