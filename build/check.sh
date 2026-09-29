#!/bin/bash
set -euo pipefail

# shellcheck disable=SC1091
source "$(dirname "$0")/variables.sh"

PASS=0
FAIL=0

ok() {
    echo "[ OK ] $1"
    PASS=$((PASS+1))
}

error() {
    echo "[FAIL] $1"
    FAIL=$((FAIL+1))
}

check_file() {
    if [[ -f "$1" ]]; then
        ok "$1"
    else
        error "$1"
    fi
}

check_dir() {
    if [[ -d "$1" ]]; then
        ok "$1"
    else
        error "$1"
    fi
}

check_cmd() {
    if command -v "$1" >/dev/null 2>&1; then
        ok "$1"
    else
        error "$1"
    fi
}

echo
echo "========================================"
echo " DAIA Project Validation"
echo "========================================"
echo

echo "Checking directories..."
check_dir "$PROJECT_ROOT/runtime"
check_dir "$PROJECT_ROOT/runtime/config"
check_dir "$PROJECT_ROOT/runtime/lib"
check_dir "$PROJECT_ROOT/runtime/modules"
check_dir "$PROJECT_ROOT/runtime/systemd"

echo
echo "Checking required files..."
check_file "$PROJECT_ROOT/payload/config/pragna.conf"

check_file "$PROJECT_ROOT/runtime/runtime.sh"
check_file "$PROJECT_ROOT/runtime/firstboot.sh"

check_file "$PROJECT_ROOT/runtime/lib/logging.sh"
check_file "$PROJECT_ROOT/runtime/lib/common.sh"
check_file "$PROJECT_ROOT/runtime/lib/validation.sh"

check_file "$PROJECT_ROOT/runtime/systemd/daia-firstboot.service"

echo
echo "Checking build tools..."
check_cmd xorriso
check_cmd rsync

echo
echo "Checking Debian ISO..."

if [[ -f "$SOURCE_ISO" ]]; then
    ok "$SOURCE_ISO"
else
    error "Debian ISO not found"
fi

echo
echo "========================================"

if [[ $FAIL -eq 0 ]]; then
    echo "Validation successful."
    echo "$PASS checks passed."
    echo "========================================"
    exit 0
else
    echo "$FAIL checks failed."
    echo "========================================"
    exit 1
fi
