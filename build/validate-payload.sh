#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

PAYLOAD_DIR="${1:-$PROJECT_ROOT/payload}"

passed=0
failed=0

ok() {
    printf '[ OK ] %s\n' "$1"
    passed=$((passed + 1))
}

fail() {
    printf '[FAIL] %s\n' "$1" >&2
    failed=$((failed + 1))
}

check_file() {
    local path="$1"

    if [[ -f "$path" ]]; then
        ok "$path"
    else
        fail "$path"
    fi
}

check_directory() {
    local path="$1"

    if [[ -d "$path" ]]; then
        ok "$path"
    else
        fail "$path"
    fi
}

echo "========================================"
echo " DAIA Payload Validation"
echo "========================================"
echo

check_directory "$PAYLOAD_DIR/packages/docker"
check_directory "$PAYLOAD_DIR/packages/ollama"
check_directory "$PAYLOAD_DIR/packages/dependencies"
check_directory "$PAYLOAD_DIR/images"

if [[ "${DAIA_ENABLE_OPEN_WEBUI:-false}" == "true" ]]
then
    check_file "$PAYLOAD_DIR/images/open-webui.tar"
fi

check_directory "$PAYLOAD_DIR/models/default"
check_directory "$PAYLOAD_DIR/branding/wallpapers"
check_directory "$PAYLOAD_DIR/branding/icons"

echo
echo "Checks passed: $passed"
echo "Checks failed: $failed"

if (( failed > 0 )); then
    echo
    echo "Payload validation failed."
    exit 1
fi

echo
echo "Payload structure is valid."
