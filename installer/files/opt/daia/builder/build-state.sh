#!/usr/bin/env bash

# DAIA Build State
#
# Maintains the mutable state of a single DAIA build.
#
# This module stores build status, phase, timestamps, plugin progress,
# counters, and failure information. It does not enforce build workflow,
# perform logging, or execute build operations.

declare -gi __DAIA_BUILD_STATE_INITIALIZED=0
declare -gA __DAIA_BUILD_STATE=()

declare -gr DAIA_BUILD_STATE_STATUS_INITIALIZED="initialized"
declare -gr DAIA_BUILD_STATE_STATUS_RUNNING="running"
declare -gr DAIA_BUILD_STATE_STATUS_SUCCESS="success"
declare -gr DAIA_BUILD_STATE_STATUS_FAILED="failed"

declare -gr DAIA_BUILD_STATE_PHASE_INITIALIZING="initializing"
declare -gr DAIA_BUILD_STATE_PHASE_WORKSPACE="workspace"
declare -gr DAIA_BUILD_STATE_PHASE_PLUGINS="plugins"
declare -gr DAIA_BUILD_STATE_PHASE_IMAGE="image"
declare -gr DAIA_BUILD_STATE_PHASE_CLEANUP="cleanup"
declare -gr DAIA_BUILD_STATE_PHASE_COMPLETE="complete"


# -----------------------------------------------------------------------------
# Private helpers
# -----------------------------------------------------------------------------

_daia_build_state_error() {
    local message="${1:-}"

    printf 'DAIA build-state error: %s\n' "${message}" >&2
}


_daia_build_state_require_initialization() {
    if [[ "${__DAIA_BUILD_STATE_INITIALIZED}" -ne 1 ]]; then
        _daia_build_state_error "build state is not initialized"
        return 1
    fi

    return 0
}


_daia_build_state_set() {
    local key="${1:-}"
    local value="${2-}"

    if ! _daia_build_state_require_initialization; then
        return 1
    fi

    if [[ -z "${key}" ]]; then
        _daia_build_state_error "state key must not be empty"
        return 1
    fi

    __DAIA_BUILD_STATE["${key}"]="${value}"

    return 0
}


_daia_build_state_get() {
    local key="${1:-}"

    if ! _daia_build_state_require_initialization; then
        return 1
    fi

    if [[ -z "${key}" ]]; then
        _daia_build_state_error "state key must not be empty"
        return 1
    fi

    printf '%s\n' "${__DAIA_BUILD_STATE[${key}]-}"

    return 0
}


_daia_build_state_validate_status() {
    local status="${1:-}"

    case "${status}" in
        "${DAIA_BUILD_STATE_STATUS_INITIALIZED}"|"${DAIA_BUILD_STATE_STATUS_RUNNING}"|"${DAIA_BUILD_STATE_STATUS_SUCCESS}"|"${DAIA_BUILD_STATE_STATUS_FAILED}")
            return 0
            ;;
        *)
            _daia_build_state_error "invalid build status: ${status}"
            return 1
            ;;
    esac
}


_daia_build_state_validate_phase() {
    local phase="${1:-}"

    case "${phase}" in
        "${DAIA_BUILD_STATE_PHASE_INITIALIZING}"|"${DAIA_BUILD_STATE_PHASE_WORKSPACE}"|"${DAIA_BUILD_STATE_PHASE_PLUGINS}"|"${DAIA_BUILD_STATE_PHASE_IMAGE}"|"${DAIA_BUILD_STATE_PHASE_CLEANUP}"|"${DAIA_BUILD_STATE_PHASE_COMPLETE}")
            return 0
            ;;
        *)
            _daia_build_state_error "invalid build phase: ${phase}"
            return 1
            ;;
    esac
}


_daia_build_state_validate_count() {
    local count="${1:-}"

    if [[ ! "${count}" =~ ^[0-9]+$ ]]; then
        _daia_build_state_error \
            "count must be a non-negative integer: ${count}"
        return 1
    fi

    return 0
}


_daia_build_state_increment() {
    local key="${1:-}"
    local current=""

    if ! _daia_build_state_require_initialization; then
        return 1
    fi

    if [[ -z "${key}" ]]; then
        _daia_build_state_error "counter key must not be empty"
        return 1
    fi

    current="${__DAIA_BUILD_STATE[${key}]-}"

    if ! _daia_build_state_validate_count "${current}"; then
        return 1
    fi

    __DAIA_BUILD_STATE["${key}"]=$((current + 1))

    return 0
}


# -----------------------------------------------------------------------------
# Lifecycle
# -----------------------------------------------------------------------------

daia_build_state_init() {
    if [[ "${__DAIA_BUILD_STATE_INITIALIZED}" -eq 1 ]]; then
        _daia_build_state_error "build state is already initialized"
        return 1
    fi

    __DAIA_BUILD_STATE=(
        [status]="${DAIA_BUILD_STATE_STATUS_INITIALIZED}"
        [phase]=""
        [current_plugin]=""
        [started_at]=""
        [finished_at]=""
        [failure_message]=""
        [total_plugins]=0
        [completed_plugins]=0
        [failed_plugins]=0
        [skipped_plugins]=0
    )

    __DAIA_BUILD_STATE_INITIALIZED=1

    return 0
}


daia_build_state_clear() {
    if ! _daia_build_state_require_initialization; then
        return 1
    fi

    __DAIA_BUILD_STATE=()
    __DAIA_BUILD_STATE_INITIALIZED=0

    return 0
}


daia_build_state_is_initialized() {
    [[ "${__DAIA_BUILD_STATE_INITIALIZED}" -eq 1 ]]
}


# -----------------------------------------------------------------------------
# Status
# -----------------------------------------------------------------------------

daia_build_state_set_status() {
    local status="${1:-}"

    if ! _daia_build_state_require_initialization; then
        return 1
    fi

    if ! _daia_build_state_validate_status "${status}"; then
        return 1
    fi

    _daia_build_state_set status "${status}"
}


daia_build_state_status() {
    _daia_build_state_get status
}


# -----------------------------------------------------------------------------
# Phase
# -----------------------------------------------------------------------------

daia_build_state_set_phase() {
    local phase="${1:-}"

    if ! _daia_build_state_require_initialization; then
        return 1
    fi

    if ! _daia_build_state_validate_phase "${phase}"; then
        return 1
    fi

    _daia_build_state_set phase "${phase}"
}


daia_build_state_phase() {
    _daia_build_state_get phase
}


# -----------------------------------------------------------------------------
# Current plugin
# -----------------------------------------------------------------------------

daia_build_state_set_current_plugin() {
    local plugin="${1:-}"

    if ! _daia_build_state_require_initialization; then
        return 1
    fi

    if [[ -z "${plugin}" ]]; then
        _daia_build_state_error "plugin name must not be empty"
        return 1
    fi

    _daia_build_state_set current_plugin "${plugin}"
}


daia_build_state_clear_current_plugin() {
    _daia_build_state_set current_plugin ""
}


daia_build_state_current_plugin() {
    _daia_build_state_get current_plugin
}


# -----------------------------------------------------------------------------
# Timing
# -----------------------------------------------------------------------------

daia_build_state_set_started_at() {
    local timestamp="${1:-}"

    if ! _daia_build_state_require_initialization; then
        return 1
    fi

    if [[ -z "${timestamp}" ]]; then
        _daia_build_state_error "start timestamp must not be empty"
        return 1
    fi

    _daia_build_state_set started_at "${timestamp}"
}


daia_build_state_started_at() {
    _daia_build_state_get started_at
}


daia_build_state_set_finished_at() {
    local timestamp="${1:-}"

    if ! _daia_build_state_require_initialization; then
        return 1
    fi

    if [[ -z "${timestamp}" ]]; then
        _daia_build_state_error "finish timestamp must not be empty"
        return 1
    fi

    _daia_build_state_set finished_at "${timestamp}"
}


daia_build_state_finished_at() {
    _daia_build_state_get finished_at
}


# -----------------------------------------------------------------------------
# Failure information
# -----------------------------------------------------------------------------

daia_build_state_set_failure_message() {
    local message="${1:-}"

    if ! _daia_build_state_require_initialization; then
        return 1
    fi

    if [[ -z "${message}" ]]; then
        _daia_build_state_error "failure message must not be empty"
        return 1
    fi

    _daia_build_state_set failure_message "${message}"
}


daia_build_state_failure_message() {
    _daia_build_state_get failure_message
}


# -----------------------------------------------------------------------------
# Plugin statistics
# -----------------------------------------------------------------------------

daia_build_state_set_total_plugins() {
    local count="${1:-}"

    if ! _daia_build_state_require_initialization; then
        return 1
    fi

    if ! _daia_build_state_validate_count "${count}"; then
        return 1
    fi

    _daia_build_state_set total_plugins "${count}"
}


daia_build_state_total_plugins() {
    _daia_build_state_get total_plugins
}


daia_build_state_increment_completed_plugins() {
    _daia_build_state_increment completed_plugins
}


daia_build_state_completed_plugins() {
    _daia_build_state_get completed_plugins
}


daia_build_state_increment_failed_plugins() {
    _daia_build_state_increment failed_plugins
}


daia_build_state_failed_plugins() {
    _daia_build_state_get failed_plugins
}


daia_build_state_increment_skipped_plugins() {
    _daia_build_state_increment skipped_plugins
}


daia_build_state_skipped_plugins() {
    _daia_build_state_get skipped_plugins
}


daia_build_state_processed_plugins() {
    local completed=""
    local failed=""
    local skipped=""

    if ! _daia_build_state_require_initialization; then
        return 1
    fi

    completed="${__DAIA_BUILD_STATE[completed_plugins]}"
    failed="${__DAIA_BUILD_STATE[failed_plugins]}"
    skipped="${__DAIA_BUILD_STATE[skipped_plugins]}"

    printf '%s\n' "$((completed + failed + skipped))"

    return 0
}
