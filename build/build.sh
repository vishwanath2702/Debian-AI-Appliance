#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck disable=SC1091
source "$SCRIPT_DIR/variables.sh"

echo "========================================"
echo "      DAIA BUILD PIPELINE"
echo "========================================"
echo

echo "[1/7] Validating project..."
"$SCRIPT_DIR/check.sh"

echo
echo "[2/7] Cleaning workspace..."
"$SCRIPT_DIR/clean.sh"

echo
echo "[3/7] Extracting Debian ISO..."
"$SCRIPT_DIR/extract.sh"

echo
echo "[4/7] Patching boot configuration..."
"$SCRIPT_DIR/patch.sh"

echo
echo "[5/7] Injecting DAIA..."
"$SCRIPT_DIR/inject.sh"

echo
echo "[6/7] Verifying injection..."
"$SCRIPT_DIR/verify.sh"

echo
echo "[7/7] Building ISO..."
"$SCRIPT_DIR/rebuild.sh"

echo
echo "Calculating SHA256..."
sha256sum "$OUTPUT_ISO"

echo
echo "ISO size:"
du -h "$OUTPUT_ISO"

echo
echo "========================================"
echo " DAIA build completed successfully."
echo "========================================"
echo "ISO:"
echo "  $OUTPUT_ISO"
