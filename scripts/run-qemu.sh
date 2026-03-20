#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════
# Run the Rust Web Appliance in QEMU
# ═══════════════════════════════════════════════════════════════════
#
# Serial Lifeline (Task 4):
#   ALL println!/eprintln! output goes to COM1 serial.
#   QEMU's `-serial stdio` pipes this to your terminal.
#   If the kernel panics, you'll see the backtrace here.
#
# Usage:
#   ./scripts/run-qemu.sh              # Basic mode (user networking)
#   ./scripts/run-qemu.sh --tap        # TAP networking (needs sudo)
#   ./scripts/run-qemu.sh --virtiofs   # TAP + VirtioFS (full stack)
#
# Port forwarding (basic mode):
#   Host http://localhost:10080  → Guest :80   (HTTP redirect)
#   Host https://localhost:9443  → Guest :443  (primary HTTPS)
#   Host https://localhost:18443 → Guest :8443 (API / mobile HTTPS)
# ═══════════════════════════════════════════════════════════════════
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"

MODE="${1:---basic}"

# ── Locate binaries ─────────────────────────────────────────────
BINARY="target/x86_64-unknown-hermit/release/rust-web-appliance"
if [ ! -f "$BINARY" ]; then
    BINARY="target/x86_64-unknown-hermit/debug/rust-web-appliance"
fi
if [ ! -f "$BINARY" ]; then
    echo "ERROR: No binary found. Run ./scripts/build.sh first."
    exit 1
fi

# hermit-loader: check common locations
LOADER=""
for candidate in \
    "$PROJECT_ROOT/hermit-loader-x86_64" \
    "$PROJECT_ROOT/hermit-loader" \
    "$HOME/.cargo/bin/hermit-loader-x86_64" \
    "/usr/local/bin/hermit-loader-x86_64"; do
    if [ -f "$candidate" ]; then
        LOADER="$candidate"
        break
    fi
done

if [ -z "$LOADER" ]; then
    echo "════════════════════════════════════════════════════"
    echo "  hermit-loader not found!"
    echo ""
    echo "  Download it:"
    echo "    curl -L -o hermit-loader-x86_64 \\"
    echo "      https://github.com/hermit-os/loader/releases/latest/download/hermit-loader-x86_64"
    echo "    chmod +x hermit-loader-x86_64"
    echo ""
    echo "  Or build from source:"
    echo "    git clone https://github.com/hermit-os/loader.git"
    echo "    cd loader && cargo xtask build --target x86_64 --release"
    echo "════════════════════════════════════════════════════"
    exit 1
fi

echo "══════════════════════════════════════════════════════"
echo "  Rust Web Appliance — QEMU Launch"
echo "  Loader: $LOADER"
echo "  Binary: $BINARY"
echo "  Mode:   $MODE"
echo "══════════════════════════════════════════════════════"
echo
echo "  Serial output below (COM1 → stdio)"
echo "  Press Ctrl+A, X to quit QEMU"
echo "──────────────────────────────────────────────────────"
echo

# ── Common QEMU args ────────────────────────────────────────────
QEMU_BASE=(
    qemu-system-x86_64
    -smp 1
    -m 256M
    -display none
    -serial stdio
    -device "isa-debug-exit,iobase=0xf4,iosize=0x04"
    -kernel "$LOADER"
    -initrd "$BINARY"
)

# Enable KVM if available
if [ -e /dev/kvm ]; then
    QEMU_BASE+=(-enable-kvm -cpu host)
    echo "[qemu] KVM acceleration enabled"
else
    QEMU_BASE+=(-cpu qemu64,apic,fsgsbase,rdtscp,xsave,xsaveopt,fxsr)
    echo "[qemu] Software emulation (no KVM)"
fi

case "$MODE" in
    --basic)
        # User-mode networking with port forwarding
        # No root required. HTTPS accessible at localhost:9443
        # RTL8139 NIC is REQUIRED for QEMU user-mode (SLIRP) networking.
        # DHCP (dhcpv4 feature) handles IP assignment — SLIRP assigns 10.0.2.15.
        echo "[qemu] User-mode networking (RTL8139 / SLIRP):"
        echo "[qemu]   localhost:10080 → guest:80   (HTTP redirect)"
        echo "[qemu]   localhost:9443  → guest:443  (primary HTTPS)"
        echo "[qemu]   localhost:18443 → guest:8443 (API / mobile HTTPS)"
        exec "${QEMU_BASE[@]}" \
            -netdev "user,id=u1,hostfwd=tcp::10080-:80,hostfwd=tcp::9443-:443,hostfwd=tcp::18443-:8443" \
            -device "rtl8139,netdev=u1"
        ;;

    --tap)
        # TAP networking — requires setup:
        #   sudo ip tuntap add tap10 mode tap user $USER
        #   sudo ip addr add 10.0.5.1/24 dev tap10
        #   sudo ip link set dev tap10 up
        echo "[qemu] TAP networking: tap10"
        exec "${QEMU_BASE[@]}" \
            -netdev "tap,id=net0,ifname=tap10,script=no,downscript=no" \
            -device "virtio-net-pci,netdev=net0,disable-legacy=on"
        ;;

    --virtiofs)
        # Full stack: TAP + VirtioFS
        # Requires virtiofsd running:
        #   mkdir -p /tmp/guestfs/www /tmp/guestfs/data /tmp/guestfs/backend
        #   cp -r frontend/* /tmp/guestfs/www/        # sync frontend drop zone
        #   cp backend/endpoints.json /tmp/guestfs/backend/  # sync API config
        #   virtiofsd --socket-path=/tmp/vhostqemu --shared-dir=/tmp/guestfs
        echo "[qemu] TAP networking + VirtioFS"
        exec "${QEMU_BASE[@]}" \
            -netdev "tap,id=net0,ifname=tap10,script=no,downscript=no" \
            -device "virtio-net-pci,netdev=net0,disable-legacy=on" \
            -chardev "socket,id=char0,path=/tmp/vhostqemu" \
            -device "vhost-user-fs-pci,queue-size=1024,chardev=char0,tag=root" \
            -object "memory-backend-file,id=mem,size=256M,mem-path=/dev/shm,share=on" \
            -numa "node,memdev=mem"
        ;;

    *)
        echo "Unknown mode: $MODE"
        echo "Usage: $0 [--basic|--tap|--virtiofs]"
        exit 1
        ;;
esac
