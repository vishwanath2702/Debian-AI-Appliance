#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck disable=SC1091
source "$SCRIPT_DIR/variables.sh"

echo "========================================"
echo "      DAIA BUILD PIPELINE"
echo "========================================"
echo

echo "[1/5] Validating project..."
"$SCRIPT_DIR/check.sh"

echo
echo "[2/5] Cleaning workspace..."
"$SCRIPT_DIR/clean.sh"

echo
echo "[3/5] Building DAIA payload..."
"$SCRIPT_DIR/build-payload.sh"

echo
echo "[4/5] Building DAIA release binary..."
cargo build     --release     --manifest-path "$PROJECT_ROOT/engine/Cargo.toml"     -p cli

echo
echo "[5/5] Building DAIA ISO with Rust engine..."
"$PROJECT_ROOT/engine/target/release/daia"     build-iso     desktop     "$WORK_DIR/rootfs"     "$SOURCE_ISO"     "$WORK_DIR/iso-build"     "$OUTPUT_ISO"     "$WORK_DIR/payload/daia"

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
