#!/usr/bin/env bash
#
# DAIA Capability Synchronizer
#
# Rebuilds the planner Capability Registry from validated plugin data exposed
# through the Registry Adapter. This module depends only on public adapter and
# capability-registry APIs.
#
# Public API:
#   daia_capability_synchronizer_sync
#

if [[ -n "${__DAIA_CAPABILITY_SYNCHRONIZER_MODULE_INITIALIZED:-}" ]]; then
    if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then
        return 0
    fi

    exit 0
fi

__DAIA_CAPABILITY_SYNCHRONIZER_MODULE_INITIALIZED=1

###############################################################################
# Internal helpers
###############################################################################

_daia_capability_synchronizer_error() {
    printf 'DAIA capability-synchronizer error: %s\n' "$*" >&2
}

_daia_capability_synchronizer_validate_argument_count() {
    local expected="$1"
    local function_name="$2"
    local actual="$3"

    if [[ "$actual" -ne "$expected" ]]; then
        _daia_capability_synchronizer_error \
            "$function_name expects $expected argument(s); received $actual"
        return 1
    fi
}

_daia_capability_synchronizer_require_api() {
    local function_name
    local -a required_functions=(
        daia_registry_adapter_plugin_ids
        daia_registry_adapter_plugin_capabilities
        daia_capability_registry_init
        daia_capability_registry_register
    )

    for function_name in "${required_functions[@]}"; do
        if ! declare -F "$function_name" >/dev/null; then
            _daia_capability_synchronizer_error \
                "required API is unavailable: $function_name"
            return 1
        fi
    done
}

_daia_capability_synchronizer_collect() {
    local plugin_id
    local capability
    local plugin_output
    local capability_output
    local -n plugin_ids_ref="$1"
    local -n registration_plugins_ref="$2"
    local -n registration_capabilities_ref="$3"

    plugin_output="$(daia_registry_adapter_plugin_ids)" || {
        _daia_capability_synchronizer_error \
            "could not enumerate plugins through the Registry Adapter"
        return 1
    }

    if [[ -n "$plugin_output" ]]; then
        while IFS= read -r plugin_id; do
            [[ -n "$plugin_id" ]] || continue
            plugin_ids_ref+=("$plugin_id")
        done <<< "$plugin_output"
    fi

    for plugin_id in "${plugin_ids_ref[@]}"; do
        capability_output="$(
            daia_registry_adapter_plugin_capabilities "$plugin_id"
        )" || {
            _daia_capability_synchronizer_error \
                "could not read capabilities for plugin: $plugin_id"
            return 1
        }

        while IFS= read -r capability; do
            [[ -n "$capability" ]] || continue
            registration_plugins_ref+=("$plugin_id")
            registration_capabilities_ref+=("$capability")
        done <<< "$capability_output"
    done
}

_daia_capability_synchronizer_rebuild() {
    local index

    # Arguments are names of caller-owned arrays.
    # shellcheck disable=SC2178
    local -n registration_plugins_ref="$1"
    # shellcheck disable=SC2178
    local -n registration_capabilities_ref="$2"

    daia_capability_registry_init || {
        _daia_capability_synchronizer_error \
            "could not initialize the Capability Registry"
        return 1
    }

    for index in "${!registration_plugins_ref[@]}"; do
        daia_capability_registry_register \
            "${registration_plugins_ref[$index]}" \
            "${registration_capabilities_ref[$index]}" || {
            _daia_capability_synchronizer_error \
                "could not register capability ${registration_capabilities_ref[$index]} for plugin ${registration_plugins_ref[$index]}"
            return 1
        }
    done
}
###############################################################################
# Public API
###############################################################################

daia_capability_synchronizer_sync() {
    # Arrays are populated indirectly through nameref parameters.
    # shellcheck disable=SC2034
    local -a plugin_ids=()
    # shellcheck disable=SC2034
    local -a registration_plugins=()
    # shellcheck disable=SC2034
    local -a registration_capabilities=()

    _daia_capability_synchronizer_validate_argument_count \
        0 "daia_capability_synchronizer_sync" "$#" || return 1

}
