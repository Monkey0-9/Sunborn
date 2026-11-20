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

echo "=========================================================="
echo "   INSTALLATION COMPLETE!                                 "
echo "=========================================================="
echo "   Binary:   ${INSTALL_DIR}/sunborn-gateway"
echo "   Data:     ${DATA_DIR}"
echo "   Start with: SUNBORN_SERVER__HOST=127.0.0.1 ${INSTALL_DIR}/sunborn-gateway"
echo "=========================================================="
