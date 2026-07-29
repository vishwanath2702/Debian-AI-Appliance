#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIRECTORY="$(
    cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
    pwd
)"
readonly SCRIPT_DIRECTORY

ENGINE_DIRECTORY="${SCRIPT_DIRECTORY}/engine"
readonly ENGINE_DIRECTORY
readonly ISO_DIRECTORY="${SCRIPT_DIRECTORY}/iso"
readonly WORK_DIRECTORY="${SCRIPT_DIRECTORY}/work"
readonly OUTPUT_DIRECTORY="${SCRIPT_DIRECTORY}/output"
readonly VERSION_FILE="${SCRIPT_DIRECTORY}/VERSION"
main() {
    require_command cargo

    if [[ ! -f "${ENGINE_DIRECTORY}/Cargo.toml" ]]; then
        printf 'Error: Rust workspace not found: %s\n' "${ENGINE_DIRECTORY}" >&2
        return 1
    fi

    case "${1:-}" in
        build)
            shift
            build_iso "$@"
            ;;
        *)
            run_cli "$@"
            ;;
    esac
}

build_iso() {
    if [[ "$#" -ne 1 ]]; then
        printf 'Usage: %s build <capability>\n' "${0##*/}" >&2
        return 1
    fi

    local capability="$1"
    local rootfs
    local source_iso
    local iso_work_directory
    local output_iso
    local version

    rootfs="${DAIA_ROOTFS:-${WORK_DIRECTORY}/rootfs}"
    source_iso="${DAIA_SOURCE_ISO:-}"
    iso_work_directory="${DAIA_ISO_WORK_DIRECTORY:-${WORK_DIRECTORY}/iso/${capability}}"

    if [[ -z "${source_iso}" ]]; then
        source_iso="$(find_source_iso)" || return
    fi

    if [[ -f "${VERSION_FILE}" ]]; then
        version="$(<"${VERSION_FILE}")"
    else
        version="dev"
    fi

    output_iso="${DAIA_OUTPUT_ISO:-${OUTPUT_DIRECTORY}/daia-${version}.iso}"

    if [[ ! -d "${rootfs}" ]]; then
        printf 'Error: root filesystem not found: %s\n' "${rootfs}" >&2
        printf 'Set DAIA_ROOTFS to the prepared root filesystem directory.\n' >&2
        return 1
    fi

    mkdir -p "${iso_work_directory}" "${OUTPUT_DIRECTORY}"

    printf 'Capability : %s\n' "${capability}"
    printf 'Rootfs     : %s\n' "${rootfs}"
    printf 'Source ISO : %s\n' "${source_iso}"
    printf 'Work area  : %s\n' "${iso_work_directory}"
    printf 'Output ISO : %s\n\n' "${output_iso}"

    run_cli \
        build-iso \
        "${capability}" \
        "${rootfs}" \
        "${source_iso}" \
        "${iso_work_directory}" \
        "${output_iso}"
}

find_source_iso() {
    local source_iso

    source_iso="$(
        find "${ISO_DIRECTORY}" \
            -maxdepth 1 \
            -type f \
            -name 'debian-*-amd64-netinst.iso' \
            -print \
            -quit
    )"

    if [[ -z "${source_iso}" ]]; then
        printf 'Error: no Debian amd64 netinst ISO found in: %s\n' \
            "${ISO_DIRECTORY}" >&2
        return 1
    fi

    printf '%s\n' "${source_iso}"
}

run_cli() {
    cargo run \
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
