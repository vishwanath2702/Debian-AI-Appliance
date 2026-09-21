#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : runtime/runtime.sh
# Purpose    : Execute the DAIA runtime module lifecycle.
#
# Version    : 1.0.0
# Codename   : Pragna
# License    : GPL-3.0
#
# Runtime Lifecycle
# -----------------
# Each selected runtime module is processed in this order:
#
#   validate
#   install
#   verify
#
# Module loading, contract validation, and operation dispatch
# are provided by lib/lifecycle.sh.
#
# This file must be executed and must not be sourced.
# ==========================================================

set -euo pipefail

############################################################
# Runtime location
############################################################

RUNTIME_DIRECTORY="$(
    cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &&
    pwd
)"
readonly RUNTIME_DIRECTORY

readonly RUNTIME_CONFIG_FILE="${RUNTIME_DIRECTORY}/config/pragna.conf"
readonly RUNTIME_LIFECYCLE_LIBRARY="${RUNTIME_DIRECTORY}/lib/lifecycle.sh"

############################################################
# Load configuration
############################################################

if [[ ! -f "$RUNTIME_CONFIG_FILE" ]]
then
    printf 'ERROR: Runtime configuration file does not exist: %s\n' \
        "$RUNTIME_CONFIG_FILE" >&2
    exit 1
fi

# shellcheck source=/dev/null
source "$RUNTIME_CONFIG_FILE"

############################################################
# Load lifecycle library
############################################################

if [[ ! -r "$RUNTIME_LIFECYCLE_LIBRARY" ]]
then
    printf 'ERROR: Runtime lifecycle library is unavailable: %s\n' \
        "$RUNTIME_LIFECYCLE_LIBRARY" >&2
    exit 1
fi

# shellcheck source=/dev/null
source "$RUNTIME_LIFECYCLE_LIBRARY"

############################################################
# Runtime module selection
############################################################

readonly -a RUNTIME_MODULES=("ai")

############################################################
# runtime_module_execute
############################################################

runtime_module_execute()
{
    local module_name="${1:-}"
    local lifecycle_operation

    module_load "$module_name" || return 1

    for lifecycle_operation in validate install verify
    do
        module_operation_run \
            "$module_name" \
            "$lifecycle_operation" || return $?
    done

    return 0
}

############################################################
# main
############################################################

main()
{
    local module_name

    for module_name in "${RUNTIME_MODULES[@]}"
    do
        runtime_module_execute "$module_name" || return $?
    done

    return 0
}

main "$@"

############################################################
# End of File
############################################################
