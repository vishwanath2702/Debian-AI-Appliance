#!/usr/bin/env bash

# shellcheck disable=SC2317

# DAIA Builder Tests
#
# Tests the Builder lifecycle using controlled phase callbacks.
#
# Expected files in the same directory:
#   build-state.sh
#   builder.sh

set -u
set -o pipefail
TEST_SCRIPT_DIR="$(
    cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 &&
        pwd
)" || {
    printf 'TEST ERROR: could not determine script directory\n' >&2
    exit 1
}

readonly TEST_SCRIPT_DIR

readonly BUILD_STATE_SCRIPT="${TEST_SCRIPT_DIR}/build-state.sh"
readonly BUILDER_SCRIPT="${TEST_SCRIPT_DIR}/builder.sh"

declare -gi TESTS_RUN=0
declare -gi TESTS_PASSED=0
declare -gi TESTS_FAILED=0

declare -ga TEST_PHASE_CALLS=()


###############################################################################
# Test framework
###############################################################################

_test_error() {
    local message="${1:-}"

    printf 'TEST ERROR: %s\n' "${message}" >&2
}


_test_fail() {
    local message="${1:-}"

    TESTS_FAILED=$((TESTS_FAILED + 1))

    printf 'not ok - %s\n' "${message}"

    return 1
}


_test_pass() {
    local message="${1:-}"

    TESTS_PASSED=$((TESTS_PASSED + 1))

    printf 'ok - %s\n' "${message}"

    return 0
}


_test_assert_equal() {
    local expected="${1-}"
    local actual="${2-}"
    local description="${3:-values should be equal}"

    if [[ "${actual}" != "${expected}" ]]; then
        _test_error "${description}"
        _test_error "expected: ${expected}"
        _test_error "actual:   ${actual}"
        return 1
    fi

    return 0
}


_test_assert_success() {
    local exit_status="${1:-1}"
    local description="${2:-command should succeed}"

    if ((exit_status != 0)); then
        _test_error "${description}"
        _test_error "exit status: ${exit_status}"
        return 1
    fi

    return 0
}


_test_assert_failure() {
    local exit_status="${1:-0}"
    local description="${2:-command should fail}"

    if ((exit_status == 0)); then
        _test_error "${description}"
        _test_error "command unexpectedly succeeded"
        return 1
    fi

    return 0
}


_test_assert_state_initialized() {
    if ! daia_build_state_is_initialized; then
        _test_error "build state should be initialized"
        return 1
    fi

    return 0
}

_test_assert_state_not_initialized() {
    local description="${1:-build state should not be initialized}"

    if daia_build_state_is_initialized; then
        _test_error "${description}"
        return 1
    fi

    return 0
}


_test_reset_phase_calls() {
    TEST_PHASE_CALLS=()
}


_test_phase_call_sequence() {
    local separator=">"
    local output=""
    local phase=""

    for phase in "${TEST_PHASE_CALLS[@]}"; do
        if [[ -n "${output}" ]]; then
            output+="${separator}"
        fi

        output+="${phase}"
    done

    printf '%s\n' "${output}"
}


_test_reset_build_state() {
    if daia_build_state_is_initialized; then
        daia_build_state_clear >/dev/null 2>&1 || {
            _test_error "could not clear build state"
            return 1
        }
    fi

    return 0
}


_test_prepare() {
    _test_reset_phase_calls
    _test_reset_build_state
}


_test_finish() {
    _test_reset_build_state
}


_test_run() {
    local test_name="${1:-}"
    local test_function="${2:-}"
    local test_status=0

    TESTS_RUN=$((TESTS_RUN + 1))

    if ! _test_prepare; then
        _test_fail "${test_name}: test setup failed"
        return 1
    fi

    "${test_function}"
    test_status=$?

    if ! _test_finish; then
        _test_fail "${test_name}: test cleanup failed"
        return 1
    fi

    if ((test_status == 0)); then
        _test_pass "${test_name}"
        return 0
    fi

    _test_fail "${test_name}"
    return 1
}


###############################################################################
# Test callbacks
###############################################################################

test_workspace_success() {
    TEST_PHASE_CALLS+=("workspace")
    return 0
}


test_workspace_failure() {
    TEST_PHASE_CALLS+=("workspace")
    return 1
}


test_plugins_success() {
    TEST_PHASE_CALLS+=("plugins")
    return 0
}


test_plugins_failure() {
    TEST_PHASE_CALLS+=("plugins")
    return 1
}


test_image_success() {
    TEST_PHASE_CALLS+=("image")
    return 0
}


test_image_failure() {
    TEST_PHASE_CALLS+=("image")
    return 1
}


test_cleanup_success() {
    TEST_PHASE_CALLS+=("cleanup")
    return 0
}


test_cleanup_failure() {
    TEST_PHASE_CALLS+=("cleanup")
    return 1
}


###############################################################################
# Tests
###############################################################################

test_successful_build() {
    local exit_status=0
    local status=""
    local phase=""
    local started_at=""
    local finished_at=""
    local failure_message=""
    local call_sequence=""

    daia_builder_run \
        test_workspace_success \
        test_plugins_success \
        test_image_success \
        test_cleanup_success
    exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "successful build should return zero" || return 1

    _test_assert_state_initialized || return 1

    status="$(daia_build_state_status)" || return 1
    phase="$(daia_build_state_phase)" || return 1
    started_at="$(daia_build_state_started_at)" || return 1
    finished_at="$(daia_build_state_finished_at)" || return 1
    failure_message="$(daia_build_state_failure_message)" || return 1
    call_sequence="$(_test_phase_call_sequence)"

    _test_assert_equal \
        "${DAIA_BUILD_STATE_STATUS_SUCCESS}" \
        "${status}" \
        "successful build should have success status" || return 1

    _test_assert_equal \
        "${DAIA_BUILD_STATE_PHASE_COMPLETE}" \
        "${phase}" \
        "successful build should finish in complete phase" || return 1

    _test_assert_equal \
        "workspace>plugins>image>cleanup" \
        "${call_sequence}" \
        "successful build should execute phases in order" || return 1

    if [[ -z "${started_at}" ]]; then
        _test_error "successful build should record a start timestamp"
        return 1
    fi

    if [[ -z "${finished_at}" ]]; then
        _test_error "successful build should record a finish timestamp"
        return 1
    fi

    _test_assert_equal \
        "" \
        "${failure_message}" \
        "successful build should not record a failure message" || return 1

    return 0
}


test_workspace_failure_stops_build() {
    local exit_status=0
    local status=""
    local phase=""
    local failure_message=""
    local call_sequence=""

    daia_builder_run \
        test_workspace_failure \
        test_plugins_success \
        test_image_success \
        test_cleanup_success \
        >/dev/null 2>&1
    exit_status=$?

    _test_assert_failure \
        "${exit_status}" \
        "workspace failure should fail the build" || return 1

    status="$(daia_build_state_status)" || return 1
    phase="$(daia_build_state_phase)" || return 1
    failure_message="$(daia_build_state_failure_message)" || return 1
    call_sequence="$(_test_phase_call_sequence)"

    _test_assert_equal \
        "${DAIA_BUILD_STATE_STATUS_FAILED}" \
        "${status}" \
        "workspace failure should set failed status" || return 1

    _test_assert_equal \
        "${DAIA_BUILD_STATE_PHASE_WORKSPACE}" \
        "${phase}" \
        "workspace failure should preserve workspace phase" || return 1

    _test_assert_equal \
        "workspace" \
        "${call_sequence}" \
        "workspace failure should stop later callbacks" || return 1

    _test_assert_equal \
        "primary build execution failed" \
        "${failure_message}" \
        "workspace failure should record a failure message" || return 1

    return 0
}


test_plugin_failure_stops_image() {
    local exit_status=0
    local status=""
    local phase=""
    local call_sequence=""

    daia_builder_run \
        test_workspace_success \
        test_plugins_failure \
        test_image_success \
        test_cleanup_success \
        >/dev/null 2>&1
    exit_status=$?

    _test_assert_failure \
        "${exit_status}" \
        "plugin failure should fail the build" || return 1

    status="$(daia_build_state_status)" || return 1
    phase="$(daia_build_state_phase)" || return 1
    call_sequence="$(_test_phase_call_sequence)"

    _test_assert_equal \
        "${DAIA_BUILD_STATE_STATUS_FAILED}" \
        "${status}" \
        "plugin failure should set failed status" || return 1

    _test_assert_equal \
        "${DAIA_BUILD_STATE_PHASE_PLUGINS}" \
        "${phase}" \
        "plugin failure should preserve plugins phase" || return 1

    _test_assert_equal \
        "workspace>plugins" \
        "${call_sequence}" \
        "plugin failure should prevent image and cleanup callbacks" || return 1

    return 0
}


test_image_failure_stops_cleanup() {
    local exit_status=0
    local status=""
    local phase=""
    local call_sequence=""

    daia_builder_run \
        test_workspace_success \
        test_plugins_success \
        test_image_failure \
        test_cleanup_success \
        >/dev/null 2>&1
    exit_status=$?

    _test_assert_failure \
        "${exit_status}" \
        "image failure should fail the build" || return 1

    status="$(daia_build_state_status)" || return 1
    phase="$(daia_build_state_phase)" || return 1
    call_sequence="$(_test_phase_call_sequence)"

    _test_assert_equal \
        "${DAIA_BUILD_STATE_STATUS_FAILED}" \
        "${status}" \
        "image failure should set failed status" || return 1

    _test_assert_equal \
        "${DAIA_BUILD_STATE_PHASE_IMAGE}" \
        "${phase}" \
        "image failure should preserve image phase" || return 1

    _test_assert_equal \
        "workspace>plugins>image" \
        "${call_sequence}" \
        "image failure should prevent cleanup callback" || return 1

    return 0
}


test_cleanup_failure_marks_build_failed() {
    local exit_status=0
    local status=""
    local phase=""
    local failure_message=""
    local call_sequence=""

    daia_builder_run \
        test_workspace_success \
        test_plugins_success \
        test_image_success \
        test_cleanup_failure \
        >/dev/null 2>&1
    exit_status=$?

    _test_assert_failure \
        "${exit_status}" \
        "cleanup failure should fail the build" || return 1

    status="$(daia_build_state_status)" || return 1
    phase="$(daia_build_state_phase)" || return 1
    failure_message="$(daia_build_state_failure_message)" || return 1
    call_sequence="$(_test_phase_call_sequence)"

    _test_assert_equal \
        "${DAIA_BUILD_STATE_STATUS_FAILED}" \
        "${status}" \
        "cleanup failure should set failed status" || return 1

    _test_assert_equal \
        "${DAIA_BUILD_STATE_PHASE_CLEANUP}" \
        "${phase}" \
        "cleanup failure should preserve cleanup phase" || return 1

    _test_assert_equal \
        "workspace>plugins>image>cleanup" \
        "${call_sequence}" \
        "cleanup failure should occur after all primary phases" || return 1

    _test_assert_equal \
        "build cleanup or finalization failed" \
        "${failure_message}" \
        "cleanup failure should record a failure message" || return 1

    return 0
}


test_duplicate_initialization_is_rejected() {
    local first_status=0
    local second_status=0
    local status=""

    daia_builder_initialize
    first_status=$?

    daia_builder_initialize >/dev/null 2>&1
    second_status=$?

    _test_assert_success \
        "${first_status}" \
        "first initialization should succeed" || return 1

    _test_assert_failure \
        "${second_status}" \
        "duplicate initialization should fail" || return 1

    status="$(daia_build_state_status)" || return 1

    _test_assert_equal \
        "${DAIA_BUILD_STATE_STATUS_RUNNING}" \
        "${status}" \
        "duplicate initialization should not corrupt running state" || return 1

    return 0
}


test_missing_callback_is_rejected() {
    local exit_status=0
    local status=""
    local phase=""
    local call_sequence=""

    daia_builder_run \
        test_workspace_success \
        nonexistent_plugins_callback \
        test_image_success \
        test_cleanup_success \
        >/dev/null 2>&1
    exit_status=$?

    _test_assert_failure \
        "${exit_status}" \
        "missing callback should fail the build" || return 1

    status="$(daia_build_state_status)" || return 1
    phase="$(daia_build_state_phase)" || return 1
    call_sequence="$(_test_phase_call_sequence)"

    _test_assert_equal \
        "${DAIA_BUILD_STATE_STATUS_FAILED}" \
        "${status}" \
        "missing callback should set failed status" || return 1

    _test_assert_equal \
        "${DAIA_BUILD_STATE_PHASE_INITIALIZING}" \
        "${phase}" \
        "missing callback should fail before phase execution" || return 1

    _test_assert_equal \
        "" \
        "${call_sequence}" \
        "missing callback should prevent every callback from running" || return 1

    return 0
}


test_execute_requires_initialization() {
    local exit_status=0

    daia_builder_execute \
        test_workspace_success \
        test_plugins_success \
        test_image_success \
        >/dev/null 2>&1
    exit_status=$?

    _test_assert_failure \
        "${exit_status}" \
        "execute should reject uninitialized state" || return 1

    _test_assert_state_not_initialized \
        "failed execute call should not initialize state" || return 1

    return 0
}


test_execute_rejects_incorrect_argument_count() {
    local exit_status=0
    local status=""
    local phase=""

    daia_builder_initialize >/dev/null 2>&1 || return 1

    daia_builder_execute \
        test_workspace_success \
        test_plugins_success \
        >/dev/null 2>&1
    exit_status=$?

    _test_assert_failure \
        "${exit_status}" \
        "execute should reject an incorrect argument count" || return 1

    status="$(daia_build_state_status)" || return 1
    phase="$(daia_build_state_phase)" || return 1

    _test_assert_equal \
        "${DAIA_BUILD_STATE_STATUS_RUNNING}" \
        "${status}" \
        "invalid execute arguments should preserve running status" || return 1

    _test_assert_equal \
        "${DAIA_BUILD_STATE_PHASE_INITIALIZING}" \
        "${phase}" \
        "invalid execute arguments should preserve initializing phase" || return 1

    _test_assert_equal \
        "" \
        "$(_test_phase_call_sequence)" \
        "invalid execute arguments should not execute callbacks" || return 1

    return 0
}


test_run_rejects_incorrect_argument_count() {
    local exit_status=0

    daia_builder_run \
        test_workspace_success \
        test_plugins_success \
        test_image_success \
        >/dev/null 2>&1
    exit_status=$?

    _test_assert_failure \
        "${exit_status}" \
        "run should reject an incorrect argument count" || return 1

    _test_assert_state_not_initialized \
        "invalid run arguments should not initialize build state" || return 1

    _test_assert_equal \
        "" \
        "$(_test_phase_call_sequence)" \
        "invalid run arguments should not execute callbacks" || return 1

    return 0
}


test_finalize_rejects_incorrect_argument_count() {
    local exit_status=0
    local status=""
    local phase=""

    daia_builder_initialize >/dev/null 2>&1 || return 1

    daia_builder_finalize \
        >/dev/null 2>&1
    exit_status=$?

    _test_assert_failure \
        "${exit_status}" \
        "finalize should reject an incorrect argument count" || return 1

    status="$(daia_build_state_status)" || return 1
    phase="$(daia_build_state_phase)" || return 1

    _test_assert_equal \
        "${DAIA_BUILD_STATE_STATUS_RUNNING}" \
        "${status}" \
        "invalid finalize arguments should preserve running status" || return 1

    _test_assert_equal \
        "${DAIA_BUILD_STATE_PHASE_INITIALIZING}" \
        "${phase}" \
        "invalid finalize arguments should preserve initializing phase" || return 1

    _test_assert_equal \
        "" \
        "$(_test_phase_call_sequence)" \
        "invalid finalize arguments should not execute cleanup" || return 1

    return 0
}

test_finalize_requires_initialization() {
    local exit_status=0

    daia_builder_finalize \
        test_cleanup_success \
        >/dev/null 2>&1
    exit_status=$?

    _test_assert_failure \
        "${exit_status}" \
        "finalize should reject uninitialized state" || return 1

    _test_assert_state_not_initialized \
        "failed finalize call should not initialize state" || return 1

    return 0
}


test_finalize_rejects_missing_cleanup_callback() {
    local exit_status=0
    local status=""
    local phase=""

    daia_builder_initialize >/dev/null 2>&1 || return 1

    daia_builder_finalize \
        test_cleanup_missing \
        >/dev/null 2>&1
    exit_status=$?

    _test_assert_failure \
        "${exit_status}" \
        "finalize should reject a missing cleanup callback" || return 1

    status="$(daia_build_state_status)" || return 1
    phase="$(daia_build_state_phase)" || return 1

    _test_assert_equal \
        "${DAIA_BUILD_STATE_STATUS_RUNNING}" \
        "${status}" \
        "missing cleanup callback should preserve running status" || return 1

    _test_assert_equal \
        "${DAIA_BUILD_STATE_PHASE_INITIALIZING}" \
        "${phase}" \
        "missing cleanup callback should preserve initializing phase" || return 1

    _test_assert_equal \
        "" \
        "$(_test_phase_call_sequence)" \
        "missing cleanup callback should not execute cleanup" || return 1

    return 0
}


test_run_records_missing_callback_failure() {
    local exit_status=0
    local status=""
    local phase=""

    daia_builder_run \
        test_workspace_success \
        test_plugins_missing \
        test_image_success \
        test_cleanup_success \
        >/dev/null 2>&1
    exit_status=$?

    _test_assert_failure \
        "${exit_status}" \
        "run should fail when a callback is missing" || return 1

    status="$(daia_build_state_status)" || return 1
    phase="$(daia_build_state_phase)" || return 1

    _test_assert_equal \
        "${DAIA_BUILD_STATE_STATUS_FAILED}" \
        "${status}" \
        "missing callback should mark the build failed" || return 1
    _test_assert_equal \
        "${DAIA_BUILD_STATE_PHASE_INITIALIZING}" \
        "${phase}" \
        "missing callback should preserve the initializing phase" || return 1

    _test_assert_equal \
        "" \
        "$(_test_phase_call_sequence)" \
        "callback validation should occur before any callback executes" || return 1

    return 0
}



test_workspace_failure_records_workspace_phase() {
    local exit_status=0
    local status=""
    local phase=""

    daia_builder_run \
        test_workspace_failure \
        test_plugins_success \
        test_image_success \
        test_cleanup_success \
        >/dev/null 2>&1
    exit_status=$?

    _test_assert_failure \
        "${exit_status}" \
        "workspace failure should fail the build" || return 1

    status="$(daia_build_state_status)" || return 1
    phase="$(daia_build_state_phase)" || return 1

    _test_assert_equal \
        "${DAIA_BUILD_STATE_STATUS_FAILED}" \
        "${status}" \
        "workspace failure should mark the build failed" || return 1

    _test_assert_equal \
        "${DAIA_BUILD_STATE_PHASE_WORKSPACE}" \
        "${phase}" \
        "workspace failure should preserve the workspace phase" || return 1

    _test_assert_equal \
        "workspace" \
        "$(_test_phase_call_sequence)" \
        "workspace failure should prevent later callbacks" || return 1

    return 0
}
###############################################################################
# Bootstrap
###############################################################################

if [[ ! -r "${BUILD_STATE_SCRIPT}" ]]; then
    _test_error "required file is not readable: ${BUILD_STATE_SCRIPT}"
    exit 1
fi

if [[ ! -r "${BUILDER_SCRIPT}" ]]; then
    _test_error "required file is not readable: ${BUILDER_SCRIPT}"
    exit 1
fi

# The path is computed by this test harness and validated as readable above.
# shellcheck disable=SC1090,SC1091
source "${BUILD_STATE_SCRIPT}"

# The path is computed by this test harness and validated as readable above.
# shellcheck disable=SC1090,SC1091
source "${BUILDER_SCRIPT}"



###############################################################################
# Test execution
###############################################################################

printf 'DAIA Builder Tests\n'
printf '==================\n'

_test_run \
    "successful build executes the complete lifecycle" \
    test_successful_build

_test_run \
    "workspace failure stops the build" \
    test_workspace_failure_stops_build

_test_run \
    "plugin failure prevents image execution" \
    test_plugin_failure_stops_image

_test_run \
    "image failure prevents cleanup execution" \
    test_image_failure_stops_cleanup

_test_run \
    "cleanup failure marks the build failed" \
    test_cleanup_failure_marks_build_failed

_test_run \
    "duplicate initialization is rejected" \
    test_duplicate_initialization_is_rejected

_test_run \
    "missing callback is rejected before execution" \
    test_missing_callback_is_rejected

_test_run \
    "execute requires initialized state" \
    test_execute_requires_initialization

_test_run \
    "execute rejects an incorrect argument count" \
    test_execute_rejects_incorrect_argument_count

_test_run \
    "finalize requires initialized state" \
    test_finalize_requires_initialization

_test_run \
    "run rejects an incorrect argument count" \
    test_run_rejects_incorrect_argument_count

_test_run \
    "finalize rejects an incorrect argument count" \
    test_finalize_rejects_incorrect_argument_count

_test_run \
    "finalize rejects a missing cleanup callback" \
    test_finalize_rejects_missing_cleanup_callback

_test_run \
    "run records a missing callback failure" \
    test_run_records_missing_callback_failure

_test_run \
    "workspace failure records the workspace phase" \
    test_workspace_failure_records_workspace_phase



printf '\n'
printf 'Tests run:    %d\n' "${TESTS_RUN}"
printf 'Tests passed: %d\n' "${TESTS_PASSED}"
printf 'Tests failed: %d\n' "${TESTS_FAILED}"

if ((TESTS_FAILED != 0)); then
    exit 1
fi


exit 0
