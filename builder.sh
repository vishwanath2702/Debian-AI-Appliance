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
        clean)
            shift
            bash "${SCRIPT_DIRECTORY}/build/clean.sh" "$@"
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

    # Public laptop profile: use the existing guarded fresh-build pipeline.
    # The Rust capability remains "desktop" until declarative profiles
    # are integrated into the engine.
    if [[ "$capability" == "laptop" ]]; then
        if [[ -n "${DAIA_WORK_DIR:-}" || -n "${DAIA_OUTPUT_ISO:-}" ]]; then
            printf 'Error: laptop build selects its own workspace and output.\n' >&2
            return 1
        fi

        local stamp
        local workspace
        local output_iso

        stamp="$(date -u +%Y%m%d-%H%M%S)"
        workspace="${WORK_DIRECTORY}/daia-fresh-laptop-${stamp}"
        output_iso="${OUTPUT_DIRECTORY}/daia-laptop-${stamp}.iso"

        if [[ -e "$workspace" || -L "$workspace" ||
              -e "$output_iso" || -L "$output_iso" ]]; then
            printf 'Error: laptop build destination already exists.\n' >&2
            return 1
        fi

        umask 077

        local lock_file="${WORK_DIRECTORY}/.daia-build-cleanup.lock"
        mkdir -p -- "${WORK_DIRECTORY}"

        if [[ -L "$lock_file" ]]; then
            printf 'Error: unsafe build lock symlink.\n' >&2
            return 1
        fi

        (
            exec 9>>"$lock_file"
            flock -n -x 9 || {
                printf 'Error: another build or cleanup is active.\n' >&2
                exit 1
            }

            # Explicit opt-in for preserving earlier build workspaces.
            # The build lock remains held in either mode.
            case "${DAIA_SKIP_AUTO_CLEANUP:-0}" in
                0)
                    DAIA_CLEANUP_LOCK_FD=9 bash "${SCRIPT_DIRECTORY}/build/clean.sh" --auto-clean --execute --confirm DELETE --lock-held
                    ;;
                1)
                    printf 'DAIA_AUTO_CLEANUP=SKIPPED (explicit preservation)\\n'
                    ;;
                *)
                    printf 'Error: DAIA_SKIP_AUTO_CLEANUP must be 0 or 1.\\n' >&2
                    exit 1
                    ;;
            esac

            mkdir -m 700 -- "$workspace"
            printf 'running\n' > "$workspace/.daia-build-status"

            if env DAIA_WORK_DIR="$workspace" DAIA_OUTPUT_ISO="$output_iso" DAIA_BUILD_HANDOFF=1 DAIA_OLLAMA_OFFLINE_APPROVED="${DAIA_OLLAMA_OFFLINE_APPROVED:-false}" bash "${SCRIPT_DIRECTORY}/build/build.sh"; then
                if ! python3 - "$output_iso" <<'ISO_VERIFY_PY'
import hashlib
import os
import stat
import sys

path = sys.argv[1]
fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
try:
    info = os.fstat(fd)
    if not stat.S_ISREG(info.st_mode):
        raise RuntimeError("ISO output is not a regular file")
    if info.st_size < 32768:
        raise RuntimeError("ISO output is too small")
    os.lseek(fd, 16 * 2048, os.SEEK_SET)
    descriptor = os.read(fd, 2048)
    if len(descriptor) != 2048 or descriptor[:7] != bytes((1,)) + b"CD001" + bytes((1,)):
        raise RuntimeError("ISO9660 primary volume descriptor missing")
    os.lseek(fd, 0, os.SEEK_SET)
    digest = hashlib.sha256()
    while True:
        chunk = os.read(fd, 1024 * 1024)
        if not chunk:
            break
        digest.update(chunk)
    if os.fstat(fd).st_size != info.st_size:
        raise RuntimeError("ISO size changed during verification")
    print("ISO_VERIFIED_SHA256=" + digest.hexdigest())
    print("ISO_VERIFIED_BYTES=" + str(info.st_size))
finally:
    os.close(fd)
ISO_VERIFY_PY
                then
                    printf 'failed\n' > "$workspace/.daia-build-status"
                    printf 'Error: ISO verification failed; workspace preserved.\n' >&2
                    exit 1
                fi
                printf 'succeeded\n' > "$workspace/.daia-build-status"

                # The output ISO has passed the verification gate.
                # Remove disposable successful-build intermediates
                # while preserving the ISO in the output directory.
                if ! DAIA_CLEANUP_LOCK_FD=9 bash "${SCRIPT_DIRECTORY}/build/clean.sh" --auto-clean --execute --confirm DELETE --lock-held; then
                    printf 'Warning: post-build cleanup failed; verified ISO remains available.\n' >&2
                fi
            else
                result=$?
                printf 'failed\n' > "$workspace/.daia-build-status"
                exit "$result"
            fi
        )
        return
    fi

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
        "${output_iso}" \
        "${WORK_DIRECTORY}/payload/daia"
}

find_source_iso() {
    local -a source_isos=()

    mapfile -t source_isos < <(
        find "${ISO_DIRECTORY}" \
            -maxdepth 1 \
            -type f \
            -name 'debian-*-amd64-netinst.iso' \
            -print \
            | sort
    )

    if [[ "${#source_isos[@]}" -eq 0 ]]; then
        printf 'Error: no Debian amd64 netinst ISO found in: %s\n' \
            "${ISO_DIRECTORY}" >&2
        return 1
    fi

    if [[ "${#source_isos[@]}" -ne 1 ]]; then
        printf 'Error: multiple Debian amd64 netinst ISOs found in: %s\n' \
            "${ISO_DIRECTORY}" >&2
        printf 'Set DAIA_SOURCE_ISO explicitly to select one.\n' >&2
        printf '  %s\n' "${source_isos[@]}" >&2
        return 1
    fi

    printf '%s\n' "${source_isos[0]}"
}

run_cli() {
    cargo run \
        --quiet \
        --manifest-path "${ENGINE_DIRECTORY}/Cargo.toml" \
        --package cli \
        --bin daia \
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
