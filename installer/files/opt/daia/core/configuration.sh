#!/usr/bin/env bash
#
# DAIA Configuration Manager
#
# Loads an installation configuration and provides controlled access
# to its values.
#
# Public API:
#   daia_load_configuration <config-file>
#   daia_configuration_loaded
#   daia_get <variable-name>
#

# Prevent the module from being initialized more than once.
if [[ -n "${__DAIA_CONFIGURATION_MODULE_INITIALIZED:-}" ]]; then
# Allow this file to be either sourced or executed.
# shellcheck disable=SC2317
     return 0 2>/dev/null || exit 0
fi

__DAIA_CONFIGURATION_MODULE_INITIALIZED=1
__DAIA_CONFIGURATION_FILE=""
__DAIA_CONFIGURATION_LOADED=0


###############################################################################
# Internal helpers
###############################################################################

_daia_configuration_error() {
    printf 'DAIA configuration error: %s\n' "$*" >&2
}


_daia_valid_variable_name() {
    local variable_name="${1:-}"

    [[ "$variable_name" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]]
}


###############################################################################
# Public API
###############################################################################

daia_load_configuration() {
    local config_file="${1:-}"

    if [[ -z "$config_file" ]]; then
        _daia_configuration_error \
            "configuration file path was not provided"
        return 2
    fi

    if [[ ! -e "$config_file" ]]; then
        _daia_configuration_error \
            "configuration file does not exist: $config_file"
        return 1
    fi

    if [[ ! -f "$config_file" ]]; then
        _daia_configuration_error \
            "configuration path is not a regular file: $config_file"
        return 1
    fi

    if [[ ! -r "$config_file" ]]; then
        _daia_configuration_error \
            "configuration file is not readable: $config_file"
        return 1
    fi

    # Check Bash syntax before evaluating the configuration.
    if ! bash -n "$config_file"; then
        _daia_configuration_error \
            "configuration file contains invalid Bash syntax: $config_file"
        return 1
    fi

    # shellcheck disable=SC1090
    if ! source "$config_file"; then
        _daia_configuration_error \
            "failed to load configuration file: $config_file"
        return 1
    fi

    __DAIA_CONFIGURATION_FILE="$config_file"
    __DAIA_CONFIGURATION_LOADED=1

    return 0
}


daia_configuration_loaded() {
    [[ "$__DAIA_CONFIGURATION_LOADED" -eq 1 ]]
}


daia_get() {
    local variable_name="${1:-}"

    if ! daia_configuration_loaded; then
        _daia_configuration_error \
            "configuration has not been loaded"
        return 1
    fi

    if [[ -z "$variable_name" ]]; then
        _daia_configuration_error \
            "configuration variable name was not provided"
        return 2
    fi

    if ! _daia_valid_variable_name "$variable_name"; then
        _daia_configuration_error \
            "invalid configuration variable name: $variable_name"
        return 2
    fi

    # Missing variables produce an empty value in this initial version.
    if ! declare -p "$variable_name" &>/dev/null; then
        printf '\n'
        return 0
    fi

    printf '%s\n' "${!variable_name}"
}
