#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : runtime/lib/lifecycle.sh
# Purpose    : Provide the DAIA runtime module lifecycle
#              contract.
#
# Version    : 1.0.0
# Codename   : Pragna
# License    : GPL-3.0
#
# Public API
# ----------
# - module_path
# - module_contract_validate
# - module_load
# - module_operation_run
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
# module_path
#
# Return the path to a named runtime module.
#
# The module directory is derived from this library's own
# location so the same layout works in the source tree and in
# the installed /opt/daia runtime.
#
# Arguments:
#   $1 - Module name
#
# Returns:
#   The module path on standard output.
#   1 when the module name is empty.
############################################################

module_path()
{
    local module_name="${1:-}"
    local library_directory
    local runtime_directory

    if [[ -z "$module_name" ]]
    then
        printf 'ERROR: module_path requires a module name.\n' >&2
        return 1
    fi

    if [[ ! "$module_name" =~ ^[a-z][a-z0-9_-]*$ ]]
    then
        printf 'ERROR: Invalid module name: %s\n' "$module_name" >&2
        return 1
    fi

    library_directory="$(
        cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &&
        pwd
    )" || return 1

    runtime_directory="$(
        cd -- "${library_directory}/.." &&
        pwd
    )" || return 1

    printf '%s/modules/%s.sh\n' \
        "$runtime_directory" \
        "$module_name"
}

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
# module_load
#
# Load a named runtime module and validate its lifecycle
# contract.
#
# Arguments:
#   $1 - Module name
#
# Returns:
#   0 when the module was loaded and satisfies the contract.
#   1 otherwise.
############################################################

module_load()
{
    local module_name="${1:-}"
    local module_file

    module_file="$(module_path "$module_name")" || return 1

    if [[ ! -f "$module_file" ]]
    then
        printf 'ERROR: Runtime module is unavailable: %s\n' \
            "$module_file" >&2
        return 1
    fi

    if ! source "$module_file"
    then
        printf 'ERROR: Failed to load runtime module: %s\n' \
            "$module_file" >&2
        return 1
    fi

    module_contract_validate "$module_name"
}

############################################################
# module_operation_run
#
# Run one lifecycle operation for a named module.
#
# Arguments:
#   $1 - Module name
#   $2 - Lifecycle operation: validate, install, or verify
#
# Returns:
#   The lifecycle function's return status.
#   1 when the module contract or operation is invalid.
############################################################

module_operation_run()
{
    local module_name="${1:-}"
    local lifecycle_operation="${2:-}"
    local function_name

    module_contract_validate "$module_name" || return 1

    case "$lifecycle_operation" in
        validate|install|verify)
            ;;
        *)
            printf 'ERROR: Unsupported module lifecycle operation: %s\n' \
                "$lifecycle_operation" >&2
            return 1
            ;;
    esac

    function_name="${module_name}_${lifecycle_operation}"

    "$function_name"
}

############################################################
# End of File
############################################################
