#!/usr/bin/env bash
# systemd-inferenced: Production Uninstaller
set -euo pipefail

MODE="system"
PURGE=false
BIN_DIR="/usr/bin"
ALT_BIN_DIR="/usr/local/bin"
UNIT_DIR="/usr/lib/systemd/system"
ALT_UNIT_DIR="/etc/systemd/system"
CONF_DIR="/etc/systemd"
RUN_DIR="/run/systemd-inferenced"
STATE_DIR="/var/lib/systemd-inferenced"
LOG_DIR="/var/log/systemd-inferenced"

for arg in "$@"; do
    case "${arg}" in
        --purge)
            PURGE=true
            ;;
        --user)
            MODE="user"
            BIN_DIR="${HOME}/.local/bin"
            ALT_BIN_DIR=""
            UNIT_DIR="${XDG_CONFIG_HOME:-${HOME}/.config}/systemd/user"
            ALT_UNIT_DIR=""
            CONF_DIR="${XDG_CONFIG_HOME:-${HOME}/.config}/systemd"
            RUN_DIR="${XDG_RUNTIME_DIR:-/tmp}/systemd-inferenced"
            STATE_DIR="${HOME}/.local/share/systemd-inferenced"
            LOG_DIR="${HOME}/.local/state/systemd-inferenced"
            ;;
        --help|-h)
            echo "Usage: $0 [--user|--system] [--purge]"
            echo "  --system  Uninstall system-wide installation (default if root)"
            echo "  --user    Uninstall user-mode installation"
            echo "  --purge   Remove configuration and persistent state data"
            exit 0
            ;;
    esac
done

if [[ "${MODE}" == "system" && "${EUID}" -ne 0 ]]; then
    echo "Notice: Non-root user detected; defaulting to --user uninstallation."
    MODE="user"
    BIN_DIR="${HOME}/.local/bin"
    ALT_BIN_DIR=""
    UNIT_DIR="${XDG_CONFIG_HOME:-${HOME}/.config}/systemd/user"
    ALT_UNIT_DIR=""
    CONF_DIR="${XDG_CONFIG_HOME:-${HOME}/.config}/systemd"
    RUN_DIR="${XDG_RUNTIME_DIR:-/tmp}/systemd-inferenced"
    STATE_DIR="${HOME}/.local/share/systemd-inferenced"
    LOG_DIR="${HOME}/.local/state/systemd-inferenced"
fi

echo "==> Uninstalling systemd-inferenced (${MODE} mode)..."

# 1. Stop and disable systemd service and socket units
echo "==> Stopping active units..."
if [[ "${MODE}" == "system" ]]; then
    systemctl stop systemd-inferenced.service 2>/dev/null || true
    systemctl stop systemd-inferenced.socket 2>/dev/null || true
    systemctl disable systemd-inferenced.service 2>/dev/null || true
    systemctl disable systemd-inferenced.socket 2>/dev/null || true
else
    systemctl --user stop systemd-inferenced.service 2>/dev/null || true
    systemctl --user stop systemd-inferenced.socket 2>/dev/null || true
    systemctl --user disable systemd-inferenced.service 2>/dev/null || true
    systemctl --user disable systemd-inferenced.socket 2>/dev/null || true
fi

# 2. Clean up runtime sockets and directories
echo "==> Cleaning up runtime sockets in ${RUN_DIR}..."
rm -f "${RUN_DIR}/io.systemd.inferenced1" 2>/dev/null || true
rm -f "${RUN_DIR}/sentry.sock" 2>/dev/null || true
rm -f "${RUN_DIR}/fd.sock" 2>/dev/null || true
rm -f "${RUN_DIR}/io.sock" 2>/dev/null || true
rm -rf "${RUN_DIR}" 2>/dev/null || true

# 3. Remove installed systemd unit files
echo "==> Removing systemd unit files..."
for dir in "${UNIT_DIR}" ${ALT_UNIT_DIR}; do
    if [[ -d "${dir}" ]]; then
        rm -f "${dir}/systemd-inferenced.service"
        rm -f "${dir}/systemd-inferenced.socket"
        rm -f "${dir}/ai.slice"
        rm -f "${dir}/ai-batch.slice"
        rm -f "${dir}/ai-sentry.slice"
    fi
done

if [[ "${MODE}" == "system" ]]; then
    rm -f /usr/lib/sysusers.d/systemd-inferenced.conf 2>/dev/null || true
    rm -f /usr/lib/tmpfiles.d/systemd-inferenced.conf 2>/dev/null || true
fi

# 4. Remove installed binaries
echo "==> Removing executable binaries..."
for dir in "${BIN_DIR}" ${ALT_BIN_DIR}; do
    if [[ -n "${dir}" && -d "${dir}" ]]; then
        rm -f "${dir}/systemd-inferenced"
        rm -f "${dir}/inferenctl"
    fi
done

# 5. Purge persistent state and config if requested
if [[ "${PURGE}" == true ]]; then
    echo "==> Purging configuration and state..."
    rm -f "${CONF_DIR}/inferenced.conf" 2>/dev/null || true
    rm -rf "${CONF_DIR}/inferenced.conf.d" 2>/dev/null || true
    rm -rf "${STATE_DIR}" 2>/dev/null || true
    rm -rf "${LOG_DIR}" 2>/dev/null || true
fi

# 6. Reload systemd daemon
echo "==> Reloading systemd daemon..."
if [[ "${MODE}" == "system" ]]; then
    systemctl daemon-reload 2>/dev/null || true
    systemctl reset-failed 2>/dev/null || true
else
    systemctl --user daemon-reload 2>/dev/null || true
    systemctl --user reset-failed 2>/dev/null || true
fi

echo "==> Uninstallation complete."
