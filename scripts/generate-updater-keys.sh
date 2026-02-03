#!/bin/bash
# Generate signing keys for Tauri auto-updater
#
# This script generates the public/private key pair used to sign updates.
# The private key should be stored as a GitHub secret (TAURI_SIGNING_PRIVATE_KEY)
# The public key should be added to tauri.conf.json
#
# Usage: ./scripts/generate-updater-keys.sh

set -e

echo "Generating Tauri updater signing keys..."
echo ""

# Check if tauri CLI is installed
if ! command -v cargo-tauri &> /dev/null && ! command -v tauri &> /dev/null; then
    echo "Installing Tauri CLI..."
    cargo install tauri-cli --locked
fi

# Generate keys
echo "Generating key pair..."
cargo tauri signer generate -w ~/.tauri/btrf-mount-windows.key

echo ""
echo "=========================================="
echo "Keys generated successfully!"
echo "=========================================="
echo ""
echo "1. Add the PRIVATE KEY to GitHub Secrets:"
echo "   - Go to: https://github.com/pegasusheavy/btrf-mount-windows/settings/secrets/actions"
echo "   - Create secret: TAURI_SIGNING_PRIVATE_KEY"
echo "   - Paste the contents of: ~/.tauri/btrf-mount-windows.key"
echo ""
echo "2. If you set a password, also add:"
echo "   - Create secret: TAURI_SIGNING_PRIVATE_KEY_PASSWORD"
echo ""
echo "3. Add the PUBLIC KEY to tauri.conf.json:"
echo "   - The public key was printed above"
echo "   - Add it to: plugins.updater.pubkey"
echo ""
