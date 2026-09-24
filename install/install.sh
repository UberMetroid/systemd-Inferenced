#!/usr/bin/env bash
# systemd-inferenced: Zero-Trust Systemd Installer
set -euo pipefail

echo "==> Installing systemd-inferenced..."

# 1. Provision unprivileged system user
if ! id -u inferenced >/dev/null 2>&1; then
    echo "Creating system user 'inferenced'..."
    useradd -r -s /usr/sbin/nologin -d /var/lib/systemd-inferenced -M inferenced
    usermod -aG render,video inferenced 2>/dev/null || true
fi

# 2. Build binaries in release mode
echo "Compiling release binaries..."
cargo build --release -p systemd-inferenced -p inferenctl

# 3. Install binaries to /usr/bin
install -Dm755 target/release/systemd-inferenced /usr/bin/systemd-inferenced
install -Dm755 target/release/inferenctl /usr/bin/inferenctl

# 4. Install systemd units & configuration
install -Dm644 systemd/systemd-inferenced.service /usr/lib/systemd/system/systemd-inferenced.service
install -Dm644 systemd/systemd-inferenced.socket /usr/lib/systemd/system/systemd-inferenced.socket
install -Dm644 systemd/ai.slice /usr/lib/systemd/system/ai.slice
install -Dm644 systemd/ai-sentry.slice /usr/lib/systemd/system/ai-sentry.slice
install -Dm644 systemd/inferenced.conf /etc/systemd/inferenced.conf

# 5. Create runtime directories
mkdir -p /run/systemd-inferenced /var/lib/systemd-inferenced /var/log/systemd-inferenced
chown -R inferenced:inferenced /run/systemd-inferenced /var/lib/systemd-inferenced /var/log/systemd-inferenced

# 6. Reload and enable systemd units
systemctl daemon-reload
echo "==> Installation complete! Run 'systemctl enable --now systemd-inferenced.socket' to start."
