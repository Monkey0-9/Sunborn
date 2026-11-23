#!/usr/bin/env bash
# =================================================================
# Sunborn end-to-end smoke test against a running gateway.
#
# Verifies the broker-wired request path end to end:
#   1. /health and /ready answer
#   2. a fresh prompt is an origin miss (cached=false)
#   3. the identical prompt returns an exact cache hit
#   4. /metrics reports at least one exact-tier hit
#
# Requires a running gateway (see `make dev`). Uses only curl + grep.
#
# Override defaults with SUNBORN_BASE_URL, SUNBORN_API_KEY,
# SUNBORN_TENANT_ID, SUNBORN_APPLICATION_ID, SUNBORN_MODEL.
# =================================================================
set -euo pipefail

BASE="${SUNBORN_BASE_URL:-http://127.0.0.1:8080}"
KEY="${SUNBORN_API_KEY:-test-key-alpha}"
TENANT="${SUNBORN_TENANT_ID:-tenant-alpha}"
APP="${SUNBORN_APPLICATION_ID:-app-default}"
MODEL="${SUNBORN_MODEL:-gpt-4o}"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

request() {
  local payload="$1"
  curl -fsS -m 30 \
    -H "content-type: application/json" \
    -H "x-api-key: ${KEY}" \
    -d "$payload" \
    "${BASE}/v1/requests"
}

echo "[1/4] health and readiness"
curl -fsS -m 5 "${BASE}/health" >/dev/null || fail "gateway is not healthy at ${BASE}/health"
# /ready returns 503 when a configured dependency (e.g. an LLM provider key)
# is unavailable. That is an honest degraded state, not a smoke-test failure:
# print the body and status rather than hiding them behind `curl -f`.
ready_code="$(curl -sS -m 10 -o /dev/null -w '%{http_code}' "${BASE}/ready" || true)"
ready_body="$(curl -sS -m 10 "${BASE}/ready" || true)"
echo "      /ready: HTTP ${ready_code} ${ready_body}"

echo "[2/4] fresh prompt must be an origin miss"
nonce="$(date +%s)-${RANDOM}"
payload="$(printf '{"tenant_id":"%s","application_id":"%s","model":"%s","prompt":"Reply with the single word OK. Smoke nonce %s."}' \
  "$TENANT" "$APP" "$MODEL" "$nonce")"
first="$(request "$payload")"
case "$first" in
  *'"cached":false'*) echo "      miss confirmed (cached=false)" ;;
  *) fail "first request was unexpectedly cached: $first" ;;
esac

echo "[3/4] identical prompt must be an exact cache hit"
second="$(request "$payload")"
case "$second" in
  *'"cache_path":"exact"'*) echo "      exact hit confirmed" ;;
  *) fail "second request was not an exact cache hit: $second" ;;
esac
case "$second" in
  *'"cached":true'*) ;;
  *) fail "second request did not report cached=true: $second" ;;
esac

echo "[4/4] metrics must report an exact-tier hit"
metrics="$(curl -fsS -m 5 "${BASE}/metrics")"
hits="$(printf '%s\n' "$metrics" | grep 'sunborn_cache_hits_total{scope="process_local",tier="exact"}' | awk '{print $2}' | head -1)"
if [ -z "${hits:-}" ] || [ "$hits" = "0" ]; then
  fail "expected a non-zero exact-tier hit count, got '${hits:-<missing>}'"
fi
echo "      exact-tier hits: ${hits}"

echo "PASS: broker-wired gateway served an origin miss followed by an exact cache hit."
