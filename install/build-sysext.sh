#!/usr/bin/env bash
# build-sysext.sh: Package systemd-inferenced into an immutable systemd-sysext image
# Compatible with SteamOS, Fedora Silverblue/Bazzite, and openSUSE MicroOS.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
OUTPUT_DIR="${ROOT_DIR}/target/sysext"
STAGING="${OUTPUT_DIR}/staging"
IMAGE="${OUTPUT_DIR}/systemd-inferenced.raw"

echo "=== Building systemd-inferenced Sysext Image ==="

# 1. Compile release binaries
echo "[1/4] Compiling release binaries..."
cargo build --release --workspace

# 2. Assemble staging tree adhering strictly to /usr
echo "[2/4] Assembling /usr staging directory..."
rm -rf "${STAGING}"
mkdir -p "${STAGING}/usr/bin"
mkdir -p "${STAGING}/usr/lib/systemd/system"
mkdir -p "${STAGING}/usr/lib/udev/rules.d"
mkdir -p "${STAGING}/usr/lib/extension-release.d"

# Copy binaries
cp -p "${ROOT_DIR}/target/release/systemd-inferenced" "${STAGING}/usr/bin/"
cp -p "${ROOT_DIR}/target/release/inferenctl" "${STAGING}/usr/bin/"

# Copy systemd units and slices
cp -p "${ROOT_DIR}/systemd/systemd-inferenced.service" "${STAGING}/usr/lib/systemd/system/"
cp -p "${ROOT_DIR}/systemd/systemd-inferenced.socket" "${STAGING}/usr/lib/systemd/system/"
cp -p "${ROOT_DIR}/systemd/ai.slice" "${STAGING}/usr/lib/systemd/system/"
cp -p "${ROOT_DIR}/systemd/ai-sentry.slice" "${STAGING}/usr/lib/systemd/system/"
cp -p "${ROOT_DIR}/systemd/ai-batch.slice" "${STAGING}/usr/lib/systemd/system/"

# Copy udev rules
cp -p "${ROOT_DIR}/systemd/70-inferenced.rules" "${STAGING}/usr/lib/udev/rules.d/"

# Copy extension release metadata
cp -p "${ROOT_DIR}/systemd/extension-release.systemd-inferenced" \
      "${STAGING}/usr/lib/extension-release.d/extension-release.systemd-inferenced"

# 3. Build image (prefer EROFS for low memory decompression overhead, fallback to squashfs)
echo "[3/4] Creating image ${IMAGE}..."
mkdir -p "${OUTPUT_DIR}"
rm -f "${IMAGE}"

if command -v mkfs.erofs >/dev/null 2>&1; then
    echo "Using mkfs.erofs (zero-copy low-memory page cache integration)..."
    mkfs.erofs -zlz4 "${IMAGE}" "${STAGING}"
elif command -v mksquashfs >/dev/null 2>&1; then
    echo "Using mksquashfs (standard squashfs)..."
    mksquashfs "${STAGING}" "${IMAGE}" -comp zstd -b 64K -noappend
else
    echo "WARNING: Neither mkfs.erofs nor mksquashfs found. Staged tree available at: ${STAGING}"
    exit 0
fi

echo "[4/4] Sysext image generated successfully: ${IMAGE}"
echo "To deploy on an immutable host:"
echo "  sudo cp ${IMAGE} /var/lib/extensions/"
echo "  sudo systemd-sysext refresh"
