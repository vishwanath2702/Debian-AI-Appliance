#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

OLLAMA_VERSION="v0.34.4"
OLLAMA_ARCHITECTURE="amd64"
OLLAMA_ARCHIVE_NAME="ollama-linux-${OLLAMA_ARCHITECTURE}.tar.zst"
OLLAMA_URL_SCHEME="https"
OLLAMA_URL_HOST="github.com"
OLLAMA_DOWNLOAD_URL="${OLLAMA_URL_SCHEME}://${OLLAMA_URL_HOST}/ollama/ollama/releases/download/${OLLAMA_VERSION}/${OLLAMA_ARCHIVE_NAME}"
OLLAMA_SHA256="c238986e61d40c0cc5f4a9b9e40b9eea104350b77efa34741fc134e105cb9533"

OLLAMA_PAYLOAD_DIR="$PROJECT_ROOT/payload/packages/ollama"
OLLAMA_ARCHIVE_PATH="$OLLAMA_PAYLOAD_DIR/$OLLAMA_ARCHIVE_NAME"
OLLAMA_TEMP_PATH="${OLLAMA_ARCHIVE_PATH}.part"

mkdir -p "$OLLAMA_PAYLOAD_DIR"

if [[ -s "$OLLAMA_ARCHIVE_PATH" ]]; then
    if printf '%s  %s\n' "$OLLAMA_SHA256" "$OLLAMA_ARCHIVE_PATH" |
        sha256sum --check --status
    then
        echo "Ollama payload already present and verified:"
        echo "  $OLLAMA_ARCHIVE_PATH"
        exit 0
    fi

    echo "Existing Ollama payload has an unexpected checksum:" >&2
    echo "  $OLLAMA_ARCHIVE_PATH" >&2
    exit 1
fi

rm -f "$OLLAMA_TEMP_PATH"

echo "Downloading Ollama ${OLLAMA_VERSION}:"
echo "  $OLLAMA_DOWNLOAD_URL"

curl \
    --fail \
    --location \
    --show-error \
    --output "$OLLAMA_TEMP_PATH" \
    "$OLLAMA_DOWNLOAD_URL"

if ! printf '%s  %s\n' "$OLLAMA_SHA256" "$OLLAMA_TEMP_PATH" |
    sha256sum --check --status
then
    echo "Ollama payload checksum verification failed" >&2
    rm -f "$OLLAMA_TEMP_PATH"
    exit 1
fi

mv "$OLLAMA_TEMP_PATH" "$OLLAMA_ARCHIVE_PATH"

echo "Ollama payload downloaded and verified:"
echo "  $OLLAMA_ARCHIVE_PATH"
