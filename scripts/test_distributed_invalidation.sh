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

# exact: disable semantic hits and prove peer-local exact-cache purge.
# semantic: enable semantic hits and prove a shared Qdrant candidate written
#           by A is served by B, then purged on A's mutation.
# outbox: exercise the live PostgreSQL outbox worker: normal completion,
#         expired-lease reclaim after a simulated crash, retry, and DLQ.
# replay: sever replica B's Redis connection, mutate on A, reconnect B, and
#         prove the durable stream backlog purges B's stale local cache.
# finops: with a durable cluster-wide spend ceiling, two replicas racing for the
#         same tenant budget must grant exactly one reservation.
# crash:  abort the gateway between purge and outbox ACK, restart it, and prove
#         the unacknowledged event is reclaimed rather than lost.
MODE="${DIST_MODE:-exact}"
case "${MODE}" in
    exact|semantic|outbox|replay|finops|crash) ;;
    *) echo "DIST_MODE must be 'exact', 'semantic', 'outbox', 'replay', 'finops', or 'crash'" >&2; exit 2 ;;
esac

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
PROXY_PID=""
REPLAY_PROXY_PORT=56380

cleanup() {
    set +e
    [ -n "${PROXY_PID}" ] && kill "${PROXY_PID}" 2>/dev/null
    [ -n "${A_PID}" ] && kill "${A_PID}" 2>/dev/null
    [ -n "${B_PID}" ] && kill "${B_PID}" 2>/dev/null
    wait "${PROXY_PID}" 2>/dev/null
    wait "${A_PID}" 2>/dev/null
    wait "${B_PID}" 2>/dev/null
    if [ "${KEEP_STACK:-0}" != "1" ]; then
        docker compose -p "${PROJECT}" -f "${COMPOSE_FILE}" down -v --remove-orphans >/dev/null 2>&1
    fi
}
trap cleanup EXIT
trap 'fail "Command failed at line $LINENO"' ERR

fail() {
    echo "FAIL: $*" >&2
    echo "--- redis proxy log (tail) ---" >&2
    tail -60 "${ROOT_DIR}/target/redis-proxy.log" >&2 2>/dev/null || true
    echo "--- replica A log (tail) ---" >&2
    tail -60 "${A_LOG}" >&2 2>/dev/null || true
    echo "--- replica B log (tail) ---" >&2
    tail -60 "${B_LOG}" >&2 2>/dev/null || true
    echo "--- postgres container log (tail) ---" >&2
    docker compose -p "${PROJECT}" -f "${COMPOSE_FILE}" logs --tail 40 postgres >&2 2>/dev/null || true
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

# Clean up any leftover logs or artifacts from previous runs or CI caches
rm -f "${A_LOG}" "${B_LOG}" "${A_LOG}.crash" "${ROOT_DIR}/target/finops-"*.json "${ROOT_DIR}/target/dist-"*.json "${ROOT_DIR}/target/redis-proxy.log"

echo "Waiting for PostgreSQL, Redis, and Qdrant to be fully operational..."
READY=0
for i in $(seq 1 90); do
    if docker compose -p "${PROJECT}" -f "${COMPOSE_FILE}" exec -T redis redis-cli ping 2>/dev/null | grep -q PONG \
        && curl -fsS http://127.0.0.1:56333/readyz >/dev/null 2>&1 \
        && docker compose -p "${PROJECT}" -f "${COMPOSE_FILE}" logs postgres 2>/dev/null | grep -q "PostgreSQL init process complete; ready for start up." \
        && docker compose -p "${PROJECT}" -f "${COMPOSE_FILE}" exec -T postgres psql -U sunborn -d sunborn -c "SELECT 1 FROM pg_constraint WHERE conname = 'spend_reservations_amount_micro_units_non_negative';" >/dev/null 2>&1; then
        READY=1
        break
    fi
    sleep 1
done

[ "${READY}" = "1" ] || fail "PostgreSQL, Redis, or Qdrant did not become ready within 90s"

# The test tenant/application must exist before finops/cache writes (foreign keys).
# Robust retry loop in case postgres has a microsecond socket transition after restart.
SEEDED=0
for attempt in $(seq 1 30); do
    if docker compose -p "${PROJECT}" -f "${COMPOSE_FILE}" exec -T postgres \
        psql -U sunborn -d sunborn -v ON_ERROR_STOP=1 >/dev/null 2>&1 <<SQL
INSERT INTO tenants (id, name, status, rate_limit_rps)
VALUES ('${TENANT}', 'Distributed Test Tenant', 'active', 1000)
ON CONFLICT (id) DO NOTHING;
INSERT INTO applications (id, tenant_id, name, api_key_hash, status)
VALUES ('${APP}', '${TENANT}', 'Distributed Test App', 'test-fixture-hash', 'active')
ON CONFLICT (tenant_id, id) DO NOTHING;
SQL
    then
        SEEDED=1
        break
    fi
    sleep 1
done

[ "${SEEDED}" = "1" ] || fail "Failed to seed test tenant and application into PostgreSQL"

echo "=== 3/7: start two independent gateway replicas ==="
COMMON_ENV=(
    "SUNBORN_ENV=development"
    "SUNBORN_DB_BACKEND=postgres"
    "POSTGRES_URL=postgres://sunborn:sunborn_dist_test_password@127.0.0.1:55432/sunborn"
    "SUNBORN_CACHE_BACKEND=memory"
    "SUNBORN_VECTOR_BACKEND=qdrant"
    "QDRANT_URL=http://127.0.0.1:56333"
    "SUNBORN_REDIS_URL=redis://127.0.0.1:56379"
    "SUNBORN_API_KEYS=[{\"key\":\"${ADMIN_KEY}\",\"tenant\":\"${TENANT}\",\"app\":\"${APP}\",\"is_admin\":true}]"
    "SUNBORN_MASTER_KEK=${MASTER_KEK}"
    "SUNBORN_LEARNING_HMAC_SECRET=${HMAC_SECRET}"
    "SUNBORN_ATTESTATION_SEED_HEX=${ATTESTATION_SEED}"
    "RUST_LOG=info"
)
if [ "${MODE}" = "exact" ] || [ "${MODE}" = "replay" ]; then
    COMMON_ENV+=("SUNBORN_DISABLE_SEMANTIC_CACHE=true")
fi
if [ "${MODE}" = "crash" ]; then
    # Armed only for the dedicated crash harness; the gateway refuses to honor
    # this hook in production mode.
    COMMON_ENV+=("SUNBORN_FAULT_INJECT_OUTBOX_CRASH=after_purge")
fi

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

psql_scalar() {
    docker compose -p "${PROJECT}" -f "${COMPOSE_FILE}" exec -T postgres \
        psql -U sunborn -d sunborn -tAc "$1"
}

if [ "${MODE}" = "outbox" ]; then
    echo "=== 4/7: seed live PostgreSQL outbox rows ==="
    GOOD_SOURCE="bbbbbbbb-1111-4222-8333-444444444444"
    STALE_SOURCE="cccccccc-1111-4222-8333-444444444444"
    BAD_ENTITY="dddddddd-1111-4222-8333-444444444444"
    docker compose -p "${PROJECT}" -f "${COMPOSE_FILE}" exec -T postgres \
        psql -U sunborn -d sunborn -v ON_ERROR_STOP=1 >/dev/null <<SQL
INSERT INTO outbox_events
    (tenant_id, entity_type, entity_id, action, payload, status, retry_count, max_retries, claimed_by, claimed_at)
VALUES
    ('${TENANT}', 'knowledge_source', '${GOOD_SOURCE}', 'invalidate', '{}'::jsonb, 'PENDING', 0, 5, NULL, NULL),
    ('${TENANT}', 'knowledge_source', '${STALE_SOURCE}', 'invalidate', '{}'::jsonb, 'PROCESSING', 0, 5, 'dead-worker', NOW() - INTERVAL '10 minutes'),
    ('${TENANT}', 'unsupported_entity', '${BAD_ENTITY}', 'noop', '{}'::jsonb, 'PENDING', 0, 2, NULL, NULL);
SQL
    echo "seeded one pending event, one expired claim, and one poison event"

    echo "=== 5/7: wait for the live worker to claim, retry, and dead-letter ==="
    GOOD_STATUS=""; STALE_STATUS=""; BAD_STATUS=""
    for _ in $(seq 1 40); do
        GOOD_STATUS="$(psql_scalar "SELECT status FROM outbox_events WHERE entity_id='${GOOD_SOURCE}';")"
        STALE_STATUS="$(psql_scalar "SELECT status FROM outbox_events WHERE entity_id='${STALE_SOURCE}';")"
        BAD_STATUS="$(psql_scalar "SELECT status FROM outbox_events WHERE entity_id='${BAD_ENTITY}';")"
        if [ "${GOOD_STATUS}" = "PROCESSED" ] \
            && [ "${STALE_STATUS}" = "PROCESSED" ] \
            && [ "${BAD_STATUS}" = "FAILED" ]; then
            break
        fi
        sleep 0.5
    done

    echo "=== 6/7: assert normal completion and expired-lease reclaim ==="
    [ "${GOOD_STATUS}" = "PROCESSED" ] || fail "normal outbox event was not processed (status=${GOOD_STATUS})"
    [ "${STALE_STATUS}" = "PROCESSED" ] || fail "expired outbox lease was not reclaimed and processed (status=${STALE_STATUS})"
    psql_scalar "SELECT processed_at IS NOT NULL FROM outbox_events WHERE entity_id='${GOOD_SOURCE}';" \
        | grep -qx t || fail "processed event has no processed_at timestamp"
    psql_scalar "SELECT claimed_by FROM outbox_events WHERE entity_id='${STALE_SOURCE}';" \
        | grep -q 'replica-a' || fail "stale event was not claimed by the live replica worker"
    echo "PASS: pending and expired-claim events were processed by the live worker"

    echo "=== 7/7: assert retry exhaustion reached the durable DLQ ==="
    [ "${BAD_STATUS}" = "FAILED" ] || fail "poison outbox event did not reach FAILED (status=${BAD_STATUS})"
    RETRY_COUNT="$(psql_scalar "SELECT retry_count FROM outbox_events WHERE entity_id='${BAD_ENTITY}';")"
    [ "${RETRY_COUNT}" = "2" ] || fail "poison event retry_count=${RETRY_COUNT}, expected 2"
    DLQ_COUNT="$(psql_scalar "SELECT count(*) FROM outbox_dlq WHERE entity_id='${BAD_ENTITY}';")"
    [ "${DLQ_COUNT}" = "1" ] || fail "poison event produced ${DLQ_COUNT} DLQ rows, expected 1"
    psql_scalar "SELECT last_error FROM outbox_events WHERE entity_id='${BAD_ENTITY}';" \
        | grep -q 'Unsupported outbox event' || fail "DLQ row does not retain the failure reason"
    echo "PASS: retry exhaustion persisted the poison event to outbox_dlq"
    exit 0
fi

if [ "${MODE}" = "crash" ]; then
    echo "=== 4/6: seed an outbox event and let replica A abort before the ACK ==="
    CRASH_SOURCE="eeeeeeee-1111-4222-8333-444444444444"
    docker compose -p "${PROJECT}" -f "${COMPOSE_FILE}" exec -T postgres \
        psql -U sunborn -d sunborn -v ON_ERROR_STOP=1 >/dev/null <<SQL
INSERT INTO outbox_events
    (tenant_id, entity_type, entity_id, action, payload, status, retry_count, max_retries)
VALUES
    ('${TENANT}', 'knowledge_source', '${CRASH_SOURCE}', 'invalidate', '{}'::jsonb, 'PENDING', 0, 5);
SQL

    A_EXITED=0
    for _ in $(seq 1 60); do
        if ! kill -0 "${A_PID}" 2>/dev/null; then
            A_EXITED=1
            break
        fi
        sleep 0.25
    done
    [ "${A_EXITED}" = "1" ] || fail "replica A did not abort at the purge/ACK boundary"
    wait "${A_PID}" 2>/dev/null || true
    A_PID=""
    grep -q 'FAULT INJECTION: aborting between purge and ACK' "${A_LOG}" \
        || fail "replica A exited without reaching the injected crash point"
    mv "${A_LOG}" "${A_LOG}.crash"

    echo "=== 5/6: assert the event survived the crash unacknowledged ==="
    CRASH_STATUS="$(psql_scalar "SELECT status FROM outbox_events WHERE entity_id='${CRASH_SOURCE}';")"
    [ "${CRASH_STATUS}" = "PROCESSING" ] \
        || fail "crashed event status is '${CRASH_STATUS}', expected an unacknowledged PROCESSING lease"
    psql_scalar "SELECT processed_at IS NULL FROM outbox_events WHERE entity_id='${CRASH_SOURCE}';" \
        | grep -qx t || fail "crashed event was acknowledged despite the abort"
    echo "event survived the crash with an unacknowledged lease"

    echo "=== 6/6: restart, reclaim the expired lease, and assert no loss ==="
    psql_scalar "UPDATE outbox_events SET claimed_at = NOW() - INTERVAL '10 minutes' WHERE entity_id='${CRASH_SOURCE}';" >/dev/null
    A_ENV=()
    for item in "${COMMON_ENV[@]}"; do
        case "${item}" in
            SUNBORN_FAULT_INJECT_OUTBOX_CRASH=*) ;;
            *) A_ENV+=("${item}") ;;
        esac
    done
    env "${A_ENV[@]}" PORT="${A_PORT}" POD_NAME="replica-a" \
        "${BIN}" >"${A_LOG}" 2>&1 &
    A_PID=$!
    wait_http "http://127.0.0.1:${A_PORT}/health" "restarted replica A"

    RECLAIMED=""
    for _ in $(seq 1 60); do
        RECLAIMED="$(psql_scalar "SELECT status FROM outbox_events WHERE entity_id='${CRASH_SOURCE}';")"
        if [ "${RECLAIMED}" = "PROCESSED" ]; then
            break
        fi
        sleep 0.5
    done
    [ "${RECLAIMED}" = "PROCESSED" ] \
        || fail "unacknowledged event was not reclaimed after restart (status=${RECLAIMED})"
    ROW_COUNT="$(psql_scalar "SELECT count(*) FROM outbox_events WHERE entity_id='${CRASH_SOURCE}';")"
    [ "${ROW_COUNT}" = "1" ] || fail "crash produced ${ROW_COUNT} outbox rows, expected exactly 1"
    psql_scalar "SELECT processed_at IS NOT NULL FROM outbox_events WHERE entity_id='${CRASH_SOURCE}';" \
        | grep -qx t || fail "reclaimed event has no processed_at timestamp"
    echo "PASS: the unacknowledged event was reclaimed exactly once (no loss, no duplicate row)"
    exit 0
fi

B_ENV=("${COMMON_ENV[@]}")
if [ "${MODE}" = "replay" ]; then
    PYTHON_CMD="python3"
    command -v python3 >/dev/null 2>&1 || PYTHON_CMD="python"
    ${PYTHON_CMD} -u scripts/redis_tcp_proxy.py "${REPLAY_PROXY_PORT}" 56379 \
        >"${ROOT_DIR}/target/redis-proxy.log" 2>&1 &
    PROXY_PID=$!
    for _ in $(seq 1 40); do
        if ${PYTHON_CMD} -c "import socket; s=socket.create_connection(('127.0.0.1',${REPLAY_PROXY_PORT}),0.2); s.close()" 2>/dev/null; then
            break
        fi
        if ! kill -0 "${PROXY_PID}" 2>/dev/null; then
            echo "Proxy process ${PROXY_PID} died prematurely:" >&2
            cat "${ROOT_DIR}/target/redis-proxy.log" >&2 || true
            fail "Redis replay proxy exited prematurely"
        fi
        sleep 0.25
    done
    ${PYTHON_CMD} -c "import socket; s=socket.create_connection(('127.0.0.1',${REPLAY_PROXY_PORT}),0.2); s.close()" \
        || fail "Redis replay proxy did not start on port ${REPLAY_PROXY_PORT}"
    B_ENV=()
    for item in "${COMMON_ENV[@]}"; do
        case "${item}" in
            SUNBORN_REDIS_URL=*) ;;
            *) B_ENV+=("${item}") ;;
        esac
    done
    B_ENV+=("SUNBORN_REDIS_URL=redis://127.0.0.1:${REPLAY_PROXY_PORT}")
fi

env "${B_ENV[@]}" PORT="${B_PORT}" POD_NAME="replica-b" \
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

cache_status() {
    local port="$1" key="$2"
    curl -sS -o /dev/null -w '%{http_code}' -H "x-api-key: ${ADMIN_KEY}" \
        "http://127.0.0.1:${port}/v1/caches/${key}"
}

qdrant_count() {
    curl -fsS -X POST \
        -H 'content-type: application/json' \
        -d "{\"filter\":{\"must\":[{\"key\":\"tenant_id\",\"match\":{\"value\":\"${TENANT}\"}}]},\"exact\":true}" \
        "http://127.0.0.1:56333/collections/sunborn_vectors/points/count" \
        | python -c 'import json,sys; print(json.load(sys.stdin)["result"]["count"])'
}

if [ "${MODE}" = "finops" ]; then
    echo "=== 4/6: seed a durable cluster-wide spend ceiling ==="
    psql_scalar "INSERT INTO spend_tenant_ceilings (tenant_id, ceiling_usd, ceiling_micro_units, updated_at)
                 VALUES ('${TENANT}', 0.0001, 100, NOW())
                 ON CONFLICT (tenant_id) DO UPDATE
                 SET ceiling_usd = EXCLUDED.ceiling_usd, ceiling_micro_units = EXCLUDED.ceiling_micro_units, updated_at = NOW();" \
        >/dev/null || fail "failed to seed the durable spend ceiling"
    for port in "${A_PORT}" "${B_PORT}"; do
        curl -fsS -H "x-api-key: ${ADMIN_KEY}" \
            "http://127.0.0.1:${port}/v1/cluster/status" \
            | python -c '
import json,sys
r=json.load(sys.stdin)
assert r["spend_velocity_scope"] == "cluster_distributed", r
' || fail "replica on port ${port} is not enforcing spend through the durable ledger"
    done
    echo "both replicas report cluster_distributed spend enforcement"

    echo "=== 5/6: race both replicas for the same tenant budget ==="
    rm -f "${ROOT_DIR}/target/finops-a.code" "${ROOT_DIR}/target/finops-b.code"
    curl -sS -o "${ROOT_DIR}/target/finops-a.json" -w '%{http_code}' -X POST \
        -H "x-api-key: ${ADMIN_KEY}" -H "content-type: application/json" \
        --data "$(request_json)" \
        "http://127.0.0.1:${A_PORT}/v1/requests" >"${ROOT_DIR}/target/finops-a.code" &
    FINOPS_A_PID=$!
    curl -sS -o "${ROOT_DIR}/target/finops-b.json" -w '%{http_code}' -X POST \
        -H "x-api-key: ${ADMIN_KEY}" -H "content-type: application/json" \
        --data "$(request_json)" \
        "http://127.0.0.1:${B_PORT}/v1/requests" >"${ROOT_DIR}/target/finops-b.code" &
    FINOPS_B_PID=$!
    wait "${FINOPS_A_PID}" 2>/dev/null || true
    wait "${FINOPS_B_PID}" 2>/dev/null || true
    A_CODE="$(cat "${ROOT_DIR}/target/finops-a.code")"
    B_CODE="$(cat "${ROOT_DIR}/target/finops-b.code")"
    echo "replica A: HTTP ${A_CODE}, replica B: HTTP ${B_CODE}"

    CODES="$(printf '%s\n%s\n' "${A_CODE}" "${B_CODE}" | sort | tr '\n' ' ')"
    [ "${CODES}" = "200 429 " ] \
        || fail "expected exactly one granted and one throttled request, got: ${CODES}"

    DENIED_BODY="${ROOT_DIR}/target/finops-b.json"
    if [ "${B_CODE}" = "200" ]; then
        DENIED_BODY="${ROOT_DIR}/target/finops-a.json"
    fi
    grep -q 'Spend velocity ceiling' "${DENIED_BODY}" \
        || fail "the rejected replica did not report a spend velocity rejection"
    echo "PASS: the shared budget admitted exactly one replica"

    echo "=== 6/6: assert the ledger reflects one settled origin dispatch ==="
    SETTLED=""
    for _ in $(seq 1 20); do
        SETTLED="$(psql_scalar "SELECT count(*) FROM spend_ledger_entries WHERE tenant_id='${TENANT}';")"
        ACTIVE="$(psql_scalar "SELECT count(*) FROM spend_reservations WHERE tenant_id='${TENANT}';")"
        if [ "${SETTLED}" = "1" ] && [ "${ACTIVE}" = "0" ]; then
            break
        fi
        sleep 0.25
    done
    [ "${SETTLED}" = "1" ] || fail "expected one settled spend entry, found ${SETTLED}"
    [ "${ACTIVE}" = "0" ] || fail "expected zero dangling reservations, found ${ACTIVE}"
    RETRY_CODE="$(curl -sS -o /dev/null -w '%{http_code}' -X POST \
        -H "x-api-key: ${ADMIN_KEY}" -H "content-type: application/json" \
        --data "$(request_json)" \
        "http://127.0.0.1:${A_PORT}/v1/requests")"
    [ "${RETRY_CODE}" = "429" ] || fail "post-settlement request returned ${RETRY_CODE}, expected 429"
    echo "PASS: durable ledger shows exactly one settled dispatch and blocks further spend"
    exit 0
fi

if [ "${MODE}" = "semantic" ]; then
    echo "=== 4/7: seed a semantic candidate on replica A ==="
    post_request "${A_PORT}" "${ROOT_DIR}/target/dist-a.json"
    KEY_A="$(python -c 'import json,sys; print(json.load(open(sys.argv[1]))["cache_key"])' "${ROOT_DIR}/target/dist-a.json")"
    A_CACHED="$(python -c 'import json,sys; print(json.load(open(sys.argv[1]))["cached"])' "${ROOT_DIR}/target/dist-a.json")"
    [ -n "${KEY_A}" ] || fail "replica A did not return a cache key"
    [ "${A_CACHED}" = "False" ] || fail "first request unexpectedly hit cache"
    COUNT_BEFORE="$(qdrant_count)"
    [ "${COUNT_BEFORE}" -ge 1 ] || fail "replica A did not index a Qdrant candidate"
    echo "Qdrant candidates for tenant after A's write: ${COUNT_BEFORE}"

    echo "=== 5/7: replica B must serve the shared candidate ==="
    post_request "${B_PORT}" "${ROOT_DIR}/target/dist-b.json"
    B_CACHED="$(python -c 'import json,sys; print(json.load(open(sys.argv[1]))["cached"])' "${ROOT_DIR}/target/dist-b.json")"
    [ "${B_CACHED}" = "True" ] || fail "replica B did not serve the shared Qdrant candidate"
    python -c '
import json,sys
a=json.load(open(sys.argv[1]))
b=json.load(open(sys.argv[2]))
assert a["content"] == b["content"], (a["content"], b["content"])
' "${ROOT_DIR}/target/dist-a.json" "${ROOT_DIR}/target/dist-b.json" \
        || fail "replica B returned different content from the shared candidate"
    echo "PASS: replica B served replica A's shared-Qdrant semantic candidate"

    echo "=== 6/7: record dependency on A only, then mutate on A ==="
    curl -fsS -X POST \
        -H "x-api-key: ${ADMIN_KEY}" \
        -H "content-type: application/json" \
        --data "{\"source_id\":\"${SOURCE_ID}\",\"cache_key\":\"${KEY_A}\",\"tenant_id\":\"${TENANT}\"}" \
        "http://127.0.0.1:${A_PORT}/v1/dependencies" >/dev/null \
        || fail "replica A dependency registration failed"
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
' || fail "semantic invalidation did not produce a clean broadcast"

    echo "=== 7/7: assert the shared Qdrant candidate is purged ==="
    COUNT_AFTER="${COUNT_BEFORE}"
    for _ in $(seq 1 40); do
        COUNT_AFTER="$(qdrant_count)"
        if [ "${COUNT_AFTER}" = "0" ]; then
            break
        fi
        sleep 0.25
    done
    [ "${COUNT_AFTER}" = "0" ] || fail "Qdrant still holds ${COUNT_AFTER} candidate(s) after invalidation"

    post_request "${B_PORT}" "${ROOT_DIR}/target/dist-b-after.json"
    AFTER_CACHED="$(python -c 'import json,sys; print(json.load(open(sys.argv[1]))["cached"])' "${ROOT_DIR}/target/dist-b-after.json")"
    [ "${AFTER_CACHED}" = "False" ] || fail "replica B served stale semantic content after invalidation"
    echo "PASS: shared Qdrant candidate was purged and replica B fell through to origin"
    exit 0
fi

if [ "${MODE}" = "replay" ]; then
    echo "=== 4/7: seed the same exact-cache key on both replicas ==="
    post_request "${A_PORT}" "${ROOT_DIR}/target/dist-a.json"
    post_request "${B_PORT}" "${ROOT_DIR}/target/dist-b.json"
    KEY_A="$(python -c 'import json,sys; print(json.load(open(sys.argv[1]))["cache_key"])' "${ROOT_DIR}/target/dist-a.json")"
    KEY_B="$(python -c 'import json,sys; print(json.load(open(sys.argv[1]))["cache_key"])' "${ROOT_DIR}/target/dist-b.json")"
    [ -n "${KEY_A}" ] && [ "${KEY_A}" = "${KEY_B}" ] || fail "replicas produced different cache keys: ${KEY_A} vs ${KEY_B}"
    [ "$(cache_status "${A_PORT}" "${KEY_A}")" = "200" ] || fail "replica A cache entry missing before disconnect"
    [ "$(cache_status "${B_PORT}" "${KEY_B}")" = "200" ] || fail "replica B cache entry missing before disconnect"
    echo "both replicas have a local cache entry"

    echo "=== 5/7: sever replica B's Redis connection without stopping B ==="
    kill "${PROXY_PID}" 2>/dev/null || true
    wait "${PROXY_PID}" 2>/dev/null || true
    PROXY_PID=""
    sleep 1
    [ "$(cache_status "${B_PORT}" "${KEY_B}")" = "200" ] \
        || fail "replica B lost its local cache when only Redis was disconnected"
    echo "replica B is serving its stale local entry while disconnected"

    echo "=== 6/7: mutate on replica A while replica B is disconnected ==="
    curl -fsS -X POST \
        -H "x-api-key: ${ADMIN_KEY}" \
        -H "content-type: application/json" \
        --data "{\"source_id\":\"${SOURCE_ID}\",\"cache_key\":\"${KEY_A}\",\"tenant_id\":\"${TENANT}\"}" \
        "http://127.0.0.1:${A_PORT}/v1/dependencies" >/dev/null \
        || fail "replica A dependency registration failed"
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
' || fail "replay invalidation did not produce a clean broadcast"
    STREAM_LEN="$(docker compose -p "${PROJECT}" -f "${COMPOSE_FILE}" exec -T redis \
        redis-cli XLEN sunborn:invalidation:events | tr -d '\r')"
    [ "${STREAM_LEN:-0}" -ge 1 ] || fail "mutation did not leave a durable stream entry for replay"
    [ "$(cache_status "${B_PORT}" "${KEY_B}")" = "200" ] \
        || fail "replica B was purged while its Redis connection was severed"
    echo "A's event is durable in Redis (XLEN=${STREAM_LEN}); B is still stale"

    echo "=== 7/7: reconnect B and assert the backlog replays ==="
    PYTHON_CMD="python3"
    command -v python3 >/dev/null 2>&1 || PYTHON_CMD="python"
    ${PYTHON_CMD} -u scripts/redis_tcp_proxy.py "${REPLAY_PROXY_PORT}" 56379 \
        >"${ROOT_DIR}/target/redis-proxy.log" 2>&1 &
    PROXY_PID=$!
    for _ in $(seq 1 40); do
        if ${PYTHON_CMD} -c "import socket; s=socket.create_connection(('127.0.0.1',${REPLAY_PROXY_PORT}),0.2); s.close()" 2>/dev/null; then
            break
        fi
        if ! kill -0 "${PROXY_PID}" 2>/dev/null; then
            echo "Proxy process ${PROXY_PID} died prematurely on reconnect:" >&2
            cat "${ROOT_DIR}/target/redis-proxy.log" >&2 || true
            fail "Redis replay proxy exited prematurely on reconnect"
        fi
        sleep 0.25
    done
    ${PYTHON_CMD} -c "import socket; s=socket.create_connection(('127.0.0.1',${REPLAY_PROXY_PORT}),0.2); s.close()" \
        || fail "Redis replay proxy did not restart"

    LAST_CODE=""
    for _ in $(seq 1 60); do
        LAST_CODE="$(cache_status "${B_PORT}" "${KEY_B}")"
        if [ "${LAST_CODE}" = "404" ]; then
            grep -q 'Redis stream read failed; reconnecting' "${B_LOG}" \
                || fail "B purged the key but did not log a Redis reconnect"
            echo "PASS: replica B reconnected and replayed A's durable stream event"
            exit 0
        fi
        sleep 0.5
    done

    fail "replica B did not replay the backlog after reconnect (last HTTP ${LAST_CODE})"
fi

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
    --data "{\"source_id\":\"${SOURCE_ID}\",\"cache_key\":\"${KEY_A}\",\"tenant_id\":\"${TENANT}\"}" \
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
