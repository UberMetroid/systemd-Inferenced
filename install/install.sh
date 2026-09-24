#!/usr/bin/env bash
# systemd-inferenced: Production Systemd Installer
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${SCRIPT_DIR}"

MODE="system"
BIN_DIR="/usr/bin"
UNIT_DIR="/usr/lib/systemd/system"
CONF_DIR="/etc/systemd"
RUN_DIR="/run/systemd-inferenced"
STATE_DIR="/var/lib/systemd-inferenced"
LOG_DIR="/var/log/systemd-inferenced"

for arg in "$@"; do
    case "${arg}" in
        --user)
            MODE="user"
            BIN_DIR="${HOME}/.local/bin"
            UNIT_DIR="${XDG_CONFIG_HOME:-${HOME}/.config}/systemd/user"
            CONF_DIR="${XDG_CONFIG_HOME:-${HOME}/.config}/systemd"
            RUN_DIR="${XDG_RUNTIME_DIR:-/tmp}/systemd-inferenced"
            STATE_DIR="${HOME}/.local/share/systemd-inferenced"
            LOG_DIR="${HOME}/.local/state/systemd-inferenced"
            ;;
        --help|-h)
            echo "Usage: $0 [--user|--system]"
            echo "  --system  Install system-wide as root (default if EUID==0)"
            echo "  --user    Install into user home directory"
            exit 0
            ;;
    esac
done

if [[ "${MODE}" == "system" && "${EUID}" -ne 0 ]]; then
    echo "Warning: Not running as root. Defaulting to --user mode installation."
    MODE="user"
    BIN_DIR="${HOME}/.local/bin"
    UNIT_DIR="${XDG_CONFIG_HOME:-${HOME}/.config}/systemd/user"
    CONF_DIR="${XDG_CONFIG_HOME:-${HOME}/.config}/systemd"
    RUN_DIR="${XDG_RUNTIME_DIR:-/tmp}/systemd-inferenced"
    STATE_DIR="${HOME}/.local/share/systemd-inferenced"
    LOG_DIR="${HOME}/.local/state/systemd-inferenced"
fi

echo "==> Installing systemd-inferenced in ${MODE} mode..."

# 1. Provision unprivileged system user and groups (system mode)
if [[ "${MODE}" == "system" ]]; then
    if ! getent group sentry >/dev/null 2>&1; then
        echo "Creating system group 'sentry'..."
        groupadd -r sentry 2>/dev/null || true
    fi
    if ! getent group inferenced >/dev/null 2>&1; then
        echo "Creating system group 'inferenced'..."
        groupadd -r inferenced 2>/dev/null || true
    fi
    if ! id -u inferenced >/dev/null 2>&1; then
        echo "Creating unprivileged system user 'inferenced'..."
        useradd -r -s /usr/sbin/nologin -g inferenced -G render,video,sentry \
            -d "${STATE_DIR}" -M inferenced 2>/dev/null || true
    else
        usermod -aG render,video,sentry inferenced 2>/dev/null || true
    fi
fi

# 2. Build binaries in release mode
echo "==> Compiling release binaries (systemd-inferenced, inferenctl)..."
if command -v cargo >/dev/null 2>&1; then
    cargo build --release -p systemd-inferenced -p inferenctl
else
    echo "Error: cargo is required to compile release binaries." >&2
    exit 1
fi

# 3. Install binaries
echo "==> Installing binaries to ${BIN_DIR}..."
mkdir -p "${BIN_DIR}"
install -Dm755 target/release/systemd-inferenced "${BIN_DIR}/systemd-inferenced"
install -Dm755 target/release/inferenctl "${BIN_DIR}/inferenctl"

# 4. Install systemd units & configuration
echo "==> Installing systemd unit files to ${UNIT_DIR}..."
mkdir -p "${UNIT_DIR}" "${CONF_DIR}"
install -Dm644 systemd/systemd-inferenced.service "${UNIT_DIR}/systemd-inferenced.service"
install -Dm644 systemd/systemd-inferenced.socket "${UNIT_DIR}/systemd-inferenced.socket"
install -Dm644 systemd/ai.slice "${UNIT_DIR}/ai.slice"
install -Dm644 systemd/ai-batch.slice "${UNIT_DIR}/ai-batch.slice"
install -Dm644 systemd/ai-sentry.slice "${UNIT_DIR}/ai-sentry.slice"

if [[ "${MODE}" == "system" ]]; then
    install -Dm644 systemd/sysusers.d/systemd-inferenced.conf "/usr/lib/sysusers.d/systemd-inferenced.conf" 2>/dev/null || true
    install -Dm644 systemd/tmpfiles.d/systemd-inferenced.conf "/usr/lib/tmpfiles.d/systemd-inferenced.conf" 2>/dev/null || true
fi

if [[ ! -f "${CONF_DIR}/inferenced.conf" ]]; then
    install -Dm644 systemd/inferenced.conf "${CONF_DIR}/inferenced.conf"
fi

# 5. Provision runtime, state, and log directories
echo "==> Setting up directories..."
mkdir -p "${RUN_DIR}" "${STATE_DIR}" "${LOG_DIR}"
if [[ "${MODE}" == "system" ]]; then
    chown -R inferenced:inferenced "${RUN_DIR}" "${STATE_DIR}" "${LOG_DIR}" 2>/dev/null || true
    chmod 0755 "${RUN_DIR}" "${STATE_DIR}" "${LOG_DIR}"
fi

# 6. Reload systemd daemon
echo "==> Reloading systemd..."
if [[ "${MODE}" == "system" ]]; then
    systemctl daemon-reload
    echo "==> Installation complete!"
    echo "To activate:"
    echo "  sudo systemctl enable --now systemd-inferenced.socket"
    echo "To verify:"
    echo "  inferenctl status"
else
    systemctl --user daemon-reload
    echo "==> User-mode installation complete!"
    echo "To activate:"
    echo "  systemctl --user enable --now systemd-inferenced.socket"
    echo "To verify:"
    echo "  inferenctl status"
fi
