#!/usr/bin/env bash

###############################################################################
# DAIA Build State Unit Tests
#
# Unit tests for:
#     installer/files/opt/daia/builder/build-state.sh
###############################################################################

set -o nounset
set -o pipefail

###############################################################################
# Project Paths
###############################################################################

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
BUILD_STATE="${PROJECT_ROOT}/installer/files/opt/daia/builder/build-state.sh"

###############################################################################
# Load Module
###############################################################################

if [[ ! -f "${BUILD_STATE}" ]]; then
    printf 'ERROR: Unable to locate build-state.sh\n' >&2
    exit 1
fi

# shellcheck source=/dev/null
source "${BUILD_STATE}"

###############################################################################
# Test State
###############################################################################

tests_run=0
tests_failed=0

###############################################################################
# Reporting
###############################################################################

pass() {
    local message="$1"

    ((tests_run++))
    printf 'PASS: %s\n' "${message}"
}

fail() {
    local message="$1"

    ((tests_run++))
    ((tests_failed++))

    printf 'FAIL: %s\n' "${message}" >&2
}

###############################################################################
# Assertions
###############################################################################

assert_equals() {
    local expected="$1"
    local actual="$2"
    local message="$3"

    if [[ "${expected}" == "${actual}" ]]; then
        pass "${message}"
    else
        fail "${message}"
        printf '       Expected: %s\n' "${expected}" >&2
        printf '       Actual  : %s\n' "${actual}" >&2
    fi
}

assert_empty() {
    local value="$1"
    local message="$2"

    assert_equals "" "${value}" "${message}"
}

assert_true() {
    local rc="$1"
    local message="$2"

    if [[ "${rc}" -eq 0 ]]; then
        pass "${message}"
    else
        fail "${message}"
    fi
}

assert_false() {
    local rc="$1"
    local message="$2"

    if [[ "${rc}" -ne 0 ]]; then
        pass "${message}"
    else
        fail "${message}"
    fi
}

assert_success() {
    assert_true "$@"
}

assert_failure() {
    assert_false "$@"
}

###############################################################################
# Test Isolation Helpers
###############################################################################

reset_build_state() {
    if daia_build_state_is_initialized; then
        daia_build_state_clear >/dev/null 2>&1 || return 1
    fi

    return 0
}


prepare_initialized_build_state() {
    reset_build_state || return 1
    daia_build_state_init >/dev/null 2>&1
}


###############################################################################
# Lifecycle Tests
###############################################################################

test_init_initializes_build_state() {
    local rc=0

    reset_build_state

    daia_build_state_init >/dev/null 2>&1
    rc=$?

    assert_success "${rc}" \
        "init initializes build state"

    reset_build_state
}


test_init_rejects_double_initialization() {
    local rc=0

    prepare_initialized_build_state

    daia_build_state_init >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "init rejects double initialization"

    reset_build_state
}


test_clear_resets_build_state() {
    local rc=0

    prepare_initialized_build_state
    daia_build_state_clear >/dev/null 2>&1

    daia_build_state_is_initialized
    rc=$?

    assert_false "${rc}" \
        "clear resets build state"

    reset_build_state
}


test_clear_requires_initialization() {
    local rc=0

    reset_build_state

    daia_build_state_clear >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "clear requires initialization"

    reset_build_state
}


test_is_initialized_returns_false_before_init() {
    local rc=0

    reset_build_state

    daia_build_state_is_initialized
    rc=$?

    assert_false "${rc}" \
        "is_initialized returns false before init"
}


test_is_initialized_returns_true_after_init() {
    local rc=0

    prepare_initialized_build_state

    daia_build_state_is_initialized
    rc=$?

    assert_true "${rc}" \
        "is_initialized returns true after init"

    reset_build_state
}


###############################################################################
# Default State Tests
###############################################################################

test_default_status_is_initialized() {
    local actual=""

    if ! prepare_initialized_build_state; then
        fail "default status is initialized (setup failed)"
        return
    fi

    actual="$(daia_build_state_status)"

    assert_equals "initialized" "${actual}" \
        "default status is initialized"

    reset_build_state
}


test_default_phase_is_empty() {
    local actual=""

    if ! prepare_initialized_build_state; then
        fail "default phase is empty (setup failed)"
        return
    fi

    actual="$(daia_build_state_phase)"

    assert_empty "${actual}" \
        "default phase is empty"

    reset_build_state
}


test_default_current_plugin_is_empty() {
    local actual=""

    if ! prepare_initialized_build_state; then
        fail "default current plugin is empty (setup failed)"
        return
    fi

    actual="$(daia_build_state_current_plugin)"

    assert_empty "${actual}" \
        "default current plugin is empty"

    reset_build_state
}


test_default_started_at_is_empty() {
    local actual=""

    if ! prepare_initialized_build_state; then
        fail "default started_at is empty (setup failed)"
        return
    fi

    actual="$(daia_build_state_started_at)"

    assert_empty "${actual}" \
        "default started_at is empty"

    reset_build_state
}


test_default_finished_at_is_empty() {
    local actual=""

    if ! prepare_initialized_build_state; then
        fail "default finished_at is empty (setup failed)"
        return
    fi

    actual="$(daia_build_state_finished_at)"

    assert_empty "${actual}" \
        "default finished_at is empty"

    reset_build_state
}


test_default_failure_message_is_empty() {
    local actual=""

    if ! prepare_initialized_build_state; then
        fail "default failure message is empty (setup failed)"
        return
    fi

    actual="$(daia_build_state_failure_message)"

    assert_empty "${actual}" \
        "default failure message is empty"

    reset_build_state
}


test_default_total_plugins_is_zero() {
    local actual=""

    if ! prepare_initialized_build_state; then
        fail "default total plugins is zero (setup failed)"
        return
    fi

    actual="$(daia_build_state_total_plugins)"

    assert_equals "0" "${actual}" \
        "default total plugins is zero"

    reset_build_state
}


test_default_completed_plugins_is_zero() {
    local actual=""

    if ! prepare_initialized_build_state; then
        fail "default completed plugins is zero (setup failed)"
        return
    fi

    actual="$(daia_build_state_completed_plugins)"

    assert_equals "0" "${actual}" \
        "default completed plugins is zero"

    reset_build_state
}


test_default_failed_plugins_is_zero() {
    local actual=""

    if ! prepare_initialized_build_state; then
        fail "default failed plugins is zero (setup failed)"
        return
    fi

    actual="$(daia_build_state_failed_plugins)"

    assert_equals "0" "${actual}" \
        "default failed plugins is zero"

    reset_build_state
}


test_default_skipped_plugins_is_zero() {
    local actual=""

    if ! prepare_initialized_build_state; then
        fail "default skipped plugins is zero (setup failed)"
        return
    fi

    actual="$(daia_build_state_skipped_plugins)"

    assert_equals "0" "${actual}" \
        "default skipped plugins is zero"

    reset_build_state
}


test_default_processed_plugins_is_zero() {
    local actual=""

    if ! prepare_initialized_build_state; then
        fail "default processed plugins is zero (setup failed)"
        return
    fi

    actual="$(daia_build_state_processed_plugins)"

    assert_equals "0" "${actual}" \
        "default processed plugins is zero"

    reset_build_state
}


###############################################################################
# Status Tests
###############################################################################

test_set_status_accepts_initialized() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_set_status "initialized"

    actual="$(daia_build_state_status)"

    assert_equals "initialized" "${actual}" \
        "set_status accepts initialized"

    reset_build_state
}


test_set_status_accepts_running() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_set_status "running"

    actual="$(daia_build_state_status)"

    assert_equals "running" "${actual}" \
        "set_status accepts running"

    reset_build_state
}


test_set_status_accepts_success() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_set_status "success"

    actual="$(daia_build_state_status)"

    assert_equals "success" "${actual}" \
        "set_status accepts success"

    reset_build_state
}


test_set_status_accepts_failed() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_set_status "failed"

    actual="$(daia_build_state_status)"

    assert_equals "failed" "${actual}" \
        "set_status accepts failed"

    reset_build_state
}


test_set_status_rejects_invalid_status() {
    local rc=0

    prepare_initialized_build_state

    daia_build_state_set_status "invalid" >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "set_status rejects invalid status"

    reset_build_state
}


test_set_status_requires_initialization() {
    local rc=0

    reset_build_state

    daia_build_state_set_status "running" >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "set_status requires initialization"
}


###############################################################################
# Phase Tests
###############################################################################

test_set_phase_accepts_initializing() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_set_phase "initializing"

    actual="$(daia_build_state_phase)"

    assert_equals "initializing" "${actual}" \
        "set_phase accepts initializing"

    reset_build_state
}


test_set_phase_accepts_workspace() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_set_phase "workspace"

    actual="$(daia_build_state_phase)"

    assert_equals "workspace" "${actual}" \
        "set_phase accepts workspace"

    reset_build_state
}


test_set_phase_accepts_plugins() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_set_phase "plugins"

    actual="$(daia_build_state_phase)"

    assert_equals "plugins" "${actual}" \
        "set_phase accepts plugins"

    reset_build_state
}


test_set_phase_accepts_image() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_set_phase "image"

    actual="$(daia_build_state_phase)"

    assert_equals "image" "${actual}" \
        "set_phase accepts image"

    reset_build_state
}


test_set_phase_accepts_cleanup() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_set_phase "cleanup"

    actual="$(daia_build_state_phase)"

    assert_equals "cleanup" "${actual}" \
        "set_phase accepts cleanup"

    reset_build_state
}


test_set_phase_accepts_complete() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_set_phase "complete"

    actual="$(daia_build_state_phase)"

    assert_equals "complete" "${actual}" \
        "set_phase accepts complete"

    reset_build_state
}


test_set_phase_rejects_invalid_phase() {
    local rc=0

    prepare_initialized_build_state

    daia_build_state_set_phase "invalid" >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "set_phase rejects invalid phase"

    reset_build_state
}


test_set_phase_requires_initialization() {
    local rc=0

    reset_build_state

    daia_build_state_set_phase "workspace" >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "set_phase requires initialization"
}


###############################################################################
# Current Plugin Tests
###############################################################################

test_set_current_plugin_stores_plugin_name() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_set_current_plugin "users"

    actual="$(daia_build_state_current_plugin)"

    assert_equals "users" "${actual}" \
        "set_current_plugin stores plugin name"

    reset_build_state
}


test_set_current_plugin_accepts_plugin_name_with_hyphen() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_set_current_plugin "system-packages"

    actual="$(daia_build_state_current_plugin)"

    assert_equals "system-packages" "${actual}" \
        "set_current_plugin accepts plugin name with hyphen"

    reset_build_state
}


test_set_current_plugin_replaces_existing_plugin() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_set_current_plugin "users"
    daia_build_state_set_current_plugin "packages"

    actual="$(daia_build_state_current_plugin)"

    assert_equals "packages" "${actual}" \
        "set_current_plugin replaces existing plugin"

    reset_build_state
}


test_set_current_plugin_rejects_empty_name() {
    local rc=0

    prepare_initialized_build_state

    daia_build_state_set_current_plugin "" >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "set_current_plugin rejects empty name"

    reset_build_state
}


test_set_current_plugin_requires_initialization() {
    local rc=0

    reset_build_state

    daia_build_state_set_current_plugin "users" >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "set_current_plugin requires initialization"
}


test_clear_current_plugin_clears_plugin_name() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_set_current_plugin "users"
    daia_build_state_clear_current_plugin

    actual="$(daia_build_state_current_plugin)"

    assert_empty "${actual}" \
        "clear_current_plugin clears plugin name"

    reset_build_state
}


test_clear_current_plugin_succeeds_when_already_empty() {
    local rc=0

    prepare_initialized_build_state

    daia_build_state_clear_current_plugin >/dev/null 2>&1
    rc=$?

    assert_success "${rc}" \
        "clear_current_plugin succeeds when already empty"

    reset_build_state
}


test_clear_current_plugin_requires_initialization() {
    local rc=0

    reset_build_state

    daia_build_state_clear_current_plugin >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "clear_current_plugin requires initialization"
}


test_current_plugin_requires_initialization() {
    local rc=0

    reset_build_state

    daia_build_state_current_plugin >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "current_plugin requires initialization"
}


###############################################################################
# Started Timestamp Tests
###############################################################################

test_set_started_at_stores_timestamp() {
    local actual=""
    local expected="2026-07-21T22:00:00Z"

    prepare_initialized_build_state

    daia_build_state_set_started_at "${expected}"

    actual="$(daia_build_state_started_at)"

    assert_equals "${expected}" "${actual}" \
        "set_started_at stores timestamp"

    reset_build_state
}


test_set_started_at_replaces_existing_timestamp() {
    local actual=""
    local expected="2026-07-21T22:30:00Z"

    prepare_initialized_build_state

    daia_build_state_set_started_at "2026-07-21T22:00:00Z"
    daia_build_state_set_started_at "${expected}"

    actual="$(daia_build_state_started_at)"

    assert_equals "${expected}" "${actual}" \
        "set_started_at replaces existing timestamp"

    reset_build_state
}


test_set_started_at_rejects_empty_timestamp() {
    local rc=0

    prepare_initialized_build_state

    daia_build_state_set_started_at "" >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "set_started_at rejects empty timestamp"

    reset_build_state
}


test_set_started_at_requires_initialization() {
    local rc=0

    reset_build_state

    daia_build_state_set_started_at \
        "2026-07-21T22:00:00Z" >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "set_started_at requires initialization"
}


test_started_at_requires_initialization() {
    local rc=0

    reset_build_state

    daia_build_state_started_at >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "started_at requires initialization"
}


###############################################################################
# Finished Timestamp Tests
###############################################################################

test_set_finished_at_stores_timestamp() {
    local actual=""
    local expected="2026-07-21T23:00:00Z"

    prepare_initialized_build_state

    daia_build_state_set_finished_at "${expected}"

    actual="$(daia_build_state_finished_at)"

    assert_equals "${expected}" "${actual}" \
        "set_finished_at stores timestamp"

    reset_build_state
}


test_set_finished_at_replaces_existing_timestamp() {
    local actual=""
    local expected="2026-07-21T23:30:00Z"

    prepare_initialized_build_state

    daia_build_state_set_finished_at "2026-07-21T23:00:00Z"
    daia_build_state_set_finished_at "${expected}"

    actual="$(daia_build_state_finished_at)"

    assert_equals "${expected}" "${actual}" \
        "set_finished_at replaces existing timestamp"

    reset_build_state
}


test_set_finished_at_rejects_empty_timestamp() {
    local rc=0

    prepare_initialized_build_state

    daia_build_state_set_finished_at "" >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "set_finished_at rejects empty timestamp"

    reset_build_state
}


test_set_finished_at_requires_initialization() {
    local rc=0

    reset_build_state

    daia_build_state_set_finished_at \
        "2026-07-21T23:00:00Z" >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "set_finished_at requires initialization"
}


test_finished_at_requires_initialization() {
    local rc=0

    reset_build_state

    daia_build_state_finished_at >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "finished_at requires initialization"
}


###############################################################################
# Failure Message Tests
###############################################################################

test_set_failure_message_stores_message() {
    local actual=""
    local expected="plugin execution failed"

    prepare_initialized_build_state

    daia_build_state_set_failure_message "${expected}"

    actual="$(daia_build_state_failure_message)"

    assert_equals "${expected}" "${actual}" \
        "set_failure_message stores message"

    reset_build_state
}


test_set_failure_message_preserves_spaces() {
    local actual=""
    local expected="unable to build image: command returned status 1"

    prepare_initialized_build_state

    daia_build_state_set_failure_message "${expected}"

    actual="$(daia_build_state_failure_message)"

    assert_equals "${expected}" "${actual}" \
        "set_failure_message preserves spaces"

    reset_build_state
}


test_set_failure_message_replaces_existing_message() {
    local actual=""
    local expected="workspace cleanup failed"

    prepare_initialized_build_state

    daia_build_state_set_failure_message "plugin execution failed"
    daia_build_state_set_failure_message "${expected}"

    actual="$(daia_build_state_failure_message)"

    assert_equals "${expected}" "${actual}" \
        "set_failure_message replaces existing message"

    reset_build_state
}


test_set_failure_message_rejects_empty_message() {
    local rc=0

    prepare_initialized_build_state

    daia_build_state_set_failure_message "" >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "set_failure_message rejects empty message"

    reset_build_state
}


test_set_failure_message_requires_initialization() {
    local rc=0

    reset_build_state

    daia_build_state_set_failure_message \
        "plugin execution failed" >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "set_failure_message requires initialization"
}


test_failure_message_requires_initialization() {
    local rc=0

    reset_build_state

    daia_build_state_failure_message >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "failure_message requires initialization"
}


###############################################################################
# Plugin Statistics Tests
###############################################################################

test_set_total_plugins_stores_zero() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_set_total_plugins 0

    actual="$(daia_build_state_total_plugins)"

    assert_equals "0" "${actual}" \
        "set_total_plugins stores zero"

    reset_build_state
}


test_set_total_plugins_stores_positive_value() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_set_total_plugins 25

    actual="$(daia_build_state_total_plugins)"

    assert_equals "25" "${actual}" \
        "set_total_plugins stores positive value"

    reset_build_state
}


test_set_total_plugins_replaces_existing_value() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_set_total_plugins 10
    daia_build_state_set_total_plugins 40

    actual="$(daia_build_state_total_plugins)"

    assert_equals "40" "${actual}" \
        "set_total_plugins replaces existing value"

    reset_build_state
}


test_set_total_plugins_rejects_negative_value() {
    local rc=0

    prepare_initialized_build_state

    daia_build_state_set_total_plugins -1 >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "set_total_plugins rejects negative value"

    reset_build_state
}


test_set_total_plugins_rejects_non_numeric_value() {
    local rc=0

    prepare_initialized_build_state

    daia_build_state_set_total_plugins abc >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "set_total_plugins rejects non numeric value"

    reset_build_state
}


test_set_total_plugins_requires_initialization() {
    local rc=0

    reset_build_state

    daia_build_state_set_total_plugins 5 >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "set_total_plugins requires initialization"
}


test_increment_completed_plugins() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_increment_completed_plugins
    daia_build_state_increment_completed_plugins

    actual="$(daia_build_state_completed_plugins)"

    assert_equals "2" "${actual}" \
        "increment_completed_plugins increments counter"

    reset_build_state
}


test_increment_failed_plugins() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_increment_failed_plugins
    daia_build_state_increment_failed_plugins
    daia_build_state_increment_failed_plugins

    actual="$(daia_build_state_failed_plugins)"

    assert_equals "3" "${actual}" \
        "increment_failed_plugins increments counter"

    reset_build_state
}


test_increment_skipped_plugins() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_increment_skipped_plugins

    actual="$(daia_build_state_skipped_plugins)"

    assert_equals "1" "${actual}" \
        "increment_skipped_plugins increments counter"

    reset_build_state
}


test_increment_completed_plugins_requires_initialization() {
    local rc=0

    reset_build_state

    daia_build_state_increment_completed_plugins >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "increment_completed_plugins requires initialization"
}


test_increment_failed_plugins_requires_initialization() {
    local rc=0

    reset_build_state

    daia_build_state_increment_failed_plugins >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "increment_failed_plugins requires initialization"
}


test_increment_skipped_plugins_requires_initialization() {
    local rc=0

    reset_build_state

    daia_build_state_increment_skipped_plugins >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "increment_skipped_plugins requires initialization"
}


test_processed_plugins_returns_zero() {
    local actual=""

    prepare_initialized_build_state

    actual="$(daia_build_state_processed_plugins)"

    assert_equals "0" "${actual}" \
        "processed_plugins returns zero"

    reset_build_state
}


test_processed_plugins_returns_sum() {
    local actual=""

    prepare_initialized_build_state

    daia_build_state_increment_completed_plugins
    daia_build_state_increment_completed_plugins

    daia_build_state_increment_failed_plugins

    daia_build_state_increment_skipped_plugins
    daia_build_state_increment_skipped_plugins

    actual="$(daia_build_state_processed_plugins)"

    assert_equals "5" "${actual}" \
        "processed_plugins returns combined total"

    reset_build_state
}


test_processed_plugins_requires_initialization() {
    local rc=0

    reset_build_state

    daia_build_state_processed_plugins >/dev/null 2>&1
    rc=$?

    assert_failure "${rc}" \
        "processed_plugins requires initialization"
}


###############################################################################
# Test Registry
###############################################################################

TESTS=(
    # Lifecycle
    test_init_initializes_build_state
    test_init_rejects_double_initialization
    test_clear_resets_build_state
    test_clear_requires_initialization
    test_is_initialized_returns_false_before_init
    test_is_initialized_returns_true_after_init

    # Default State
    test_default_status_is_initialized
    test_default_phase_is_empty
    test_default_current_plugin_is_empty
    test_default_started_at_is_empty
    test_default_finished_at_is_empty
    test_default_failure_message_is_empty
    test_default_total_plugins_is_zero
    test_default_completed_plugins_is_zero
    test_default_failed_plugins_is_zero
    test_default_skipped_plugins_is_zero
    test_default_processed_plugins_is_zero

    # Status
    test_set_status_accepts_initialized
    test_set_status_accepts_running
    test_set_status_accepts_success
    test_set_status_accepts_failed
    test_set_status_rejects_invalid_status
    test_set_status_requires_initialization

    # Phase
    test_set_phase_accepts_initializing
    test_set_phase_accepts_workspace
    test_set_phase_accepts_plugins
    test_set_phase_accepts_image
    test_set_phase_accepts_cleanup
    test_set_phase_accepts_complete
    test_set_phase_rejects_invalid_phase
    test_set_phase_requires_initialization

    # Plugin
    test_set_current_plugin_stores_plugin_name
    test_set_current_plugin_accepts_plugin_name_with_hyphen
    test_set_current_plugin_replaces_existing_plugin
    test_set_current_plugin_rejects_empty_name
    test_set_current_plugin_requires_initialization
    test_clear_current_plugin_clears_plugin_name
    test_clear_current_plugin_succeeds_when_already_empty
    test_clear_current_plugin_requires_initialization
    test_current_plugin_requires_initialization

    # Started Timestamp
    test_set_started_at_stores_timestamp
    test_set_started_at_replaces_existing_timestamp
    test_set_started_at_rejects_empty_timestamp
    test_set_started_at_requires_initialization
    test_started_at_requires_initialization

    # Finished Timestamp
    test_set_finished_at_stores_timestamp
    test_set_finished_at_replaces_existing_timestamp
    test_set_finished_at_rejects_empty_timestamp
    test_set_finished_at_requires_initialization
    test_finished_at_requires_initialization

    # Failure Message
    test_set_failure_message_stores_message
    test_set_failure_message_preserves_spaces
    test_set_failure_message_replaces_existing_message
    test_set_failure_message_rejects_empty_message
    test_set_failure_message_requires_initialization
    test_failure_message_requires_initialization

    # Statistics
    test_set_total_plugins_stores_zero
    test_set_total_plugins_stores_positive_value
    test_set_total_plugins_replaces_existing_value
    test_set_total_plugins_rejects_negative_value
    test_set_total_plugins_rejects_non_numeric_value
    test_set_total_plugins_requires_initialization
    test_increment_completed_plugins
    test_increment_failed_plugins
    test_increment_skipped_plugins
    test_increment_completed_plugins_requires_initialization
    test_increment_failed_plugins_requires_initialization
    test_increment_skipped_plugins_requires_initialization
    test_processed_plugins_returns_zero
    test_processed_plugins_returns_sum
    test_processed_plugins_requires_initialization
)

###############################################################################
# Main
###############################################################################

main() {
    local test

    printf '\n'
    printf '========================================\n'
    printf 'DAIA Build State Unit Tests\n'
    printf '========================================\n\n'

    for test in "${TESTS[@]}"; do
        "${test}"
    done

    printf '\n'
    printf '========================================\n'
    printf 'Tests run : %d\n' "${tests_run}"
    printf 'Failures  : %d\n' "${tests_failed}"
    printf '========================================\n'

    if [[ "${tests_failed}" -eq 0 ]]; then
        printf '\nAll tests passed.\n'
        return 0
    fi

    printf '\nOne or more tests failed.\n' >&2
    return 1
}

main "$@"
