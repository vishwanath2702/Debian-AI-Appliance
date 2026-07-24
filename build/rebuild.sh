#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck disable=SC1091
source "$SCRIPT_DIR/variables.sh"

echo "========================================"
echo " Building DAIA ISO"
echo "========================================"

[[ -d "$EXTRACT_DIR" ]] || {
    echo "ERROR: Extracted ISO directory not found:"
    echo "  $EXTRACT_DIR"
    exit 1
}

mkdir -p "$OUTPUT_DIR"
rm -f "$OUTPUT_ISO"

echo "Source ISO : $SOURCE_ISO"
echo "Extract tree: $EXTRACT_DIR"
echo "Output ISO : $OUTPUT_ISO"
echo

xorriso \
    -indev "$SOURCE_ISO" \
    -outdev "$OUTPUT_ISO" \
    -boot_image any replay \
    -map "$EXTRACT_DIR" / \
    -commit

[[ -f "$OUTPUT_ISO" ]] || {
    echo "ERROR: ISO creation completed without producing:"
    echo "  $OUTPUT_ISO"
    exit 1
}

echo
echo "ISO created successfully:"
echo "  $OUTPUT_ISO"
