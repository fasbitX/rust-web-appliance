#!/bin/bash
# ═══════════════════════════════════════════════════════════════════
# Generate Ed25519 keypair for Admin Console authentication
# ═══════════════════════════════════════════════════════════════════
#
# Usage:
#   ./scripts/gen-admin-keys.sh
#
# Output:
#   admin_keys/admin_pub.pem  ← Public key (committed, embedded in binary)
#   admin_priv.pem            ← Private key (KEEP SECRET, never commit)
#
# The public key is compiled into the unikernel at build time.
# The private key stays on your workstation — you select it in the
# browser to authenticate to the admin console.
# ═══════════════════════════════════════════════════════════════════
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"

PUB_KEY="admin_keys/admin_pub.pem"
PRIV_KEY="admin_priv.pem"

echo "Generating Ed25519 keypair for admin console..."
echo ""

# Generate private key
openssl genpkey -algorithm Ed25519 -out "$PRIV_KEY"

# Extract public key
openssl pkey -in "$PRIV_KEY" -pubout -out "$PUB_KEY"

echo "Done!"
echo ""
echo "  Public key:  $PUB_KEY  (committed to repo, embedded in binary)"
echo "  Private key: $PRIV_KEY (KEEP SECRET — never commit this!)"
echo ""
echo "Next steps:"
echo "  1. Rebuild: cargo build --target x86_64-unknown-hermit"
echo "  2. Open https://<appliance>/admin/ in your browser"
echo "  3. Select $PRIV_KEY when prompted to sign in"
echo ""
echo "To replace keys: re-run this script and rebuild."
