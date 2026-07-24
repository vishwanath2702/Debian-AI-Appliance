#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck disable=SC1091
source "$SCRIPT_DIR/variables.sh"

echo "========================================"
echo " Extracting Debian ISO"
echo "========================================"

rm -rf "$EXTRACT_DIR"
mkdir -p "$EXTRACT_DIR"

xorriso \
    -osirrox on \
    -indev "$SOURCE_ISO" \
    -extract / "$EXTRACT_DIR"

chmod -R u+w "$EXTRACT_DIR"

echo
echo "ISO extracted to:"
echo "  $EXTRACT_DIR"
