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
    local runtime_root="${DAIA_RUNTIME_ROOT:-/opt/daia}"
    local install_root="${DAIA_OLLAMA_INSTALL_ROOT:-/usr/local}"
    local sysusers_directory="${DAIA_SYSUSERS_DIRECTORY:-/etc/sysusers.d}"
    local systemd_system_directory="${DAIA_SYSTEMD_SYSTEM_DIRECTORY:-/etc/systemd/system}"
    local sysusers_source="${runtime_root}/sysusers.d/ollama.conf"
    local sysusers_destination="${sysusers_directory}/ollama.conf"
    local service_source="${runtime_root}/services/ollama.service"
    local service_destination="${systemd_system_directory}/ollama.service"
    local service_wants_directory="${systemd_system_directory}/multi-user.target.wants"
    local service_wants_link="${service_wants_directory}/ollama.service"

    ai_validate || return 1

    if ! command -v systemd-sysusers >/dev/null 2>&1; then
        echo "Required command is unavailable: systemd-sysusers" >&2
        return 1
    fi

    if ! command -v systemctl >/dev/null 2>&1; then
        echo "Required command is unavailable: systemctl" >&2
        return 1
    fi

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

    if [[ ! -s "$sysusers_source" ]]; then
        echo "Ollama system user definition is missing or empty: $sysusers_source" >&2
        return 1
    fi

    if ! mkdir -p "$sysusers_directory"; then
        echo "Failed to create system user definition directory: $sysusers_directory" >&2
        return 1
    fi

    if ! cp "$sysusers_source" "$sysusers_destination"; then
        echo "Failed to install Ollama system user definition: $sysusers_destination" >&2
        return 1
    fi

    if ! chmod 0644 "$sysusers_destination"; then
        echo "Failed to set Ollama system user definition permissions: $sysusers_destination" >&2
        return 1
    fi

    if ! systemd-sysusers "$sysusers_destination"; then
        echo "Failed to create Ollama system user from: $sysusers_destination" >&2
        return 1
    fi

    if [[ ! -s "$service_source" ]]; then
        echo "Ollama service definition is missing or empty: $service_source" >&2
        return 1
    fi

    if ! mkdir -p "$systemd_system_directory"; then
        echo "Failed to create systemd system directory: $systemd_system_directory" >&2
        return 1
    fi

    if ! cp "$service_source" "$service_destination"; then
        echo "Failed to install Ollama service definition: $service_destination" >&2
        return 1
    fi

    if ! chmod 0644 "$service_destination"; then
        echo "Failed to set Ollama service definition permissions: $service_destination" >&2
        return 1
    fi

    if ! mkdir -p "$service_wants_directory"; then
        echo "Failed to create Ollama service enablement directory: $service_wants_directory" >&2
        return 1
    fi

    if ! ln -sf /etc/systemd/system/ollama.service "$service_wants_link"; then
        echo "Failed to enable Ollama service: $service_wants_link" >&2
        return 1
    fi

    if ! systemctl daemon-reload; then
        echo "Failed to reload systemd after installing Ollama service" >&2
        return 1
    fi

    if ! systemctl start ollama.service; then
        echo "Failed to start Ollama service" >&2
        return 1
    fi

    return 0
}

_ai_verify_ollama_account() {
    local passwd_entry
    local group_entry
    local user_gid
    local group_gid

    if ! passwd_entry="$(getent passwd ollama)"; then
        echo "Ollama service user does not exist: ollama" >&2
        return 1
    fi

    if ! group_entry="$(getent group ollama)"; then
        echo "Ollama service group does not exist: ollama" >&2
        return 1
    fi

    user_gid="$(printf '%s\n' "$passwd_entry" | cut -d: -f4)"
    group_gid="$(printf '%s\n' "$group_entry" | cut -d: -f3)"

    if [[ -z "$user_gid" || "$user_gid" != "$group_gid" ]]; then
        echo "Ollama service user's primary group is not ollama" >&2
        return 1
    fi

    return 0
}

ai_verify() {
    local install_root="${DAIA_OLLAMA_INSTALL_ROOT:-/usr/local}"
    local systemd_system_directory="${DAIA_SYSTEMD_SYSTEM_DIRECTORY:-/etc/systemd/system}"
    local ollama_binary="${install_root}/bin/ollama"
    local ollama_library_directory="${install_root}/lib/ollama"
    local service_destination="${systemd_system_directory}/ollama.service"
    local service_wants_link="${systemd_system_directory}/multi-user.target.wants/ollama.service"

    if ! command -v getent >/dev/null 2>&1; then
        echo "Required command is unavailable: getent" >&2
        return 1
    fi

    if ! command -v cut >/dev/null 2>&1; then
        echo "Required command is unavailable: cut" >&2
        return 1
    fi

    if [[ ! -x "$ollama_binary" ]]; then
        echo "Ollama runtime binary is missing or not executable: $ollama_binary" >&2
        return 1
    fi

    if [[ ! -d "$ollama_library_directory" ]]; then
        echo "Ollama runtime library directory is missing: $ollama_library_directory" >&2
        return 1
    fi

    if [[ ! -s "$service_destination" ]]; then
        echo "Ollama service definition is missing or empty: $service_destination" >&2
        return 1
    fi

    if [[ ! -L "$service_wants_link" ]]; then
        echo "Ollama service enablement link is missing: $service_wants_link" >&2
        return 1
    fi

    if [[ "$(readlink "$service_wants_link")" != "/etc/systemd/system/ollama.service" ]]; then
        echo "Ollama service enablement link has an unexpected target: $service_wants_link" >&2
        return 1
    fi

    _ai_verify_ollama_account || return 1

    return 0
}
