#!/bin/bash
# ═══════════════════════════════════════════════════════════════════
# Generate Ed25519 keypair for Admin Console authentication
# ═══════════════════════════════════════════════════════════════════
#
# Usage:
#   ./scripts/gen-admin-keys.sh
#
# Output (both in admin_keys/):
#   admin_keys/admin_pub.pem   ← Public key (embedded in binary)
#   admin_keys/admin_priv.pem  ← Private key (embedded in binary)
#
# Both keys are compiled into the unikernel at build time.
# The private key is served to the browser so the admin console
# can auto-sign login challenges — no file picker needed.
#
# Security model: whoever builds the binary controls the keypair.
# Network access to /admin/ is the security boundary.
# ═══════════════════════════════════════════════════════════════════
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"

PUB_KEY="admin_keys/admin_pub.pem"
PRIV_KEY="admin_keys/admin_priv.pem"

echo "Generating Ed25519 keypair for admin console..."
echo ""

# Generate private key
openssl genpkey -algorithm Ed25519 -out "$PRIV_KEY"

# Extract public key
openssl pkey -in "$PRIV_KEY" -pubout -out "$PUB_KEY"

echo "Done!"
echo ""
echo "  Public key:  $PUB_KEY"
echo "  Private key: $PRIV_KEY"
echo ""
echo "Both keys will be embedded in the binary at build time."
echo ""
echo "Next steps:"
echo "  1. Rebuild: cargo build --target x86_64-unknown-hermit"
echo "  2. Open https://<appliance>/admin/ and click Sign In"
echo ""
echo "To replace keys: re-run this script and rebuild."
