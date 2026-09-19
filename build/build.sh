#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck disable=SC1091
source "$SCRIPT_DIR/variables.sh"

echo "========================================"
echo "      DAIA BUILD PIPELINE"
echo "========================================"
echo

echo "[1/8] Validating project..."
"$SCRIPT_DIR/check.sh"

echo
echo "[2/8] Cleaning workspace..."
"$SCRIPT_DIR/clean.sh"

echo
echo "[3/8] Extracting Debian ISO..."
"$SCRIPT_DIR/extract.sh"

echo
echo "[4/8] Patching boot configuration..."
"$SCRIPT_DIR/patch.sh"

echo
echo "[5/8] Building DAIA payload..."
"$SCRIPT_DIR/build-payload.sh"

echo
echo "[6/8] Injecting DAIA..."
"$SCRIPT_DIR/inject.sh"

echo
echo "[7/8] Verifying injection..."
"$SCRIPT_DIR/verify.sh"

echo
echo "[8/8] Building ISO..."
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
