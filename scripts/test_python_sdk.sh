#!/usr/bin/env bash
# ==============================================================================
# Sunborn Python SDK: Build & Clean-Environment Installation Verification
# Validates PEP 517/621 packaging, wheel distribution, and clean-room installation.
# ==============================================================================
set -eu
(set -o pipefail 2>/dev/null) && set -o pipefail || true

# The smoke test below prints check marks; without this, a cp1252 console raises
# UnicodeEncodeError and the gate fails for a reason unrelated to the SDK.
export PYTHONUTF8=1
export PYTHONIOENCODING=utf-8

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SDK_DIR="${ROOT_DIR}/sdk/python"
TEMP_VENV=".venv_smoke"

UV_CMD=()
if command -v uv >/dev/null 2>&1; then
    UV_CMD=(uv)
elif command -v uv.exe >/dev/null 2>&1; then
    UV_CMD=(uv.exe)
elif python -m uv --version >/dev/null 2>&1; then
    UV_CMD=(python -m uv)
elif python3 -m uv --version >/dev/null 2>&1; then
    UV_CMD=(python3 -m uv)
else
    echo "  ℹ uv not detected on PATH; attempting to install uv via pip..."
    if command -v python >/dev/null 2>&1; then
        python -m pip install uv
        UV_CMD=(python -m uv)
    elif command -v python3 >/dev/null 2>&1; then
        python3 -m pip install uv
        UV_CMD=(python3 -m uv)
    else
        echo "Error: Neither uv nor python found on PATH" >&2
        exit 1
    fi
fi

cleanup() {
    rm -rf "${TEMP_VENV}"
}
trap cleanup EXIT

echo "=== [1/4] Building Sunborn Python SDK Package (sdist + wheel) ==="
cd "${SDK_DIR}"
rm -rf dist build *.egg-info "${TEMP_VENV}"
"${UV_CMD[@]}" build

WHEEL_FILE="$(ls -1 dist/*.whl | head -n 1)"
echo "  ✓ Successfully built wheel: ${WHEEL_FILE}"

echo "=== [2/4] Creating Clean Isolated Environment ==="
"${UV_CMD[@]}" venv "${TEMP_VENV}"
echo "  ✓ Isolated venv initialized at ${TEMP_VENV}"

echo "=== [3/4] Installing Wheel into Isolated Environment ==="
(cd "${TEMP_VENV}" && "${UV_CMD[@]}" pip install "../${WHEEL_FILE}")
echo "  ✓ Installed package cleanly from wheel"

echo "=== [4/4] Executing Clean-Environment Smoke Test ==="
PYTHON_CMD="bin/python"
if [[ -f "${TEMP_VENV}/Scripts/python.exe" ]]; then
    PYTHON_CMD="Scripts/python.exe"
fi
(cd "${TEMP_VENV}" && "./${PYTHON_CMD}" -c "
import sunborn
from sunborn import Sunborn, AsyncSunborn, SunbornClient, AsyncSunbornClient
from sunborn.exceptions import (
    SunbornError, AuthenticationError, ForbiddenError,
    BadRequestError, NotFoundError, RateLimitError
)
from sunborn.models import CachePolicy, ExecutionTier, ToolResultRecord

print('  ✓ Package name       :', sunborn.__name__)
print('  ✓ Package version    :', sunborn.__version__)
print('  ✓ Client classes     :', Sunborn, AsyncSunborn)
print('  ✓ Model types        :', CachePolicy, ExecutionTier, ToolResultRecord)
print('  ✓ Exception hierarchy:', SunbornError, AuthenticationError)

client = Sunborn(base_url='http://127.0.0.1:8080', api_key='test-key')
assert client.base_url == 'http://127.0.0.1:8080/v1'
client.close()
print('  ✓ Synchronous lifecycle initialized and closed cleanly')
")

echo "=== Python SDK Package & Clean Installation Verification PASSED ==="
