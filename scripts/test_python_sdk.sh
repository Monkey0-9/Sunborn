#!/usr/bin/env bash
# ==============================================================================
# Sunborn Python SDK: Build & Clean-Environment Installation Verification
# Validates PEP 517/621 packaging, wheel distribution, and clean-room installation.
# ==============================================================================
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SDK_DIR="${ROOT_DIR}/sdk/python"
TEMP_VENV="$(mktemp -d /tmp/sunborn_venv_XXXXXX)"

cleanup() {
    rm -rf "${TEMP_VENV}"
}
trap cleanup EXIT

echo "=== [1/4] Building Sunborn Python SDK Package (sdist + wheel) ==="
cd "${SDK_DIR}"
rm -rf dist build *.egg-info
uv build

WHEEL_FILE="$(ls -1 dist/*.whl | head -n 1)"
echo "  ✓ Successfully built wheel: ${WHEEL_FILE}"

echo "=== [2/4] Creating Clean Isolated Environment ==="
uv venv "${TEMP_VENV}"
echo "  ✓ Isolated venv initialized at ${TEMP_VENV}"

echo "=== [3/4] Installing Wheel into Isolated Environment ==="
VIRTUAL_ENV="${TEMP_VENV}" uv pip install "${WHEEL_FILE}"
echo "  ✓ Installed package cleanly from wheel"

echo "=== [4/4] Executing Clean-Environment Smoke Test ==="
(cd "${TEMP_VENV}" && "${TEMP_VENV}/bin/python" -c "
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
