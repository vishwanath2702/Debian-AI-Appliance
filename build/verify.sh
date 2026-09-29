#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck source=variables.sh
# shellcheck disable=SC1091
source "$SCRIPT_DIR/variables.sh"

echo "========================================"
echo " Verifying Build"
echo "========================================"

fail=0

check() {

    if [[ -e "$1" ]]; then
        printf "[ OK ] %s\n" "$1"
    else
        printf "[FAIL] %s\n" "$1"
        fail=1
    fi
}

#
# Verify extracted files
#

check "$EXTRACT_DIR/daia/opt/daia/runtime.sh"

check "$EXTRACT_DIR/daia/opt/daia/firstboot.sh"

check "$EXTRACT_DIR/daia/etc/systemd/system/daia-firstboot.service"

check "$EXTRACT_DIR/daia/opt/daia/BUILD-INFO"

check "$EXTRACT_DIR/daia/opt/daia/config/pragna.conf"

check "$EXTRACT_DIR/daia/opt/daia/lib/logging.sh"

check "$EXTRACT_DIR/daia/opt/daia/lib/common.sh"

check "$EXTRACT_DIR/daia/opt/daia/lib/validation.sh"

if [[ $fail -eq 1 ]]; then
    echo
    echo "Verification FAILED."
    exit 1
fi

echo
echo "Verification successful."
