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

    # Provision the dedicated Model Exporter worker identity.
    # This does not authorize export requests or start a worker.
    local export_sysusers_source="${runtime_root}/sysusers.d/daia-export.conf"
    local export_sysusers_destination="${sysusers_directory}/daia-export.conf"

    if [[ ! -s "$export_sysusers_source" ]]; then
        echo "Exporter system user definition is missing or empty: $export_sysusers_source" >&2
        return 1
    fi

    if ! cp "$export_sysusers_source" "$export_sysusers_destination"; then
        echo "Failed to install exporter system user definition: $export_sysusers_destination" >&2
        return 1
    fi

    if ! chmod 0644 "$export_sysusers_destination"; then
        echo "Failed to set exporter system user definition permissions: $export_sysusers_destination" >&2
        return 1
    fi

    if ! systemd-sysusers "$export_sysusers_destination"; then
        echo "Failed to create exporter system user from: $export_sysusers_destination" >&2
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

    # Install Model Exporter authorization and worker definitions.
    # The worker and socket remain disabled until authorization is complete.
    local export_policy_source="${runtime_root}/polkit-1/actions/org.daia.model-export.policy"
    local export_policy_directory="${DAIA_POLKIT_ACTIONS_DIRECTORY:-/usr/share/polkit-1/actions}"
    local export_policy_destination="${export_policy_directory}/org.daia.model-export.policy"
    local export_worker_source="${runtime_root}/services/daia-model-export-worker.service"
    local export_worker_destination="${systemd_system_directory}/daia-model-export-worker.service"
    local export_socket_source="${runtime_root}/services/daia-model-export.socket"
    local export_socket_destination="${systemd_system_directory}/daia-model-export.socket"

    if [[ "$EUID" -ne 0 ]]; then
        echo "Exporter configuration installation requires root" >&2
        return 1
    fi

    if ! mkdir -p "$export_policy_directory"; then
        echo "Failed to create exporter polkit actions directory" >&2
        return 1
    fi

    local export_source
    local export_destination

    for export_source in \
        "$export_policy_source" \
        "$export_worker_source" \
        "$export_socket_source"
    do
        if [[ ! -f "$export_source" || -L "$export_source" ]]; then
            echo "Exporter configuration source is missing or unsafe: $export_source" >&2
            return 1
        fi
    done

    for export_destination in \
        "$export_policy_destination" \
        "$export_worker_destination" \
        "$export_socket_destination"
    do
        if [[ -L "$export_destination" ]]; then
            echo "Exporter configuration destination is a symlink: $export_destination" >&2
            return 1
        fi
    done

    if ! install -o root -g root -m 0644 \
        "$export_policy_source" "$export_policy_destination"; then
        echo "Failed to install exporter polkit policy" >&2
        return 1
    fi

    if ! install -o root -g root -m 0644 \
        "$export_worker_source" "$export_worker_destination"; then
        echo "Failed to install exporter worker service" >&2
        return 1
    fi

    if ! install -o root -g root -m 0644 \
        "$export_socket_source" "$export_socket_destination"; then
        echo "Failed to install exporter socket unit" >&2
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

_ai_wait_for_ollama_ready() {
    local ollama_binary="$1"
    local attempts="${DAIA_OLLAMA_READY_ATTEMPTS:-30}"
    local delay="${DAIA_OLLAMA_READY_DELAY:-1}"
    local attempt
    local last_error=""

    for ((attempt = 1; attempt <= attempts; attempt++)); do
        if last_error="$(HOME=/var/lib/ollama "$ollama_binary" list 2>&1)"; then
            return 0
        fi

        if (( attempt < attempts )); then
            sleep "$delay"
        fi
    done

    echo "Ollama service did not become ready after ${attempts} attempts" >&2
    if [[ -n "$last_error" ]]; then
        printf 'Last Ollama readiness error: %s\n' "$last_error" >&2
    fi
    return 1
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

    if ! command -v systemctl >/dev/null 2>&1; then
        echo "Required command is unavailable: systemctl" >&2
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
    # The exporter worker must have its own nonroot identity.
    local export_account
    local ollama_account
    local export_uid
    local ollama_uid

    if ! export_account="$(getent passwd daia-export)"; then
        echo "Dedicated exporter account is missing: daia-export" >&2
        return 1
    fi

    if ! ollama_account="$(getent passwd ollama)"; then
        echo "Ollama account is missing while verifying exporter isolation" >&2
        return 1
    fi

    export_uid="$(printf '%s\n' "$export_account" | cut -d: -f3)"
    ollama_uid="$(printf '%s\n' "$ollama_account" | cut -d: -f3)"

    if [[ ! "$export_uid" =~ ^[0-9]+$ ]] ||
       [[ ! "$ollama_uid" =~ ^[0-9]+$ ]] ||
       [[ "$export_uid" == "0" ]] ||
       [[ "$export_uid" == "$ollama_uid" ]]
    then
        echo "Exporter worker UID is invalid or not isolated from Ollama" >&2
        return 1
    fi


    # Verify installed exporter configuration without activating its units.
    local export_runtime_root="${DAIA_RUNTIME_ROOT:-/opt/daia}"
    local export_policy_directory="${DAIA_POLKIT_ACTIONS_DIRECTORY:-/usr/share/polkit-1/actions}"
    local export_policy_source="${export_runtime_root}/polkit-1/actions/org.daia.model-export.policy"
    local export_policy_destination="${export_policy_directory}/org.daia.model-export.policy"
    local export_worker_source="${export_runtime_root}/services/daia-model-export-worker.service"
    local export_worker_destination="${systemd_system_directory}/daia-model-export-worker.service"
    local export_socket_source="${export_runtime_root}/services/daia-model-export.socket"
    local export_socket_destination="${systemd_system_directory}/daia-model-export.socket"
    local export_source
    local export_destination
    local export_metadata

    for export_source in \
        "$export_policy_source" \
        "$export_worker_source" \
        "$export_socket_source"
    do
        if [[ ! -f "$export_source" || -L "$export_source" ]]; then
            echo "Exporter verification source missing or unsafe: $export_source" >&2
            return 1
        fi
    done

    for export_destination in \
        "$export_policy_destination" \
        "$export_worker_destination" \
        "$export_socket_destination"
    do
        if [[ ! -f "$export_destination" || -L "$export_destination" ]]; then
            echo "Exporter installed configuration missing or unsafe: $export_destination" >&2
            return 1
        fi

        if ! export_metadata="$(stat -c '%u:%g:%a' -- "$export_destination")"; then
            echo "Cannot inspect exporter configuration: $export_destination" >&2
            return 1
        fi

        if [[ "$export_metadata" != "0:0:644" ]]; then
            echo "Exporter configuration ownership or mode invalid: $export_destination" >&2
            return 1
        fi
    done

    if ! cmp -s -- "$export_policy_source" "$export_policy_destination" ||
       ! cmp -s -- "$export_worker_source" "$export_worker_destination" ||
       ! cmp -s -- "$export_socket_source" "$export_socket_destination"; then
        echo "Installed exporter configuration differs from packaged source" >&2
        return 1
    fi

    local export_unit
    local export_unit_file_state
    local export_active_state

    for export_unit in \
        daia-model-export-worker.service \
        daia-model-export.socket
    do
        if ! export_unit_file_state="$(systemctl show "$export_unit" --property=UnitFileState --value)"; then
            echo "Cannot inspect exporter unit-file state: $export_unit" >&2
            return 1
        fi

        case "$export_unit_file_state" in
            disabled|static)
                ;;
            *)
                echo "Exporter unit-file state is unsafe: $export_unit ($export_unit_file_state)" >&2
                return 1
                ;;
        esac

        if ! export_active_state="$(systemctl show "$export_unit" --property=ActiveState --value)"; then
            echo "Cannot inspect exporter active state: $export_unit" >&2
            return 1
        fi

        if [[ "$export_active_state" != "inactive" ]]; then
            echo "Exporter unit is not inactive: $export_unit ($export_active_state)" >&2
            return 1
        fi
    done

    local service_environment
    if ! service_environment="$(systemctl show ollama.service --property=Environment --value)"; then
        echo "Failed to inspect Ollama service environment" >&2
        return 1
    fi

    if [[ " ${service_environment} " != *" OLLAMA_CONTEXT_LENGTH=8192 "* ]]; then
        echo "Ollama service context length is not configured to 8192" >&2
        return 1
    fi

    if ! systemctl is-active --quiet ollama.service; then
        echo "Ollama service is not active: ollama.service" >&2
        return 1
    fi

    _ai_wait_for_ollama_ready "$ollama_binary" || return 1

    return 0
}
