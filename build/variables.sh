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

MOUNT_DIR="$WORK_DIR/mount"
EXTRACT_DIR="$WORK_DIR/extract"

VERSION_FILE="$PROJECT_ROOT/VERSION"

# Select the Debian source ISO.
#
# DAIA_SOURCE_ISO takes precedence. Automatic discovery is allowed
# only when exactly one matching ISO exists.
if [[ -n "${DAIA_SOURCE_ISO:-}" ]]; then
    SOURCE_ISO="$DAIA_SOURCE_ISO"

    if [[ ! -f "$SOURCE_ISO" ]]; then
        echo "ERROR: Selected Debian netinst ISO not found:"
        echo "  $SOURCE_ISO"
        exit 1
    fi
else
    mapfile -t SOURCE_ISOS < <(
        find "$ISO_DIR" -maxdepth 1 -type f \
            -name "debian-*-amd64-netinst.iso" \
            -print \
            | sort
    )

    if [[ "${#SOURCE_ISOS[@]}" -eq 0 ]]; then
        echo "ERROR: No Debian netinst ISO found in:"
        echo "  $ISO_DIR"
        exit 1
    fi

    if [[ "${#SOURCE_ISOS[@]}" -ne 1 ]]; then
        echo "ERROR: Multiple Debian netinst ISOs found in:"
        echo "  $ISO_DIR"
        echo "Set DAIA_SOURCE_ISO explicitly to select one."
        printf '  %s\n' "${SOURCE_ISOS[@]}"
        exit 1
    fi

    SOURCE_ISO="${SOURCE_ISOS[0]}"
fi

ISO_NAME="$(basename "$SOURCE_ISO")"

# Read project version
if [[ -f "$VERSION_FILE" ]]; then
    VERSION="$(cat "$VERSION_FILE")"
else
    VERSION="dev"
fi

OUTPUT_ISO="$OUTPUT_DIR/daia-${VERSION}.iso"
