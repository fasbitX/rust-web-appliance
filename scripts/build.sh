#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════
# Build the Rust Web Appliance for HermitOS
# ═══════════════════════════════════════════════════════════════════
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"

MODE="${1:-release}"

echo "══════════════════════════════════════════════"
echo "  Building Rust Web Appliance (${MODE})"
echo "══════════════════════════════════════════════"
echo

# Ensure nightly toolchain with rust-src (needed for build-std)
echo "[1/3] Checking toolchain..."
rustup component add rust-src --toolchain nightly 2>/dev/null || true
echo "      Toolchain ready"
echo

# Build for hermit target
echo "[2/3] Compiling for x86_64-unknown-hermit..."
if [ "$MODE" = "release" ]; then
    cargo build \
        -Z build-std=std,core,alloc,panic_abort \
        --target x86_64-unknown-hermit \
        --release
    BINARY="target/x86_64-unknown-hermit/release/rust-web-appliance"
else
    cargo build \
        -Z build-std=std,core,alloc,panic_abort \
        --target x86_64-unknown-hermit
    BINARY="target/x86_64-unknown-hermit/debug/rust-web-appliance"
fi
echo "      Binary: ${BINARY}"
echo

# Show binary info
echo "[3/3] Binary info:"
ls -lh "$BINARY"
file "$BINARY" 2>/dev/null || true
echo

echo "══════════════════════════════════════════════"
echo "  Build complete!"
echo "  Binary: ${BINARY}"
echo ""
echo "  Next steps:"
echo "    ./scripts/run-qemu.sh          # Test in QEMU"
echo "    ./scripts/make-image.sh        # Create bootable .img"
echo "══════════════════════════════════════════════"
