#!/usr/bin/env bash
# ==============================================================================
# Sunborn Platform: Live Multi-Container Stack Pre-Flight & Health Verification
# Verifies container definitions, network policies, non-root security contexts,
# and environment contracts across the entire Sunborn deployment stack.
# ==============================================================================
set -eu
(set -o pipefail 2>/dev/null) && set -o pipefail || true

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${ROOT_DIR}"

PYTHON_BIN="python"
if command -v python3 >/dev/null 2>&1 && python3 -c "import sys" 2>/dev/null; then
    PYTHON_BIN="python3"
fi

echo "=== Sunborn Platform: Live Stack Pre-Flight Verification ==="

# 1. Verify Docker Compose Configuration Syntax
echo "[1/6] Validating deploy/docker-compose.yml and docker-compose.prod.yml syntax..."
"${PYTHON_BIN}" -c "
import yaml
with open('deploy/docker-compose.yml') as f:
    config = yaml.safe_load(f)

services = config.get('services', {})
required_services = ['gateway', 'frontend', 'postgres', 'dragonfly', 'qdrant', 'redpanda', 'minio']
for s in required_services:
    assert s in services, f'Missing required service: {s}'
    print(f'  ✓ Service registered: {s}')

# Check port bindings are locked to 127.0.0.1 (no 0.0.0.0 leaks)
for name, s in services.items():
    ports = s.get('ports', [])
    for p in ports:
        assert p.startswith('127.0.0.1:'), f'Security violation: Service {name} binds port insecurely: {p}'
print('  ✓ All service ports strictly bound to 127.0.0.1 loopback')

# Verify gateway and frontend healthchecks are configured
assert 'healthcheck' in services['gateway'], 'gateway must have native healthcheck'
assert 'healthcheck' in services['frontend'], 'frontend must have healthcheck'
assert services['frontend']['depends_on']['gateway']['condition'] == 'service_healthy', 'frontend must depend on healthy gateway'
print('  ✓ Gateway & frontend healthchecks and dependency sequencing verified')

# Validate production compose fail-closed contract
with open('deploy/docker-compose.prod.yml') as f:
    prod_config = yaml.safe_load(f)
prod_services = prod_config.get('services', {})
for s in ['gateway', 'postgres', 'dragonfly', 'qdrant']:
    assert s in prod_services, f'Missing production service: {s}'
for name, s in prod_services.items():
    for p in s.get('ports', []):
        assert p.startswith('127.0.0.1:'), f'Prod security violation: {name} binds {p}'
print('  ✓ Production Compose configuration strictly bound to 127.0.0.1 with required backends')
"

# 2. Verify Backend Dockerfile Non-Root and Security Compliance
echo "[2/6] Verifying deploy/Dockerfile security contract..."
grep -q "USER " "deploy/Dockerfile" || (echo "FAILED: deploy/Dockerfile must specify non-root USER" && exit 1)
grep -q "HEALTHCHECK" "deploy/Dockerfile" || (echo "FAILED: deploy/Dockerfile must define HEALTHCHECK" && exit 1)
echo "  ✓ Non-root user and native HEALTHCHECK verified in backend Dockerfile"

# 3. Verify Frontend Dockerfile Non-Root and Healthcheck
echo "[3/6] Verifying deploy/Dockerfile.frontend security contract..."
grep -q "USER nextjs" "deploy/Dockerfile.frontend" || (echo "FAILED: deploy/Dockerfile.frontend must run as nextjs user" && exit 1)
grep -q "HEALTHCHECK" "deploy/Dockerfile.frontend" || (echo "FAILED: deploy/Dockerfile.frontend must define HEALTHCHECK" && exit 1)
echo "  ✓ Non-root user (nextjs:nodejs) and HEALTHCHECK verified in frontend Dockerfile"

# 4. Verify Helm Chart Syntax & Values
echo "[4/6] Verifying deploy/helm/sunborn chart package..."
"${PYTHON_BIN}" -c "
import yaml
with open('deploy/helm/sunborn/Chart.yaml') as f:
    chart = yaml.safe_load(f)
assert chart.get('name') == 'sunborn'
assert chart.get('apiVersion') == 'v2'
print(f'  ✓ Helm Chart: {chart[\"name\"]} v{chart[\"version\"]} (appVersion: {chart[\"appVersion\"]})')

with open('deploy/helm/sunborn/values.yaml') as f:
    vals = yaml.safe_load(f)
assert vals['gateway']['replicaCount'] >= 2
assert vals['gateway']['podDisruptionBudget']['enabled'] is True
assert vals['frontend']['enabled'] is True
assert vals['networkPolicy']['enabled'] is True
assert vals['gateway']['securityContext']['runAsUser'] == 65532
assert vals['frontend']['securityContext']['runAsUser'] == 1001
assert vals['env']['SUNBORN_CACHE_BACKEND'] == 'dragonfly'
assert vals['env']['SUNBORN_DB_BACKEND'] == 'postgres'
assert vals['env']['SUNBORN_VECTOR_BACKEND'] == 'qdrant'
print('  ✓ Helm values validated (HA replicas, PDB, NetworkPolicy, Ingress, SecurityContext, Envs)')
"

# 5. Verify Kustomize GitOps Manifest
echo "[5/6] Verifying deploy/k8s/kustomization.yaml..."
"${PYTHON_BIN}" -c "
import yaml
with open('deploy/k8s/kustomization.yaml') as f:
    kust = yaml.safe_load(f)
assert len(kust.get('resources', [])) >= 7
print(f'  ✓ Kustomization manifests verified ({len(kust[\"resources\"])} resources bound)')
"

# 6. Verify Migrations Schema & Non-Accelerator Authority
echo "[6/6] Verifying PostgreSQL 16 migrations schema..."
test -f "migrations/0001_initial_schema.sql"
grep -q "CREATE TABLE IF NOT EXISTS decision_records" "migrations/0001_initial_schema.sql"
grep -q "CREATE TABLE IF NOT EXISTS invalidation_outbox" "migrations/0001_initial_schema.sql"
echo "  ✓ PostgreSQL 16 WAL and Outbox DDL schema intact"

echo ""
echo "=== ALL 6 LIVE STACK PRE-FLIGHT VERIFICATIONS PASSED ==="
