#!/usr/bin/env bash
#
# DAIA Profile Reader
#
# Provides a stable interface for reading the capabilities requested by an
# installation profile. Profile storage and parsing remain behind the Profile
# Registry API.
#
# Public API:
#   daia_profile_reader_read <profile-id>
#

if [[ -n "${__DAIA_PROFILE_READER_MODULE_INITIALIZED:-}" ]]; then
    if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then
        return 0
    fi

    exit 0
fi

__DAIA_PROFILE_READER_MODULE_INITIALIZED=1

###############################################################################
# Internal helpers
###############################################################################

_daia_profile_reader_error() {
    printf 'DAIA profile reader error: %s\n' "$*" >&2
}

_daia_profile_reader_validate_argument_count() {
    local expected="$1"
    local function_name="$2"
    local actual="$3"

    if [[ "$actual" -ne "$expected" ]]; then
        _daia_profile_reader_error \
            "$function_name expects $expected argument(s); received $actual"
        return 1
    fi
}

_daia_profile_reader_validate_profile_id() {
    local profile_id="${1-}"

    if [[ -z "$profile_id" ]]; then
        _daia_profile_reader_error "profile ID must not be empty"
        return 1
    fi

    if [[ ! "$profile_id" =~ ^[a-z0-9][a-z0-9._-]*$ ]]; then
        _daia_profile_reader_error "invalid profile ID: $profile_id"
        return 1
    fi
}

_daia_profile_reader_require_api() {
    local function_name
    local -a required_functions=(
        daia_profile_registry_exists
        daia_profile_registry_capabilities
    )

    for function_name in "${required_functions[@]}"; do
        if ! declare -F "$function_name" >/dev/null; then
            _daia_profile_reader_error \
                "required Profile Registry API is unavailable: $function_name"
            return 1
        fi
    done
}

###############################################################################
# Public API
###############################################################################

daia_profile_reader_read() {
    local profile_id="${1-}"

    _daia_profile_reader_validate_argument_count \
        1 "daia_profile_reader_read" "$#" || return 1
    _daia_profile_reader_validate_profile_id "$profile_id" || return 1
    _daia_profile_reader_require_api || return 1

    if ! daia_profile_registry_exists "$profile_id"; then
        _daia_profile_reader_error "unknown profile: $profile_id"
        return 1
    fi

    daia_profile_registry_capabilities "$profile_id"
}
