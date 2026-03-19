#!/bin/bash
# ═══════════════════════════════════════════════════════════════════
# Generate TLS certificates for Rust Web Appliance
# ═══════════════════════════════════════════════════════════════════
#
# Usage:
#   ./scripts/gen-cert.sh                    # Generate certs in ./certs/
#   ./scripts/gen-cert.sh /path/to/output    # Generate to custom directory
#
# The generated certs should be placed in the VirtioFS shared
# directory at /data/tls/ for the appliance to load them.
# ═══════════════════════════════════════════════════════════════════

set -euo pipefail

OUT_DIR="${1:-./certs}"
CERT_FILE="$OUT_DIR/cert.pem"
KEY_FILE="$OUT_DIR/key.pem"
DAYS=3650
CN="RustWebAppliance"

mkdir -p "$OUT_DIR"

echo "Generating EC P-256 TLS certificate..."
echo "  Output: $OUT_DIR"
echo "  CN:     $CN"
echo "  Valid:  $DAYS days"
echo ""

openssl req -x509 \
    -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
    -keyout "$KEY_FILE" \
    -out "$CERT_FILE" \
    -days "$DAYS" \
    -nodes \
    -subj "/CN=$CN" \
    -addext "subjectAltName=DNS:localhost,IP:127.0.0.1"

echo ""
echo "Done! Certificate files:"
echo "  Cert: $CERT_FILE"
echo "  Key:  $KEY_FILE"
echo ""
echo "To use with the appliance, copy to VirtioFS shared dir:"
echo "  cp $CERT_FILE /path/to/shared/data/tls/cert.pem"
echo "  cp $KEY_FILE  /path/to/shared/data/tls/key.pem"
