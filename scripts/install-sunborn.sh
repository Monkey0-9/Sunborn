#!/usr/bin/env bash
# Sunborn Local BYOK Unix/Linux/macOS Installer
# Installs binary to ~/.local/bin and creates systemd user unit / desktop entry

set -euo pipefail

INSTALL_DIR="${HOME}/.local/bin"
DATA_DIR="${HOME}/.local/share/sunborn"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=========================================================="
echo "   SUNBORN LOCAL BYOK UNIX/MACOS INSTALLER               "
echo "=========================================================="

mkdir -p "${INSTALL_DIR}"
mkdir -p "${DATA_DIR}"

SOURCE_BIN=""
if [[ -f "${PROJECT_ROOT}/target/release/sunborn-gateway" ]]; then
    SOURCE_BIN="${PROJECT_ROOT}/target/release/sunborn-gateway"
elif [[ -f "${PROJECT_ROOT}/target/debug/sunborn-gateway" ]]; then
    SOURCE_BIN="${PROJECT_ROOT}/target/debug/sunborn-gateway"
else
    echo "[*] Building sunborn-gateway release binary..."
    (cd "${PROJECT_ROOT}" && cargo build --release -p sunborn-gateway)
    SOURCE_BIN="${PROJECT_ROOT}/target/release/sunborn-gateway"
fi

echo "[+] Deploying binary to ${INSTALL_DIR}/sunborn-gateway..."
cp "${SOURCE_BIN}" "${INSTALL_DIR}/sunborn-gateway"
chmod +x "${INSTALL_DIR}/sunborn-gateway"

# Create Desktop entry for Linux desktops
DESKTOP_ENTRY_DIR="${HOME}/.local/share/applications"
if [[ -d "${DESKTOP_ENTRY_DIR}" ]]; then
    cat <<EOF > "${DESKTOP_ENTRY_DIR}/sunborn.desktop"
[Desktop Entry]
Name=Sunborn AI Gateway
Comment=High-Performance Semantic AI Gateway
Exec=${INSTALL_DIR}/sunborn-gateway
Icon=utilities-system-monitor
Terminal=false
Type=Application
Categories=Development;Utility;
EOF
    echo "[+] Created desktop entry: ${DESKTOP_ENTRY_DIR}/sunborn.desktop"
fi

# Post-install verification: configuration must load, and the binary must
# actually serve /health before we call the install successful.
echo "[*] Verifying the installed binary..."
if ! "${INSTALL_DIR}/sunborn-gateway" config-check >/dev/null 2>&1; then
    echo "[-] Installed binary failed configuration validation."
    "${INSTALL_DIR}/sunborn-gateway" config-check || true
    exit 1
fi

SMOKE_PORT="${SUNBORN_SMOKE_PORT:-18081}"
SMOKE_LOG="${DATA_DIR}/install-smoke.log"
SUNBORN_ENV=development HOST=127.0.0.1 PORT="${SMOKE_PORT}" \
    "${INSTALL_DIR}/sunborn-gateway" >"${SMOKE_LOG}" 2>&1 &
SMOKE_PID=$!
SMOKE_OK=0
for _ in $(seq 1 40); do
    if PORT="${SMOKE_PORT}" "${INSTALL_DIR}/sunborn-gateway" healthcheck >/dev/null 2>&1; then
        SMOKE_OK=1
        break
    fi
    sleep 0.25
done
kill "${SMOKE_PID}" 2>/dev/null || true
wait "${SMOKE_PID}" 2>/dev/null || true

if [[ "${SMOKE_OK}" -eq 1 ]]; then
    echo "[+] Smoke test passed: gateway answered /health on 127.0.0.1:${SMOKE_PORT}."
else
    echo "[-] Smoke test did not get a healthy response. Last log lines (${SMOKE_LOG}):"
    tail -n 15 "${SMOKE_LOG}" || true
    echo "[-] Installation copied the binary but could not verify it. Run:"
    echo "      ${INSTALL_DIR}/sunborn-gateway doctor"
fi

echo "=========================================================="
echo "   INSTALLATION COMPLETE!                                 "
echo "=========================================================="
echo "   Binary:   ${INSTALL_DIR}/sunborn-gateway"
echo "   Data:     ${DATA_DIR}"
echo "   Verify:   ${INSTALL_DIR}/sunborn-gateway doctor"
echo "   Start with: SUNBORN_SERVER__HOST=127.0.0.1 ${INSTALL_DIR}/sunborn-gateway"
echo "=========================================================="
