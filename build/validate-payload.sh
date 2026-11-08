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

# Ollama payload must contain exactly one verified Linux AMD64 archive.
OLLAMA_DIRECTORY="$PAYLOAD_DIR/packages/ollama"
OLLAMA_ARCHIVE="$OLLAMA_DIRECTORY/ollama-linux-amd64.tar.zst"
OLLAMA_FETCH_SCRIPT="$SCRIPT_DIR/fetch-ollama.sh"

if [[ ! -f "$OLLAMA_FETCH_SCRIPT" ]]; then
    echo "Missing Ollama version manifest: $OLLAMA_FETCH_SCRIPT" >&2
    exit 1
fi

OLLAMA_EXPECTED_SHA256="$(
    sed -n 's/^OLLAMA_SHA256="\([0-9a-f]\{64\}\)"$/\1/p' \
        "$OLLAMA_FETCH_SCRIPT"
)"

if [[ ! "$OLLAMA_EXPECTED_SHA256" =~ ^[0-9a-f]{64}$ ]]; then
    echo "Invalid Ollama SHA256 configuration" >&2
    exit 1
fi

if [[ ! -f "$OLLAMA_ARCHIVE" || -L "$OLLAMA_ARCHIVE" ]]; then
    echo "Missing or unsafe Ollama archive: $OLLAMA_ARCHIVE" >&2
    exit 1
fi

mapfile -d '' OLLAMA_ARCHIVES < <(
    find "$OLLAMA_DIRECTORY" -maxdepth 1 \
        -name '*.tar.zst' -print0
)

if [[ "${#OLLAMA_ARCHIVES[@]}" -ne 1 ]]; then
    echo "Expected exactly one Ollama archive; found ${#OLLAMA_ARCHIVES[@]}" >&2
    exit 1
fi

if ! printf '%s  %s\n' \
    "$OLLAMA_EXPECTED_SHA256" "$OLLAMA_ARCHIVE" |
    sha256sum --check --status
then
    echo "Ollama archive checksum verification failed" >&2
    exit 1
fi

echo "PASS: Exactly one verified Ollama archive"
check_directory "$PAYLOAD_DIR/packages/dependencies"
check_directory "$PAYLOAD_DIR/images"

if [[ "${DAIA_ENABLE_OPEN_WEBUI:-false}" == "true" ]]
then
    check_file "$PAYLOAD_DIR/images/open-webui.tar"
fi

if [[ "${DAIA_ENABLE_PGVECTOR:-false}" == "true" ]]
then
    check_file "$PAYLOAD_DIR/images/pgvector.tar"
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
