#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : runtime/firstboot.sh
# Purpose    : Execute one-time DAIA first-boot provisioning.
#
# Version    : 1.0.0
# Codename   : Pragna
# License    : GPL-3.0
#
# First-Boot Lifecycle
# --------------------
# 1. Require root privileges.
# 2. Execute the canonical DAIA runtime orchestrator.
# 3. Record successful first-boot completion.
# 4. Disable the first-boot service.
#
# The completion marker is written only after the runtime
# lifecycle completes successfully.
# ==========================================================

set -euo pipefail

############################################################
# Paths
############################################################

readonly DAIA_RUNTIME="/opt/daia/runtime.sh"
readonly DAIA_COMMAND="/usr/bin/daia"
readonly DAIA_STATE_DIRECTORY="/var/lib/daia"
readonly DAIA_FIRSTBOOT_COMPLETE="${DAIA_STATE_DIRECTORY}/firstboot-complete"
readonly DAIA_FIRSTBOOT_SERVICE="daia-firstboot.service"

############################################################
# require_root
############################################################

require_root()
{
    if (( EUID != 0 ))
    then
        printf 'ERROR: DAIA first-boot provisioning requires root privileges.\n' >&2
        return 1
    fi
}

############################################################
# run_runtime
############################################################

run_runtime()
{
    if [[ ! -x "$DAIA_RUNTIME" ]]
    then
        printf 'ERROR: DAIA runtime is unavailable: %s\n' \
            "$DAIA_RUNTIME" >&2
        return 1
    fi

    "$DAIA_RUNTIME"
}

############################################################
# realize_models
############################################################

realize_models()
{
    if [[ ! -x "$DAIA_COMMAND" ]]
    then
        printf 'ERROR: DAIA command is unavailable: %s\n' \
            "$DAIA_COMMAND" >&2
        return 1
    fi

    "$DAIA_COMMAND" realize-models
}

############################################################
# install_offline_embedding_model
############################################################

install_offline_embedding_model()
{
    local config="/opt/daia/config/pragna.conf"
    local helper="/usr/libexec/daia-install-embedding-model.py"

    if [[ ! -r "$config" ]]; then
        printf "ERROR: Missing Pragna configuration: %s\n" "$config" >&2
        return 1
    fi

    # shellcheck source=/dev/null
    source "$config"

    if [[ "${DAIA_ENABLE_EMBEDDING_MODEL:-false}" != "true" ]]; then
        return 0
    fi

    if [[ ! -x "$helper" ]]; then
        printf "ERROR: Missing embedding installer: %s\n" "$helper" >&2
        return 1
    fi

    "$helper"
}

############################################################
# mark_firstboot_complete
############################################################

prepare_model_directory()
{
    local state_directory="$1"

    install -d -o root -g root -m 0755 "$state_directory"
    install -d -o root -g root -m 0755 "$state_directory/content"
}

mark_firstboot_complete()
{
    : > "$DAIA_FIRSTBOOT_COMPLETE"
}

############################################################
# disable_firstboot_service
############################################################

disable_firstboot_service()
{
    systemctl disable "$DAIA_FIRSTBOOT_SERVICE"
}

enable_rag_recovery_service()
{
    systemctl daemon-reload || return 1
    systemctl enable daia-rag-recovery.service || return 1
}

# Queue startup without waiting for first-boot completion.
# Systemd enforces recovery and completion-marker prerequisites.
queue_rag_proxy_start()
{
    systemctl start --no-block \
        daia-ollama-bridge.service \
        daia-openwebui-localhost.service
}

############################################################
# main
############################################################

main()
{
    require_root
    prepare_model_directory "$DAIA_STATE_DIRECTORY"
    run_runtime
    realize_models
    install_offline_embedding_model
    enable_rag_recovery_service
    mark_firstboot_complete
    queue_rag_proxy_start
    disable_firstboot_service
}

main "$@"

############################################################
# End of File
############################################################
