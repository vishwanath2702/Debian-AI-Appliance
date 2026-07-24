#!/usr/bin/env bash

# shellcheck disable=SC2317

# DAIA Builder
#
# Coordinates the build lifecycle using controlled phase callbacks.

set -u
set -o pipefail

# Public API:
#   daia_builder_initialize
#   daia_builder_execute
#   daia_builder_finalize
#   daia_builder_run
#
# Callback contract:
#   workspace callback: no arguments
#   plugins callback:   no arguments
#   image callback:     no arguments
#   cleanup callback:   no arguments
#
# Every callback must return zero on success and non-zero on failure.

if [[ -n "${DAIA_BUILDER_LOADED:-}" ]]; then
    return 0
fi

readonly DAIA_BUILDER_LOADED=1


###############################################################################
# Internal helpers
###############################################################################

_daia_builder_error() {
    local message="${1:-}"

    printf 'DAIA builder error: %s\n' "${message}" >&2
}


_daia_builder_validate_argument_count() {
    local expected_count="${1:-}"
    local function_name="${2:-}"
    local actual_count="${3:-}"

    if ((actual_count != expected_count)); then
        _daia_builder_error \
            "${function_name} expects ${expected_count} argument(s); received ${actual_count}"
        return 1
    fi

    return 0
}


_daia_builder_require_function() {
    local function_name="${1:-}"

    if [[ -z "${function_name}" ]]; then
        _daia_builder_error "required function name must not be empty"
        return 1
    fi

    if ! declare -F "${function_name}" >/dev/null; then
        _daia_builder_error "required API is unavailable: ${function_name}"
        return 1
    fi

    return 0
}


_daia_builder_validate_dependencies() {
    local required_function=""

    for required_function in \
        daia_build_state_init \
        daia_build_state_is_initialized \
        daia_build_state_set_status \
        daia_build_state_set_phase \
        daia_build_state_set_started_at \
        daia_build_state_set_finished_at \
        daia_build_state_set_failure_message \
        daia_build_state_clear_current_plugin
    do
        _daia_builder_require_function "${required_function}" || return 1
    done

    return 0
}


_daia_builder_validate_callback() {
    local callback="${1:-}"
    local phase="${2:-}"

    if [[ -z "${callback}" ]]; then
        _daia_builder_error "${phase} callback must not be empty"
        return 1
    fi

    if ! declare -F "${callback}" >/dev/null; then
        _daia_builder_error \
            "${phase} callback is unavailable: ${callback}"
        return 1
    fi

    return 0
}


_daia_builder_timestamp() {
    date --utc '+%Y-%m-%dT%H:%M:%SZ'
}


_daia_builder_record_failure() {
    local message="${1:-}"
    local timestamp=""

    if [[ -z "${message}" ]]; then
        message="build failed"
    fi

    timestamp="$(_daia_builder_timestamp)" || return 1

    daia_build_state_clear_current_plugin || return 1
    daia_build_state_set_failure_message "${message}" || return 1
    daia_build_state_set_status \
        "${DAIA_BUILD_STATE_STATUS_FAILED}" || return 1
    daia_build_state_set_finished_at "${timestamp}" || return 1

    return 0
}


_daia_builder_execute_phase() {
    local phase="${1:-}"
    local callback="${2:-}"

    daia_build_state_set_phase "${phase}" || return 1

    if ! "${callback}"; then
        _daia_builder_error \
            "build phase failed: ${phase}"
        return 1
    fi

    return 0
}


###############################################################################
# Public API
###############################################################################

# Initialize state for a new build.
#
# Arguments:
#   None.
#
# Returns:
#   0 on success.
#   1 if dependencies are unavailable or state cannot be initialized.
daia_builder_initialize() {
    local timestamp=""

    _daia_builder_validate_dependencies || return 1

    if daia_build_state_is_initialized; then
        _daia_builder_error "a build is already initialized"
        return 1
    fi

    timestamp="$(_daia_builder_timestamp)" || {
        _daia_builder_error "could not create build start timestamp"
        return 1
    }

    daia_build_state_init || return 1
    daia_build_state_set_phase \
        "${DAIA_BUILD_STATE_PHASE_INITIALIZING}" || return 1
    daia_build_state_set_started_at "${timestamp}" || return 1
    daia_build_state_set_status \
        "${DAIA_BUILD_STATE_STATUS_RUNNING}" || return 1

    return 0
}

# Execute the primary build phases.
#
# Arguments:
#   $1 - workspace phase callback
#   $2 - plugins phase callback
#   $3 - image phase callback
#
# Returns:
#   0 when every primary phase succeeds.
#   1 when callback validation or phase execution fails.
daia_builder_execute() {
    local workspace_callback="${1:-}"
    local plugins_callback="${2:-}"
    local image_callback="${3:-}"

    _daia_builder_validate_argument_count \
        3 \
        "daia_builder_execute" \
        "$#" || return 1

    if ! daia_build_state_is_initialized; then
        _daia_builder_error "build state is not initialized"
        return 1
    fi

    _daia_builder_validate_callback \
        "${workspace_callback}" \
        "workspace" || return 1

    _daia_builder_validate_callback \
        "${plugins_callback}" \
        "plugins" || return 1

    _daia_builder_validate_callback \
        "${image_callback}" \
        "image" || return 1

    _daia_builder_execute_phase \
        "${DAIA_BUILD_STATE_PHASE_WORKSPACE}" \
        "${workspace_callback}" || return 1

    _daia_builder_execute_phase \
        "${DAIA_BUILD_STATE_PHASE_PLUGINS}" \
        "${plugins_callback}" || return 1

    _daia_builder_execute_phase \
        "${DAIA_BUILD_STATE_PHASE_IMAGE}" \
        "${image_callback}" || return 1

    return 0
}


# Finalize a successful build.
#
# Arguments:
#   $1 - cleanup phase callback
#
# Returns:
#   0 when cleanup and final state transitions succeed.
#   1 otherwise.
daia_builder_finalize() {
    local cleanup_callback="${1:-}"
    local timestamp=""

    _daia_builder_validate_argument_count \
        1 \
        "daia_builder_finalize" \
        "$#" || return 1

    if ! daia_build_state_is_initialized; then
        _daia_builder_error "build state is not initialized"
        return 1
    fi

    _daia_builder_validate_callback \
        "${cleanup_callback}" \
        "cleanup" || return 1

    _daia_builder_execute_phase \
        "${DAIA_BUILD_STATE_PHASE_CLEANUP}" \
        "${cleanup_callback}" || return 1

    timestamp="$(_daia_builder_timestamp)" || {
        _daia_builder_error "could not create build finish timestamp"
        return 1
    }

    daia_build_state_clear_current_plugin || return 1
    daia_build_state_set_phase \
        "${DAIA_BUILD_STATE_PHASE_COMPLETE}" || return 1
    daia_build_state_set_status \
        "${DAIA_BUILD_STATE_STATUS_SUCCESS}" || return 1
    daia_build_state_set_finished_at "${timestamp}" || return 1

    return 0
}


# Run one complete build.
#
# Arguments:
#   $1 - workspace phase callback
#   $2 - plugins phase callback
#   $3 - image phase callback
#   $4 - cleanup phase callback
#
# Returns:
#   0 when the complete build succeeds.
#   1 when initialization, execution, or cleanup fails.
daia_builder_run() {
    local workspace_callback="${1:-}"
    local plugins_callback="${2:-}"
    local image_callback="${3:-}"
    local cleanup_callback="${4:-}"
    local failure_message=""

    _daia_builder_validate_argument_count \
        4 \
        "daia_builder_run" \
        "$#" || return 1

    daia_builder_initialize || return 1

    if ! daia_builder_execute \
        "${workspace_callback}" \
        "${plugins_callback}" \
        "${image_callback}"
    then
        failure_message="primary build execution failed"

        if ! _daia_builder_record_failure "${failure_message}"; then
            _daia_builder_error \
                "could not record build failure state"
        fi

        return 1
    fi

    if ! daia_builder_finalize "${cleanup_callback}"; then
        failure_message="build cleanup or finalization failed"

        if ! _daia_builder_record_failure "${failure_message}"; then
            _daia_builder_error \
                "could not record build failure state"
        fi

        return 1
    fi

    return 0
}
