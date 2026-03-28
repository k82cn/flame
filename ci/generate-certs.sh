#!/bin/bash
#
# Generate self-signed TLS certificates for Flame CI/development
#
# Usage: ./generate-certs.sh [output_dir] [san_list]
#   output_dir: Directory to output certificates (default: ./certs)
#   san_list:   Comma-separated list of SANs (default: localhost,127.0.0.1)
#
# Output files:
#   ca.crt     - CA certificate
#   ca.key     - CA private key
#   server.crt - Server certificate (signed by CA)
#   server.key - Server private key

set -e

OUTPUT_DIR="${1:-./certs}"
SAN_LIST="${2:-localhost,127.0.0.1}"
VALID_DAYS=365

echo "🔐 Generating TLS certificates..."
echo "   Output directory: $OUTPUT_DIR"
echo "   SANs: $SAN_LIST"
echo ""

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Build SAN extension string for openssl
SAN_EXT="subjectAltName = "
IFS=',' read -ra SANS <<< "$SAN_LIST"
FIRST=true
for san in "${SANS[@]}"; do
    san=$(echo "$san" | xargs)  # trim whitespace
    if [ "$FIRST" = true ]; then
        FIRST=false
    else
        SAN_EXT+=","
    fi
    # Check if it's an IP address
    if [[ "$san" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        SAN_EXT+="IP:$san"
    else
        SAN_EXT+="DNS:$san"
    fi
done

# Generate CA private key
echo "→ Generating CA private key..."
openssl genrsa -out "$OUTPUT_DIR/ca.key" 4096

# Generate CA certificate
echo "→ Generating CA certificate..."
openssl req -new -x509 -days $VALID_DAYS -key "$OUTPUT_DIR/ca.key" \
    -out "$OUTPUT_DIR/ca.crt" \
    -subj "/CN=Flame CA/O=Flame"

# Generate server private key
echo "→ Generating server private key..."
openssl genrsa -out "$OUTPUT_DIR/server.key" 4096

# Generate server CSR
echo "→ Generating server CSR..."
openssl req -new -key "$OUTPUT_DIR/server.key" \
    -out "$OUTPUT_DIR/server.csr" \
    -subj "/CN=flame-server/O=Flame"

# Create extensions file for SAN
cat > "$OUTPUT_DIR/server.ext" << EOF
authorityKeyIdentifier=keyid,issuer
basicConstraints=CA:FALSE
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
$SAN_EXT
EOF

# Sign server certificate with CA
echo "→ Signing server certificate with CA..."
openssl x509 -req -in "$OUTPUT_DIR/server.csr" \
    -CA "$OUTPUT_DIR/ca.crt" -CAkey "$OUTPUT_DIR/ca.key" \
    -CAcreateserial -out "$OUTPUT_DIR/server.crt" \
    -days $VALID_DAYS -extfile "$OUTPUT_DIR/server.ext"

# Clean up temporary files
rm -f "$OUTPUT_DIR/server.csr" "$OUTPUT_DIR/server.ext" "$OUTPUT_DIR/ca.srl"

# Set restrictive permissions on private keys
chmod 600 "$OUTPUT_DIR/ca.key" "$OUTPUT_DIR/server.key"

echo ""
echo "✓ Generated certificates in $OUTPUT_DIR:"
echo "  - ca.crt      (CA certificate)"
echo "  - ca.key      (CA private key)"
echo "  - server.crt  (Server certificate)"
echo "  - server.key  (Server private key)"
echo ""
echo "Server certificate SANs:"
openssl x509 -in "$OUTPUT_DIR/server.crt" -noout -ext subjectAltName | grep -v "X509v3" || echo "  $SAN_LIST"
