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
# mark_firstboot_complete
############################################################

mark_firstboot_complete()
{
    mkdir -p "$DAIA_STATE_DIRECTORY"
    : > "$DAIA_FIRSTBOOT_COMPLETE"
}

############################################################
# disable_firstboot_service
############################################################

disable_firstboot_service()
{
    systemctl disable "$DAIA_FIRSTBOOT_SERVICE"
}

############################################################
# main
############################################################

main()
{
    require_root
    run_runtime
    realize_models
    mark_firstboot_complete
    disable_firstboot_service
}

main "$@"

############################################################
# End of File
############################################################
