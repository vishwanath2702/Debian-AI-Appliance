#!/usr/bin/env bash
#
# DAIA Planner Capability Resolver
#
# Resolves requested capabilities into provider plugin IDs through the public
# Registry Adapter API.
#
# Provider-selection policy:
#
#   - an unknown capability is rejected
#   - a capability with no providers is rejected
#   - the first provider returned by the Registry Adapter is selected
#   - selected provider IDs are validated
#   - resolve_many emits duplicate providers only once
#
# Public API:
#   daia_capability_resolver_resolve <capability>
#   daia_capability_resolver_resolve_many <capability>...
#

if [[ -n "${__DAIA_CAPABILITY_RESOLVER_MODULE_INITIALIZED:-}" ]]; then
    if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then
        return 0
    fi

    exit 0
fi

__DAIA_CAPABILITY_RESOLVER_MODULE_INITIALIZED=1

###############################################################################
# Internal helpers
###############################################################################

_daia_capability_resolver_error() {
    printf 'DAIA capability resolver error: %s\n' "$*" >&2
}

_daia_capability_resolver_validate_argument_count() {
    local expected="$1"
    local function_name="$2"
    local actual="$3"

    if [[ "$actual" -ne "$expected" ]]; then
        _daia_capability_resolver_error \
            "$function_name expects $expected argument(s); received $actual"
        return 1
    fi
}

_daia_capability_resolver_validate_minimum_argument_count() {
    local minimum="$1"
    local function_name="$2"
    local actual="$3"

    if [[ "$actual" -lt "$minimum" ]]; then
        _daia_capability_resolver_error \
            "$function_name expects at least $minimum argument(s); received $actual"
        return 1
    fi
}

_daia_capability_resolver_validate_capability() {
    local capability="${1-}"

    if [[ -z "$capability" ]]; then
        _daia_capability_resolver_error \
            "capability must not be empty"
        return 1
    fi

    if [[ ! "$capability" =~ ^[a-z0-9]+([._-][a-z0-9]+)*$ ]]; then
        _daia_capability_resolver_error \
            "invalid capability: $capability"
        return 1
    fi
}

_daia_capability_resolver_validate_plugin_id() {
    local plugin_id="${1-}"

    if [[ -z "$plugin_id" ]]; then
        _daia_capability_resolver_error \
            "provider plugin ID must not be empty"
        return 1
    fi

    if [[ ! "$plugin_id" =~ ^[a-z0-9]+([._-][a-z0-9]+)*/[a-z0-9]+([._-][a-z0-9]+)*$ ]]; then
        _daia_capability_resolver_error \
            "invalid provider plugin ID: $plugin_id"
        return 1
    fi
}

_daia_capability_resolver_validate_provider_count() {
    local provider_count="${1-}"
    local capability="$2"

    if [[ ! "$provider_count" =~ ^[0-9]+$ ]]; then
        _daia_capability_resolver_error \
            "invalid provider count for capability $capability: $provider_count"
        return 1
    fi

    if [[ "$provider_count" -eq 0 ]]; then
        _daia_capability_resolver_error \
            "no provider is available for capability: $capability"
        return 1
    fi
}

_daia_capability_resolver_require_api() {
    local function_name
    local -a required_functions=(
        daia_registry_adapter_capability_exists
        daia_registry_adapter_capability_provider_count
        daia_registry_adapter_capability_providers
    )

    for function_name in "${required_functions[@]}"; do
        if ! declare -F "$function_name" >/dev/null; then
            _daia_capability_resolver_error \
                "required Registry Adapter API is unavailable: $function_name"
            return 1
        fi
    done
}

_daia_capability_resolver_first_provider() {
    local providers_output="$1"
    local provider

    while IFS= read -r provider; do
        if [[ -n "$provider" ]]; then
            printf '%s\n' "$provider"
            return 0
        fi
    done <<<"$providers_output"

    return 1
}

###############################################################################
# Public API
###############################################################################

daia_capability_resolver_resolve() {
    local capability="${1-}"
    local provider_count
    local providers_output
    local provider

    _daia_capability_resolver_validate_argument_count \
        1 \
        "daia_capability_resolver_resolve" \
        "$#" || return 1

    _daia_capability_resolver_validate_capability \
        "$capability" || return 1

    _daia_capability_resolver_require_api || return 1

    if ! daia_registry_adapter_capability_exists "$capability"; then
        _daia_capability_resolver_error \
            "unknown capability: $capability"
        return 1
    fi

    provider_count="$(
        daia_registry_adapter_capability_provider_count \
            "$capability"
    )" || {
        _daia_capability_resolver_error \
            "failed to read provider count for capability: $capability"
        return 1
    }

    _daia_capability_resolver_validate_provider_count \
        "$provider_count" \
        "$capability" || return 1

    providers_output="$(
        daia_registry_adapter_capability_providers \
            "$capability"
    )" || {
        _daia_capability_resolver_error \
            "failed to read providers for capability: $capability"
        return 1
    }

    provider="$(
        _daia_capability_resolver_first_provider \
            "$providers_output"
    )" || {
        _daia_capability_resolver_error \
            "no provider was returned for capability: $capability"
        return 1
    }

    _daia_capability_resolver_validate_plugin_id \
        "$provider" || return 1

    printf '%s\n' "$provider"
}

daia_capability_resolver_resolve_many() {
    local capability
    local provider
    local -A selected_providers=()

    _daia_capability_resolver_validate_minimum_argument_count \
        1 \
        "daia_capability_resolver_resolve_many" \
        "$#" || return 1

    for capability in "$@"; do
        provider="$(
            daia_capability_resolver_resolve \
                "$capability"
        )" || return 1

        if [[ -n "${selected_providers[$provider]+defined}" ]]; then
            continue
        fi

        selected_providers["$provider"]=1
        printf '%s\n' "$provider"
    done
}
