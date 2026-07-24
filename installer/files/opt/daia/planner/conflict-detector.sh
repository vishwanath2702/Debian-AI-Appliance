#!/usr/bin/env bash
#
# DAIA Planner Conflict Detector
#
# Validates that a collection of selected plugins can coexist.
#
# Public API:
#
#   daia_conflict_detector_check <plugin-id>...
#

if [[ -n "${DAIA_CONFLICT_DETECTOR_LOADED:-}" ]]; then
   # shellcheck disable=SC2317
    return 0 2>/dev/null || exit 0
fi
readonly DAIA_CONFLICT_DETECTOR_LOADED=1

###############################################################################
# Diagnostics
###############################################################################

_daia_conflict_detector_error() {
    printf 'daia-conflict-detector: %s\n' "$*" >&2
}

###############################################################################
# Validation
###############################################################################

_daia_conflict_detector_validate_argument_count() {
    local minimum_count="$1"
    local function_name="$2"
    local actual_count="$3"

    if ((actual_count < minimum_count)); then
        _daia_conflict_detector_error \
            "${function_name}: expected at least ${minimum_count} argument(s)"
        return 1
    fi
}

_daia_conflict_detector_validate_plugin_id() {
    local plugin_id="$1"

    if [[ -z "$plugin_id" ]]; then
        _daia_conflict_detector_error "plugin ID must not be empty"
        return 1
    fi

    if [[ ! "$plugin_id" =~ ^[a-z0-9][a-z0-9._-]*/[a-z0-9][a-z0-9._-]*$ ]]; then
        _daia_conflict_detector_error "invalid plugin ID: $plugin_id"
        return 1
    fi
}

_daia_conflict_detector_require_registry_adapter_api() {
    local function_name="$1"

    if ! declare -F "$function_name" >/dev/null; then
        _daia_conflict_detector_error \
            "required Registry Adapter API is unavailable: $function_name"
        return 1
    fi
}

_daia_conflict_detector_require_registry_adapter() {
    _daia_conflict_detector_require_registry_adapter_api \
        daia_registry_adapter_plugin_exists || return 1

    _daia_conflict_detector_require_registry_adapter_api \
        daia_registry_adapter_plugin_conflicts || return 1
}

###############################################################################
# Internal helpers
###############################################################################

_daia_conflict_detector_load_conflicts() {
    local plugin_id="$1"
    local conflicts_output

    if ! conflicts_output="$(
        daia_registry_adapter_plugin_conflicts "$plugin_id"
    )"; then
        _daia_conflict_detector_error \
            "failed to read conflicts for plugin: $plugin_id"
        return 1
    fi

    printf '%s' "$conflicts_output"
}

###############################################################################
# Public API
###############################################################################

daia_conflict_detector_check() {
    _daia_conflict_detector_validate_argument_count \
        1 \
        "daia_conflict_detector_check" \
        "$#" || return 1

    _daia_conflict_detector_require_registry_adapter || return 1

    # shellcheck disable=SC2034
    local -A selected_plugins=()

    local plugin_id

    for plugin_id in "$@"; do
        _daia_conflict_detector_validate_plugin_id "$plugin_id" ||
            return 1

        if ! daia_registry_adapter_plugin_exists "$plugin_id"; then
            _daia_conflict_detector_error \
                "plugin is not registered: $plugin_id"
            return 1
        fi

        selected_plugins["$plugin_id"]=1
    done

    local conflicts_output
    local conflicting_plugin

    for plugin_id in "${!selected_plugins[@]}"; do
        conflicts_output="$(
            _daia_conflict_detector_load_conflicts "$plugin_id"
        )" || return 1

        while IFS= read -r conflicting_plugin; do
            [[ -n "$conflicting_plugin" ]] || continue

            _daia_conflict_detector_validate_plugin_id \
                "$conflicting_plugin" || {
                _daia_conflict_detector_error \
                    "plugin '$plugin_id' declares an invalid conflict"
                return 1
            }

            if [[ -n "${selected_plugins[$conflicting_plugin]+_}" ]]; then
                _daia_conflict_detector_error \
                    "$plugin_id conflicts with $conflicting_plugin"
                return 1
            fi
        done <<<"$conflicts_output"
    done

    return 0
}
