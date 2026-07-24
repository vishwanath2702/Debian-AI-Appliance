#!/usr/bin/env bash
#
# DAIA Planner Dependency Resolver
#
# Expands one or more selected plugin IDs into their complete transitive
# dependency set through the public Registry Adapter API.
#
# Resolution policy:
#
#   - unknown plugins are rejected
#   - dependency lookup failures are rejected
#   - invalid dependency plugin IDs are rejected
#   - dependency cycles are rejected
#   - dependencies are emitted before the plugins that require them
#   - duplicate plugins are emitted only once
#
# Public API:
#   daia_dependency_resolver_resolve <plugin-id>
#   daia_dependency_resolver_resolve_many <plugin-id>...
#

if [[ -n "${__DAIA_DEPENDENCY_RESOLVER_MODULE_INITIALIZED:-}" ]]; then
    if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then
        return 0
    fi

    exit 0
fi

__DAIA_DEPENDENCY_RESOLVER_MODULE_INITIALIZED=1

###############################################################################
# Internal helpers
###############################################################################

_daia_dependency_resolver_error() {
    printf 'DAIA dependency resolver error: %s\n' "$*" >&2
}

_daia_dependency_resolver_validate_argument_count() {
    local expected="$1"
    local function_name="$2"
    local actual="$3"

    if [[ "$actual" -ne "$expected" ]]; then
        _daia_dependency_resolver_error \
            "$function_name expects $expected argument(s); received $actual"
        return 1
    fi
}

_daia_dependency_resolver_validate_minimum_argument_count() {
    local minimum="$1"
    local function_name="$2"
    local actual="$3"

    if [[ "$actual" -lt "$minimum" ]]; then
        _daia_dependency_resolver_error \
            "$function_name expects at least $minimum argument(s); received $actual"
        return 1
    fi
}

_daia_dependency_resolver_validate_plugin_id() {
    local plugin_id="${1-}"

    if [[ -z "$plugin_id" ]]; then
        _daia_dependency_resolver_error \
            "plugin ID must not be empty"
        return 1
    fi

    if [[ ! "$plugin_id" =~ ^[a-z0-9]+([._-][a-z0-9]+)*/[a-z0-9]+([._-][a-z0-9]+)*$ ]]; then
        _daia_dependency_resolver_error \
            "invalid plugin ID: $plugin_id"
        return 1
    fi
}

_daia_dependency_resolver_require_api() {
    local function_name
    local -a required_functions=(
        daia_registry_adapter_plugin_exists
        daia_registry_adapter_plugin_dependencies
    )

    for function_name in "${required_functions[@]}"; do
        if ! declare -F "$function_name" >/dev/null; then
            _daia_dependency_resolver_error \
                "required Registry Adapter API is unavailable: $function_name"
            return 1
        fi
    done
}

_daia_dependency_resolver_visit() {
    local plugin_id="$1"
    local resolved_name="$2"
    local visiting_name="$3"
    local dependencies_output
    local dependency
    local -n resolved_ref="$resolved_name"
    local -n visiting_ref="$visiting_name"

    if [[ -n "${resolved_ref[$plugin_id]+defined}" ]]; then
        return 0
    fi

    if [[ -n "${visiting_ref[$plugin_id]+defined}" ]]; then
        _daia_dependency_resolver_error \
            "dependency cycle detected at plugin: $plugin_id"
        return 1
    fi

    _daia_dependency_resolver_validate_plugin_id \
        "$plugin_id" || return 1

    if ! daia_registry_adapter_plugin_exists "$plugin_id"; then
        _daia_dependency_resolver_error \
            "unknown plugin: $plugin_id"
        return 1
    fi

    visiting_ref["$plugin_id"]=1

    dependencies_output="$(
        daia_registry_adapter_plugin_dependencies \
            "$plugin_id"
    )" || {
        unset 'visiting_ref[$plugin_id]'

        _daia_dependency_resolver_error \
            "failed to read dependencies for plugin: $plugin_id"
        return 1
    }

    while IFS= read -r dependency || [[ -n "$dependency" ]]; do
        if [[ -z "$dependency" ]]; then
            continue
        fi

        _daia_dependency_resolver_validate_plugin_id \
            "$dependency" || {
            unset 'visiting_ref[$plugin_id]'
            return 1
        }

        _daia_dependency_resolver_visit \
            "$dependency" \
            "$resolved_name" \
            "$visiting_name" || {
            unset 'visiting_ref[$plugin_id]'
            return 1
        }
    done <<<"$dependencies_output"

    unset 'visiting_ref[$plugin_id]'

    resolved_ref["$plugin_id"]=1
    printf '%s\n' "$plugin_id"
}

###############################################################################
# Public API
###############################################################################
# shellcheck disable=SC2034
daia_dependency_resolver_resolve() {
    local plugin_id="${1-}"
    local -A resolved_plugins=()
    local -A visiting_plugins=()

    _daia_dependency_resolver_validate_argument_count \
        1 \
        "daia_dependency_resolver_resolve" \
        "$#" || return 1

    _daia_dependency_resolver_validate_plugin_id \
        "$plugin_id" || return 1

    _daia_dependency_resolver_require_api || return 1

    _daia_dependency_resolver_visit \
        "$plugin_id" \
        resolved_plugins \
        visiting_plugins
}
# shellcheck disable=SC2034
daia_dependency_resolver_resolve_many() {
    local plugin_id
# shellcheck disable=SC2034
    local -A resolved_plugins=()
# shellcheck disable=SC2034
    local -A visiting_plugins=()

    _daia_dependency_resolver_validate_minimum_argument_count \
        1 \
        "daia_dependency_resolver_resolve_many" \
        "$#" || return 1

    _daia_dependency_resolver_require_api || return 1

    for plugin_id in "$@"; do
        _daia_dependency_resolver_validate_plugin_id \
            "$plugin_id" || return 1

        _daia_dependency_resolver_visit \
            "$plugin_id" \
            resolved_plugins \
            visiting_plugins || return 1
    done
}
