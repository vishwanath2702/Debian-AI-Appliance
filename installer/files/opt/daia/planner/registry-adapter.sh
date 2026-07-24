#!/usr/bin/env bash
#
# DAIA Planner Registry Adapter
#
# Exposes planner-oriented views of validated Plugin Registry and Capability
# Registry data without allowing planner modules to depend on registry storage
# internals.
#
# The adapter contains no provider-selection or dependency-resolution policy.
#
# Public API:
#   daia_registry_adapter_plugin_ids
#   daia_registry_adapter_plugin_exists <plugin-id>
#   daia_registry_adapter_plugin_capabilities <plugin-id>
#   daia_registry_adapter_capability_exists <capability>
#   daia_registry_adapter_capability_providers <capability>
#   daia_registry_adapter_capability_provider_count <capability>
#

if [[ -n "${__DAIA_REGISTRY_ADAPTER_MODULE_INITIALIZED:-}" ]]; then
    if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then
        return 0
    fi

    exit 0
fi

__DAIA_REGISTRY_ADAPTER_MODULE_INITIALIZED=1

###############################################################################
# Internal helpers
###############################################################################

_daia_registry_adapter_error() {
    printf 'DAIA registry-adapter error: %s\n' "$*" >&2
}

_daia_registry_adapter_validate_argument_count() {
    local expected="$1"
    local function_name="$2"
    local actual="$3"

    if [[ "$actual" -ne "$expected" ]]; then
        _daia_registry_adapter_error \
            "$function_name expects $expected argument(s); received $actual"
        return 1
    fi
}

_daia_registry_adapter_require_api() {
    local api_name="$1"

    if ! declare -F "$api_name" >/dev/null; then
        _daia_registry_adapter_error \
            "required registry API is unavailable: $api_name"
        return 1
    fi
}

_daia_registry_adapter_require_plugin_registry_listing_api() {
    _daia_registry_adapter_require_api \
        daia_registry_plugin_ids
}

_daia_registry_adapter_require_plugin_registry_exists_api() {
    _daia_registry_adapter_require_api \
        daia_registry_plugin_exists
}

_daia_registry_adapter_require_plugin_registry_metadata_api() {
    _daia_registry_adapter_require_plugin_registry_exists_api || return 1
    _daia_registry_adapter_require_api \
        daia_registry_plugin_metadata
}

_daia_registry_adapter_require_capability_exists_api() {
    _daia_registry_adapter_require_api \
        daia_capability_exists
}

_daia_registry_adapter_require_capability_providers_api() {
    _daia_registry_adapter_require_api \
        daia_capability_get_providers
}

_daia_registry_adapter_require_capability_provider_count_api() {
    _daia_registry_adapter_require_api \
        daia_capability_provider_count
}

_daia_registry_adapter_validate_plugin_id() {
    local plugin_id="${1-}"

    if [[ -z "$plugin_id" ]]; then
        _daia_registry_adapter_error "plugin ID must not be empty"
        return 1
    fi

    if [[ ! "$plugin_id" =~ ^[a-z0-9_-]+/[a-z0-9._-]+$ ]]; then
        _daia_registry_adapter_error "invalid plugin ID: $plugin_id"
        return 1
    fi
}

_daia_registry_adapter_validate_capability() {
    local capability="${1-}"

    if [[ -z "$capability" ]]; then
        _daia_registry_adapter_error "capability must not be empty"
        return 1
    fi

    if [[ ! "$capability" =~ ^[a-z0-9]+([._-][a-z0-9]+)*$ ]]; then
        _daia_registry_adapter_error "invalid capability: $capability"
        return 1
    fi
}

_daia_registry_adapter_validate_registry_capability() {
    local capability="${1-}"

    if [[ -z "$capability" ]]; then
        _daia_registry_adapter_error \
            "Plugin Registry returned an empty provided capability"
        return 1
    fi

    if [[ ! "$capability" =~ ^[a-z0-9]+([._-][a-z0-9]+)*$ ]]; then
        _daia_registry_adapter_error \
            "Plugin Registry returned an invalid provided capability: $capability"
        return 1
    fi
}

###############################################################################
# Plugin Registry views
###############################################################################

daia_registry_adapter_plugin_ids() {
    _daia_registry_adapter_validate_argument_count \
        0 "daia_registry_adapter_plugin_ids" "$#" || return 1
    _daia_registry_adapter_require_plugin_registry_listing_api || return 1

    daia_registry_plugin_ids
}

daia_registry_adapter_plugin_exists() {
    local plugin_id="${1-}"

    _daia_registry_adapter_validate_argument_count \
        1 "daia_registry_adapter_plugin_exists" "$#" || return 1
    _daia_registry_adapter_validate_plugin_id "$plugin_id" || return 1
    _daia_registry_adapter_require_plugin_registry_exists_api || return 1

    daia_registry_plugin_exists "$plugin_id"
}

daia_registry_adapter_plugin_capabilities() {
    local plugin_id="${1-}"
    local capability

    _daia_registry_adapter_validate_argument_count \
        1 "daia_registry_adapter_plugin_capabilities" "$#" || return 1
    _daia_registry_adapter_validate_plugin_id "$plugin_id" || return 1
    _daia_registry_adapter_require_plugin_registry_metadata_api || return 1

    if ! daia_registry_plugin_exists "$plugin_id"; then
        _daia_registry_adapter_error \
            "plugin is not registered: $plugin_id"
        return 1
    fi

    capability="$(
        daia_registry_plugin_metadata "$plugin_id" provides
    )" || {
        _daia_registry_adapter_error \
            "could not read provided capability for plugin: $plugin_id"
        return 1
    }

    _daia_registry_adapter_validate_registry_capability \
        "$capability" || return 1

    printf '%s\n' "$capability"
}

###############################################################################
# Capability Registry views
###############################################################################

daia_registry_adapter_capability_exists() {
    local capability="${1-}"

    _daia_registry_adapter_validate_argument_count \
        1 "daia_registry_adapter_capability_exists" "$#" || return 1
    _daia_registry_adapter_validate_capability "$capability" || return 1
    _daia_registry_adapter_require_capability_exists_api || return 1

    daia_capability_exists "$capability"
}

daia_registry_adapter_capability_providers() {
    local capability="${1-}"

    _daia_registry_adapter_validate_argument_count \
        1 "daia_registry_adapter_capability_providers" "$#" || return 1
    _daia_registry_adapter_validate_capability "$capability" || return 1
    _daia_registry_adapter_require_capability_providers_api || return 1

    daia_capability_get_providers "$capability"
}

daia_registry_adapter_capability_provider_count() {
    local capability="${1-}"

    _daia_registry_adapter_validate_argument_count \
        1 "daia_registry_adapter_capability_provider_count" "$#" || return 1
    _daia_registry_adapter_validate_capability "$capability" || return 1
    _daia_registry_adapter_require_capability_provider_count_api || return 1

    daia_capability_provider_count "$capability"
}
###############################################################################
# Plugin dependency access
###############################################################################

daia_registry_adapter_plugin_dependencies() {
    _daia_registry_adapter_validate_argument_count \
        1 \
        "daia_registry_adapter_plugin_dependencies" \
        "$#" || return 1

    local plugin_id="$1"

    _daia_registry_adapter_validate_plugin_id "$plugin_id" || return 1

    _daia_registry_adapter_require_api \
        "daia_plugin_registry_plugin_dependencies" || return 1

    daia_plugin_registry_plugin_dependencies "$plugin_id"
}
###############################################################################
# Plugin conflict access
###############################################################################

daia_registry_adapter_plugin_conflicts() {
    _daia_registry_adapter_validate_argument_count \
        1 \
        "daia_registry_adapter_plugin_conflicts" \
        "$#" || return 1

    local plugin_id="$1"

    _daia_registry_adapter_validate_plugin_id \
        "$plugin_id" || return 1

    _daia_registry_adapter_require_api \
        "daia_plugin_registry_plugin_conflicts" || return 1

    daia_plugin_registry_plugin_conflicts "$plugin_id"
}
