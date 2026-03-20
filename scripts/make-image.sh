#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════
# Create a bootable .img for DigitalOcean / bare-metal KVM
# ═══════════════════════════════════════════════════════════════════
#
# This creates a raw disk image with:
#   - MBR partition table
#   - ext2 partition with GRUB bootloader
#   - hermit-loader as the Multiboot kernel
#   - rust-web-appliance as the Multiboot module
#
# The image can be:
#   - Tested locally with: qemu-system-x86_64 -hda appliance.img
#   - Uploaded to DigitalOcean as a Custom Image
#   - dd'd to a real disk for bare-metal KVM
#
# Prerequisites: grub-install, fdisk, mke2fs, losetup (all standard)
# Must run as root (or with sudo) for losetup/mount
# ═══════════════════════════════════════════════════════════════════
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"

IMAGE_NAME="appliance.img"
IMAGE_SIZE_MB=64
BINARY="target/x86_64-unknown-hermit/release/rust-web-appliance"
LOADER="hermit-loader-x86_64"

# ── Preflight checks ────────────────────────────────────────────
echo "══════════════════════════════════════════════════════"
echo "  Creating bootable image: ${IMAGE_NAME}"
echo "══════════════════════════════════════════════════════"
echo

if [ "$(id -u)" -ne 0 ]; then
    echo "ERROR: This script must be run as root (needs losetup/mount)"
    echo "  sudo ./scripts/make-image.sh"
    exit 1
fi

if [ ! -f "$BINARY" ]; then
    echo "ERROR: Binary not found at $BINARY"
    echo "  Run: ./scripts/build.sh release"
    exit 1
fi

if [ ! -f "$LOADER" ]; then
    echo "ERROR: hermit-loader not found at $LOADER"
    echo "  Download: curl -L -o hermit-loader-x86_64 \\"
    echo "    https://github.com/hermit-os/loader/releases/latest/download/hermit-loader-x86_64"
    exit 1
fi

# ── Create raw disk image ───────────────────────────────────────
echo "[1/6] Creating ${IMAGE_SIZE_MB}MB raw image..."
dd if=/dev/zero of="$IMAGE_NAME" bs=1M count=$IMAGE_SIZE_MB status=progress

# ── Partition with MBR ───────────────────────────────────────────
echo "[2/6] Creating MBR partition table..."
# Create a single bootable partition spanning the disk
printf 'o\nn\np\n1\n2048\n\na\nw\n' | fdisk "$IMAGE_NAME" || true

# ── Set up loop device ──────────────────────────────────────────
echo "[3/6] Setting up loop device..."
LOOP_DEV=$(losetup --find --show --partscan "$IMAGE_NAME")
PART_DEV="${LOOP_DEV}p1"

# Wait for partition device to appear
sleep 1
if [ ! -b "$PART_DEV" ]; then
    partprobe "$LOOP_DEV" 2>/dev/null || true
    sleep 1
fi

# Cleanup trap
cleanup() {
    echo "[cleanup] Unmounting and detaching..."
    umount /tmp/appliance_mnt 2>/dev/null || true
    losetup -d "$LOOP_DEV" 2>/dev/null || true
    rmdir /tmp/appliance_mnt 2>/dev/null || true
}
trap cleanup EXIT

# ── Format partition ─────────────────────────────────────────────
echo "[4/6] Formatting partition as ext2..."
mke2fs -t ext2 -L "hermit" "$PART_DEV"

# ── Mount and install ────────────────────────────────────────────
echo "[5/6] Installing GRUB + appliance..."
mkdir -p /tmp/appliance_mnt
mount "$PART_DEV" /tmp/appliance_mnt

# Install GRUB
grub-install --target=i386-pc --boot-directory=/tmp/appliance_mnt/boot "$LOOP_DEV"

# Copy hermit-loader and application binary
mkdir -p /tmp/appliance_mnt/boot/hermit
cp "$LOADER"  /tmp/appliance_mnt/boot/hermit/loader
cp "$BINARY"  /tmp/appliance_mnt/boot/hermit/appliance

# Create GRUB config
cat > /tmp/appliance_mnt/boot/grub/grub.cfg << 'GRUBCFG'
set timeout=3
set default=0

menuentry "Rust Web Appliance" {
    multiboot /boot/hermit/loader
    module /boot/hermit/appliance
    boot
}
GRUBCFG

# Copy frontend files into the image at /www
if [ -d "frontend" ] && [ "$(find frontend -not -name '.gitkeep' -type f 2>/dev/null)" ]; then
    echo "      Copying frontend/ → /www ..."
    mkdir -p /tmp/appliance_mnt/www
    # Copy preserving directory structure, skip .gitkeep files
    cd frontend
    find . -type f ! -name '.gitkeep' -exec install -D {} /tmp/appliance_mnt/www/{} \;
    cd "$PROJECT_ROOT"
fi

# Copy backend config files into the image at /backend
if [ -d "backend" ] && [ -f "backend/endpoints.json" ]; then
    echo "      Copying backend/ → /backend ..."
    mkdir -p /tmp/appliance_mnt/backend
    cp backend/endpoints.json /tmp/appliance_mnt/backend/
fi

# Copy TLS certificates into the image at /data/tls
# The unikernel reads these at boot from /data/tls/cert.pem + key.pem
# If no certs are found, it falls back to the embedded dev certificate.
# To use Cloudflare Origin Certificates, place them in data/tls/ before
# building the image. See data/tls/README for instructions.
if [ -f "data/tls/cert.pem" ] && [ -f "data/tls/key.pem" ]; then
    echo "      Copying data/tls/ → /data/tls/ (production TLS certs)..."
    mkdir -p /tmp/appliance_mnt/data/tls
    cp data/tls/cert.pem /tmp/appliance_mnt/data/tls/cert.pem
    cp data/tls/key.pem  /tmp/appliance_mnt/data/tls/key.pem
    chmod 600 /tmp/appliance_mnt/data/tls/key.pem
else
    echo "      No TLS certs in data/tls/ — will use embedded dev certificate"
    echo "      (See data/tls/README for Cloudflare setup)"
    mkdir -p /tmp/appliance_mnt/data/tls
fi

sync
echo "      GRUB + loader + appliance installed"

# ── Finalize ─────────────────────────────────────────────────────
echo "[6/6] Finalizing..."
umount /tmp/appliance_mnt

echo
echo "══════════════════════════════════════════════════════"
echo "  Image created: ${IMAGE_NAME}"
echo "  Size: $(du -h "$IMAGE_NAME" | cut -f1)"
echo ""
echo "  Test locally:"
echo "    qemu-system-x86_64 -enable-kvm -m 256M \\"
echo "      -serial stdio -display none \\"
echo "      -hda ${IMAGE_NAME} \\"
echo "      -netdev user,id=u1,hostfwd=tcp::9443-:8443 \\"
echo "      -device rtl8139,netdev=u1"
echo ""
echo "    curl -vk https://localhost:9443/api/health"
echo ""
echo "  Upload to DigitalOcean:"
echo "    doctl compute image create rust-web-appliance \\"
echo "      --image-url <upload-url> --region nyc1"
echo "══════════════════════════════════════════════════════"
