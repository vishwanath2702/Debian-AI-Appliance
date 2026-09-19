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

ai_install() {
    local payload_root="${DAIA_PAYLOAD_ROOT:-/opt/daia/payload}"
    local install_root="${DAIA_OLLAMA_INSTALL_ROOT:-/usr/local}"

    ai_validate || return 1

    if [[ ! -d "$install_root" ]]; then
        echo "Ollama install root is not a directory: $install_root" >&2
        return 1
    fi

    local ollama_archive_name="ollama-linux-${DAIA_ARCHITECTURE}.tar.zst"
    local ollama_archive="${payload_root}/packages/ollama/${ollama_archive_name}"

    if ! tar \
        --use-compress-program=zstd \
        --extract \
        --file "$ollama_archive" \
        --directory "$install_root"
    then
        echo "Failed to install Ollama runtime archive: $ollama_archive" >&2
        return 1
    fi

    return 0
}

ai_verify() {
    local install_root="${DAIA_OLLAMA_INSTALL_ROOT:-/usr/local}"
    local ollama_binary="${install_root}/bin/ollama"
    local ollama_library_directory="${install_root}/lib/ollama"

    if [[ ! -x "$ollama_binary" ]]; then
        echo "Ollama runtime binary is missing or not executable: $ollama_binary" >&2
        return 1
    fi

    if [[ ! -d "$ollama_library_directory" ]]; then
        echo "Ollama runtime library directory is missing: $ollama_library_directory" >&2
        return 1
    fi

    return 0
}
