#!/bin/bash
set -euo pipefail

# shellcheck disable=SC1091
source build/variables.sh
echo "========================================"
echo " Patching Debian Installer"
echo "========================================"

patch_file() {
    local file="$1"

    if [[ ! -f "$file" ]]; then
        echo "Skipping missing file: $file"
        return
    fi

    if grep -q "preseed/file=/cdrom/preseed.cfg" "$file"; then
        echo "[OK] Already patched: $file"
        return
    fi

    sed -i \
        's|--- quiet|preseed/file=/cdrom/preseed.cfg --- quiet|g' \
        "$file"

    if grep -q "preseed/file=/cdrom/preseed.cfg" "$file"; then
        echo "[OK] Patched: $file"
    else
        echo "[FAIL] Could not patch: $file"
        exit 1
    fi
}

patch_file "$EXTRACT_DIR/isolinux/txt.cfg"
patch_file "$EXTRACT_DIR/boot/grub/grub.cfg"

echo
echo "Patch complete."
