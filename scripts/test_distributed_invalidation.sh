#!/usr/bin/env bash
# ==============================================================================
# Two-process cross-replica invalidation test.
#
# Replica A and replica B are independent gateway processes. They share Redis
# Streams (invalidation bus), PostgreSQL (epoch authority/outbox) and Qdrant
# (vector store), but each has its own process-local exact cache and dependency
# graph. A mutation on A must purge B's local cache through the durable stream,
# even though B never recorded the source-to-key dependency.
# ==============================================================================
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

COMPOSE_FILE="deploy/docker-compose.distributed-test.yml"
PROJECT="sunborn-dist-test"
ADMIN_KEY="dist-tenant-admin-key"
MASTER_KEK="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
HMAC_SECRET="dist-test-learning-hmac-secret-32-bytes"
ATTESTATION_SEED="1111111111111111111111111111111111111111111111111111111111111111"
SOURCE_ID="aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee"
PROMPT="Distributed invalidation acceptance probe: summarize the Q3 ledger."
TENANT="tenant-alpha"
APP="app-dist-test"
MODEL="gpt-4o"

A_PORT=18080
B_PORT=18081
A_LOG="${ROOT_DIR}/target/distributed-replica-a.log"
B_LOG="${ROOT_DIR}/target/distributed-replica-b.log"
A_PID=""
B_PID=""

cleanup() {
    set +e
    [ -n "${A_PID}" ] && kill "${A_PID}" 2>/dev/null
    [ -n "${B_PID}" ] && kill "${B_PID}" 2>/dev/null
    wait "${A_PID}" 2>/dev/null
    wait "${B_PID}" 2>/dev/null
    if [ "${KEEP_STACK:-0}" != "1" ]; then
        docker compose -p "${PROJECT}" -f "${COMPOSE_FILE}" down -v --remove-orphans >/dev/null 2>&1
    fi
}
trap cleanup EXIT

fail() {
    echo "FAIL: $*" >&2
    echo "--- replica A log (tail) ---" >&2
    tail -60 "${A_LOG}" >&2 2>/dev/null || true
    echo "--- replica B log (tail) ---" >&2
    tail -60 "${B_LOG}" >&2 2>/dev/null || true
    exit 1
}

wait_http() {
    local url="$1" name="$2"
    for _ in $(seq 1 60); do
        if curl -fsS "${url}" >/dev/null 2>&1; then
            return 0
        fi
        sleep 1
    done
    echo "readiness response body:" >&2
    curl -sS "${url}" >&2 2>/dev/null || true
    echo >&2
    fail "${name} did not become reachable at ${url}"
}

echo "=== 1/7: build gateway binary ==="
cargo build -p sunborn-gateway
BIN="${ROOT_DIR}/target/debug/sunborn-gateway.exe"
[ -x "${BIN}" ] || BIN="${ROOT_DIR}/target/debug/sunborn-gateway"
[ -x "${BIN}" ] || fail "gateway binary not found after build"

echo "=== 2/7: start Redis 7 / PostgreSQL 16 / Qdrant ==="
docker compose -p "${PROJECT}" -f "${COMPOSE_FILE}" down -v --remove-orphans >/dev/null 2>&1 || true
docker compose -p "${PROJECT}" -f "${COMPOSE_FILE}" up -d >/dev/null
for _ in $(seq 1 60); do
    if docker compose -p "${PROJECT}" -f "${COMPOSE_FILE}" exec -T postgres pg_isready -U sunborn -d sunborn >/dev/null 2>&1 \
        && docker compose -p "${PROJECT}" -f "${COMPOSE_FILE}" exec -T redis redis-cli ping 2>/dev/null | grep -q PONG \
        && curl -fsS http://127.0.0.1:56333/readyz >/dev/null 2>&1; then
        break
    fi
    sleep 1
done
curl -fsS http://127.0.0.1:56333/readyz >/dev/null || fail "Qdrant did not become ready"
docker compose -p "${PROJECT}" -f "${COMPOSE_FILE}" exec -T postgres pg_isready -U sunborn -d sunborn >/dev/null || fail "PostgreSQL did not become ready"
docker compose -p "${PROJECT}" -f "${COMPOSE_FILE}" exec -T redis redis-cli ping | grep -q PONG || fail "Redis did not become ready"

# The test tenant/application must exist before finops/cache writes (foreign keys).
docker compose -p "${PROJECT}" -f "${COMPOSE_FILE}" exec -T postgres \
    psql -U sunborn -d sunborn -v ON_ERROR_STOP=1 >/dev/null <<SQL
INSERT INTO tenants (id, name, status, rate_limit_rps)
VALUES ('${TENANT}', 'Distributed Test Tenant', 'active', 1000)
ON CONFLICT (id) DO NOTHING;
INSERT INTO applications (id, tenant_id, name, api_key_hash, status)
VALUES ('${APP}', '${TENANT}', 'Distributed Test App', 'test-fixture-hash', 'active')
ON CONFLICT (tenant_id, id) DO NOTHING;
SQL

echo "=== 3/7: start two independent gateway replicas ==="
COMMON_ENV=(
    "SUNBORN_ENV=development"
    "SUNBORN_DB_BACKEND=postgres"
    "POSTGRES_URL=postgres://sunborn:sunborn_dist_test_password@127.0.0.1:55432/sunborn"
    "SUNBORN_CACHE_BACKEND=memory"
    "SUNBORN_DISABLE_SEMANTIC_CACHE=true"
    "SUNBORN_VECTOR_BACKEND=qdrant"
    "QDRANT_URL=http://127.0.0.1:56333"
    "SUNBORN_REDIS_URL=redis://127.0.0.1:56379"
    "SUNBORN_API_KEYS=[{\"key\":\"${ADMIN_KEY}\",\"tenant\":\"${TENANT}\",\"app\":\"${APP}\",\"is_admin\":true}]"
    "SUNBORN_MASTER_KEK=${MASTER_KEK}"
    "SUNBORN_LEARNING_HMAC_SECRET=${HMAC_SECRET}"
    "SUNBORN_ATTESTATION_SEED_HEX=${ATTESTATION_SEED}"
    "RUST_LOG=info"
)

# Start A first and provision the tenant once. B must hydrate the same wrapped
# TDEK from PostgreSQL; independently provisioning both replicas would create
# different keys and make shared semantic candidates undecryptable.
env "${COMMON_ENV[@]}" PORT="${A_PORT}" POD_NAME="replica-a" \
    "${BIN}" >"${A_LOG}" 2>&1 &
A_PID=$!
wait_http "http://127.0.0.1:${A_PORT}/health" "replica A"
wait_http "http://127.0.0.1:${A_PORT}/ready" "replica A readiness"
PROVISION_ONE="$(curl -fsS -X POST \
    -H "x-api-key: ${ADMIN_KEY}" \
    -H "content-type: application/json" \
    --data "{\"tenant_id\":\"${TENANT}\"}" \
    "http://127.0.0.1:${A_PORT}/v1/governance/provision")"
PROVISION_TWO="$(curl -fsS -X POST \
    -H "x-api-key: ${ADMIN_KEY}" \
    -H "content-type: application/json" \
    --data "{\"tenant_id\":\"${TENANT}\"}" \
    "http://127.0.0.1:${A_PORT}/v1/governance/provision")"
echo "${PROVISION_TWO}" | python -c '
import json,sys
r=json.load(sys.stdin)
assert r["status"] == "already_provisioned", r
' || fail "second provisioning rotated or rejected the existing tenant key"
echo "tenant provisioning is idempotent (no TDEK rotation)"

env "${COMMON_ENV[@]}" PORT="${B_PORT}" POD_NAME="replica-b" \
    "${BIN}" >"${B_LOG}" 2>&1 &
B_PID=$!
wait_http "http://127.0.0.1:${B_PORT}/health" "replica B"
wait_http "http://127.0.0.1:${B_PORT}/ready" "replica B readiness"
# Allow both consumer-group listeners to finish XGROUP/XREADGROUP setup.
sleep 2

request_json() {
    cat <<JSON
{"tenant_id":"${TENANT}","application_id":"${APP}","model":"${MODEL}","prompt":"${PROMPT}"}
JSON
}

post_request() {
    local port="$1" out="$2"
    local code
    code="$(curl -sS -o "${out}" -w '%{http_code}' -X POST \
        -H "x-api-key: ${ADMIN_KEY}" \
        -H "content-type: application/json" \
        --data "$(request_json)" \
        "http://127.0.0.1:${port}/v1/requests")"
    if [ "${code}" != "200" ]; then
        echo "request failed with HTTP ${code}:" >&2
        cat "${out}" >&2
        echo >&2
        return 1
    fi
}

echo "=== 4/7: seed the same exact-cache key on both replicas ==="
post_request "${A_PORT}" "${ROOT_DIR}/target/dist-a.json"
post_request "${B_PORT}" "${ROOT_DIR}/target/dist-b.json"
KEY_A="$(python -c 'import json,sys; print(json.load(open(sys.argv[1]))["cache_key"])' "${ROOT_DIR}/target/dist-a.json")"
KEY_B="$(python -c 'import json,sys; print(json.load(open(sys.argv[1]))["cache_key"])' "${ROOT_DIR}/target/dist-b.json")"
[ -n "${KEY_A}" ] && [ "${KEY_A}" = "${KEY_B}" ] || fail "replicas produced different cache keys: ${KEY_A} vs ${KEY_B}"
echo "cache key: ${KEY_A}"

curl -fsS -H "x-api-key: ${ADMIN_KEY}" "http://127.0.0.1:${A_PORT}/v1/caches/${KEY_A}" >/dev/null \
    || fail "replica A cache entry missing before invalidation"
curl -fsS -H "x-api-key: ${ADMIN_KEY}" "http://127.0.0.1:${B_PORT}/v1/caches/${KEY_B}" >/dev/null \
    || fail "replica B cache entry missing before invalidation"
echo "both replicas have a local cache entry"

echo "=== 5/7: record dependency on replica A only ==="
curl -fsS -X POST \
    -H "x-api-key: ${ADMIN_KEY}" \
    -H "content-type: application/json" \
    --data "{\"source_id\":\"${SOURCE_ID}\",\"cache_key\":\"${KEY_A}\"}" \
    "http://127.0.0.1:${A_PORT}/v1/dependencies" >/dev/null \
    || fail "replica A dependency registration failed"

echo "=== 6/7: mutate on replica A ==="
INVALIDATION="$(curl -fsS -X POST \
    -H "x-api-key: ${ADMIN_KEY}" \
    -H "content-type: application/json" \
    --data "{\"source_id\":\"${SOURCE_ID}\",\"tenant_id\":\"${TENANT}\",\"version_tag\":\"v2\"}" \
    "http://127.0.0.1:${A_PORT}/v1/invalidation")"
echo "${INVALIDATION}" | python -m json.tool
echo "${INVALIDATION}" | python -c '
import json,sys
r=json.load(sys.stdin)
assert r["cluster_broadcast_sent"] is True, "A did not report a cluster broadcast"
assert r["failed_cache_keys"] == [], r["failed_cache_keys"]
' || fail "invalidation did not produce a clean broadcast"

echo "=== 7/7: assert replica B was purged by the durable stream ==="
for _ in $(seq 1 40); do
    CODE="$(curl -sS -o /dev/null -w '%{http_code}' -H "x-api-key: ${ADMIN_KEY}" \
        "http://127.0.0.1:${B_PORT}/v1/caches/${KEY_B}")"
    if [ "${CODE}" = "404" ]; then
        echo "PASS: replica B local cache was purged by replica A's Redis Stream event"
        exit 0
    fi
    sleep 0.25
done

fail "replica B still serves stale cache entry after invalidation (last HTTP ${CODE})"
