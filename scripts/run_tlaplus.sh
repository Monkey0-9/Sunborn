#!/usr/bin/env bash
# Model-check the TLA+ specifications in formal/ with TLC.
#
# Requires Java 17+. The TLA+ toolkit jar is fetched once and cached in
# formal/tla2tools.jar (ignored by git).
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FORMAL_DIR="${ROOT_DIR}/formal"
JAR="${FORMAL_DIR}/tla2tools.jar"

cd "${FORMAL_DIR}"

if [ ! -f "${JAR}" ]; then
    echo "=== downloading TLA+ toolkit ==="
    curl -fsSL --max-time 180 -o "${JAR}.tmp" \
        https://github.com/tlaplus/tlaplus/releases/latest/download/tla2tools.jar
    mv "${JAR}.tmp" "${JAR}"
fi

if ! java -version >/dev/null 2>&1; then
    echo "FAIL: Java 17+ is required to run TLC" >&2
    exit 1
fi

run() {
    local module="$1" config="$2" label="$3"
    echo "=== model-checking ${label} (${config}) ==="
    java -XX:+UseParallelGC -cp "${JAR}" tlc2.TLC \
        -metadir "${ROOT_DIR}/target/tlc-states" \
        -config "${config}" -deadlock "${module}" 2>&1 | tail -12
    echo
}

run EpochFencing EpochFencing.cfg "epoch monotonicity / persist-before-publish"
run OutboxAtLeastOnce OutboxAtLeastOnce.cfg "outbox safety invariants"
run OutboxAtLeastOnce OutboxLiveness.cfg "outbox bounded liveness"

echo "PASS: all TLA+ models checked with no invariant or property violation"
