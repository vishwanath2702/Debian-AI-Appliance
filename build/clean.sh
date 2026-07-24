#!/bin/bash
set -euo pipefail


SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck source=variables.sh
# shellcheck disable=SC1091
source "$SCRIPT_DIR/variables.sh"

echo "========================================"
echo " Cleaning Workspace"
echo "========================================"

#
# Unmount previous ISO if mounted
#

if mountpoint -q /tmp/daiaiso; then
    echo "Unmounting previous ISO..."
    sudo umount /tmp/daiaiso
fi

#
# Remove previous ISO
#

if [[ -f "$OUTPUT_ISO" ]]; then
    echo "Removing previous ISO..."
    rm -f "$OUTPUT_ISO"
fi

#
# Remove stale mount directory
#

rm -rf /tmp/daiaiso
mkdir -p /tmp/daiaiso

echo
echo "Workspace cleaned."
