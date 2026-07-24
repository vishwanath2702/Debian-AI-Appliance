#!/bin/bash
# shellcheck disable=SC2034

set -euo pipefail

# --------------------------------------------------
# DAIA Build Variables
# --------------------------------------------------

# shellcheck disable=SC2034

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Directories
ISO_DIR="$PROJECT_ROOT/iso"
WORK_DIR="$PROJECT_ROOT/work"
OUTPUT_DIR="$PROJECT_ROOT/output"
INSTALLER_DIR="$PROJECT_ROOT/installer"

MOUNT_DIR="$WORK_DIR/mount"
EXTRACT_DIR="$WORK_DIR/extract"

VERSION_FILE="$PROJECT_ROOT/VERSION"

# Automatically locate Debian ISO
SOURCE_ISO=$(find "$ISO_DIR" -maxdepth 1 -type f \
    -name "debian-*-amd64-netinst.iso" | head -n1)

if [[ -z "$SOURCE_ISO" ]]; then
    echo "ERROR: No Debian netinst ISO found in:"
    echo "  $ISO_DIR"
    exit 1
fi

ISO_NAME="$(basename "$SOURCE_ISO")"

# Read project version
if [[ -f "$VERSION_FILE" ]]; then
    VERSION="$(cat "$VERSION_FILE")"
else
    VERSION="dev"
fi

OUTPUT_ISO="$OUTPUT_DIR/daia-${VERSION}.iso"
