#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : runtime/lib/module.sh
# Purpose    : Provide the DAIA runtime module lifecycle
#              contract.
#
# Version    : 1.0.0
# Codename   : Pragna
# License    : GPL-3.0
#
# Public API
# ----------
# - module_contract_validate
#
# Module Contract
# ---------------
# A runtime module named <module> must expose:
#
#   <module>_validate
#   <module>_install
#   <module>_verify
#
# This file is intended to be sourced by DAIA runtime scripts.
# It must not be executed directly.
# ==========================================================

############################################################
# Prevent accidental direct execution
############################################################

if [[ "${BASH_SOURCE[0]}" == "$0" ]]
then
    printf 'ERROR: %s must be sourced, not executed.\n' \
        "${BASH_SOURCE[0]}" >&2

    exit 1
fi

############################################################
# module_contract_validate
#
# Verify that a named module exposes every required lifecycle
# function.
#
# Arguments:
#   $1 - Module name
#
# Returns:
#   0 when the module contract is satisfied.
#   1 otherwise.
############################################################

module_contract_validate()
{
    local module_name="${1:-}"
    local lifecycle_operation
    local function_name

    if [[ -z "$module_name" ]]
    then
        printf 'ERROR: module_contract_validate requires a module name.\n' >&2
        return 1
    fi

    for lifecycle_operation in validate install verify
    do
        function_name="${module_name}_${lifecycle_operation}"

        if ! declare -F "$function_name" >/dev/null 2>&1
        then
            printf 'ERROR: Module lifecycle function is unavailable: %s\n' \
                "$function_name" >&2
            return 1
        fi
    done

    return 0
}

############################################################
# End of File
############################################################
