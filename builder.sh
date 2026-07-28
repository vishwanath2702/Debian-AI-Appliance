#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIRECTORY="$(
    cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
    pwd
)"
readonly SCRIPT_DIRECTORY

ENGINE_DIRECTORY="${SCRIPT_DIRECTORY}/engine"
readonly ENGINE_DIRECTORY
main() {
    require_command cargo

    if [[ ! -f "${ENGINE_DIRECTORY}/Cargo.toml" ]]; then
        printf 'Error: Rust workspace not found: %s\n' "${ENGINE_DIRECTORY}" >&2
        return 1
    fi

    exec cargo run \
        --quiet \
        --manifest-path "${ENGINE_DIRECTORY}/Cargo.toml" \
        --package cli \
        -- "$@"
}

require_command() {
    local command_name="$1"

    if ! command -v "${command_name}" >/dev/null 2>&1; then
        printf 'Error: required command not found: %s\n' "${command_name}" >&2
        return 1
    fi
}

main "$@"
