#!/usr/bin/env bash

# DAIA local AI runtime module.
#
# This module is sourced by the DAIA runtime. It is not intended to be
# executed directly.

ai_validate() {
    local payload_root="${DAIA_PAYLOAD_ROOT:-/opt/daia/payload}"

    if [[ -z "${DAIA_ARCHITECTURE:-}" ]]; then
        echo "DAIA architecture is not configured: DAIA_ARCHITECTURE" >&2
        return 1
    fi

    local ollama_archive_name="ollama-linux-${DAIA_ARCHITECTURE}.tar.zst"
    local ollama_archive="${payload_root}/packages/ollama/${ollama_archive_name}"

    if [[ ! -s "$ollama_archive" ]]; then
        echo "Ollama runtime archive is missing or empty: $ollama_archive" >&2
        return 1
    fi

    if ! command -v tar >/dev/null 2>&1; then
        echo "Required command is unavailable: tar" >&2
        return 1
    fi

    if ! command -v zstd >/dev/null 2>&1; then
        echo "Required command is unavailable: zstd" >&2
        return 1
    fi

    return 0
}
