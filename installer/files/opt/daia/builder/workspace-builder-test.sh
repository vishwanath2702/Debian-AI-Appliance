#!/usr/bin/env bash

# shellcheck disable=SC2317

# DAIA Workspace Builder Tests
#
# Tests the Workspace Builder public API.
#
# Expected files in the same directory:
#   workspace-builder.sh

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

readonly WORKSPACE_BUILDER_SCRIPT="${TEST_SCRIPT_DIR}/workspace-builder.sh"

declare -gi TESTS_RUN=0
declare -gi TESTS_PASSED=0
declare -gi TESTS_FAILED=0

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
    local expected="${1:-}"
    local actual="${2:-}"
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

_test_prepare() {
    return 0
}

_test_finish() {
    return 0
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
# Bootstrap
###############################################################################

if [[ ! -r "${WORKSPACE_BUILDER_SCRIPT}" ]]; then
    _test_error "required file is not readable: ${WORKSPACE_BUILDER_SCRIPT}"
    exit 1
fi

# The path is computed by this test harness and validated as readable above.
# shellcheck disable=SC1090,SC1091
source "${WORKSPACE_BUILDER_SCRIPT}"

###############################################################################
# Test helpers
###############################################################################
_test_create_directory() {
    local directory

    directory="$(mktemp -d)" ||
        _test_error "failed to create temporary test directory"

    printf '%s\n' "${directory}"
}

_test_remove_directory() {
    local directory="${1-}"

    [[ -n "${directory}" ]] || return 0

    rm -rf -- "${directory}"
}

###############################################################################
# Tests
###############################################################################
test_clear_workspace() {
    local test_directory
    local workspace
    local exit_status=0

    test_directory="$(_test_create_directory)" || return 1
    workspace="${test_directory}/workspace"

    daia_workspace_builder_init "${workspace}" || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder initialization should succeed" || {
        _test_remove_directory "${test_directory}"
        return 1
    }

    daia_workspace_builder_clear || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder clear should succeed" || {
        _test_remove_directory "${test_directory}"
        return 1
    }

    if daia_workspace_builder_root >/dev/null 2>&1; then
        _test_error "workspace builder state should be cleared"
        _test_remove_directory "${test_directory}"
        return 1
    fi

    _test_remove_directory "${test_directory}"

    return 0
}

test_workspace_exists_after_create() {
    local test_directory
    local workspace
    local exit_status=0

    test_directory="$(_test_create_directory)" || return 1
    workspace="${test_directory}/workspace"

    daia_workspace_builder_init "${workspace}" || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder initialization should succeed" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    exit_status=0
    daia_workspace_builder_create >/dev/null || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder create should succeed" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    if ! daia_workspace_builder_exists; then
        _test_error "created workspace should exist"
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    fi

    daia_workspace_builder_clear >/dev/null 2>&1 || true
    _test_remove_directory "${test_directory}"

    return 0
}


test_workspace_does_not_exist_before_create() {
    local test_directory
    local workspace
    local exit_status=0

    test_directory="$(_test_create_directory)" || return 1
    workspace="${test_directory}/workspace"

    daia_workspace_builder_init "${workspace}" || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder initialization should succeed" || {
        _test_remove_directory "${test_directory}"
        return 1
    }

    if daia_workspace_builder_exists; then
        _test_error "uncreated workspace should not exist"
        daia_workspace_builder_clear
        _test_remove_directory "${test_directory}"
        return 1
    fi

    daia_workspace_builder_clear
    _test_remove_directory "${test_directory}"

    return 0
}

test_destroyed_workspace_does_not_exist() {
    local test_directory
    local workspace
    local exit_status=0

    test_directory="$(_test_create_directory)" || return 1
    workspace="${test_directory}/workspace"

    daia_workspace_builder_init "${workspace}" || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder initialization should succeed" || {
        _test_remove_directory "${test_directory}"
        return 1
    }

    exit_status=0
    daia_workspace_builder_create >/dev/null || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder create should succeed" || {
        daia_workspace_builder_clear
        _test_remove_directory "${test_directory}"
        return 1
    }

    exit_status=0
    daia_workspace_builder_destroy || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder destroy should succeed" || {
        daia_workspace_builder_clear
        _test_remove_directory "${test_directory}"
        return 1
    }

    if [[ -e "${workspace}" ]]; then
        _test_error "destroyed workspace directory should not exist"
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    fi

    if daia_workspace_builder_root >/dev/null 2>&1; then
        _test_error "workspace builder state should be cleared after destroy"
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    fi

    daia_workspace_builder_clear >/dev/null 2>&1 || true
    _test_remove_directory "${test_directory}"

    return 0
}


test_workspace_create_creates_required_directories() {
    local test_directory
    local workspace
    local exit_status=0

    test_directory="$(_test_create_directory)" || return 1
    workspace="${test_directory}/workspace"

    daia_workspace_builder_init "${workspace}" || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder initialization should succeed" || {
        _test_remove_directory "${test_directory}"
        return 1
    }

    exit_status=0
    daia_workspace_builder_create >/dev/null || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder create should succeed" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    for directory in root artifacts logs tmp; do
        if [[ ! -d "${workspace}/${directory}" ]]; then
            _test_error "missing required directory: ${directory}"
            daia_workspace_builder_clear >/dev/null 2>&1 || true
            _test_remove_directory "${test_directory}"
            return 1
        fi
    done

    daia_workspace_builder_clear >/dev/null 2>&1 || true
    _test_remove_directory "${test_directory}"

    return 0
}


test_workspace_create_creates_required_directories() {
    local test_directory
    local workspace
    local exit_status=0
    local directory_name

    test_directory="$(_test_create_directory)" || return 1
    workspace="${test_directory}/workspace"

    daia_workspace_builder_init "${workspace}" || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder initialization should succeed" || {
        _test_remove_directory "${test_directory}"
        return 1
    }

    exit_status=0
    daia_workspace_builder_create >/dev/null || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder create should succeed" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    for directory_name in \
        cache \
        logs \
        plan \
        root \
        state \
        tmp \
        work
    do
        if [[ ! -d "${workspace}/${directory_name}" ]]; then
            _test_error \
                "required workspace directory should exist: ${directory_name}"
            daia_workspace_builder_clear >/dev/null 2>&1 || true
            _test_remove_directory "${test_directory}"
            return 1
        fi
    done

    daia_workspace_builder_clear >/dev/null 2>&1 || true
    _test_remove_directory "${test_directory}"

    return 0
}

test_workspace_create_writes_marker() {
    local test_directory
    local workspace
    local marker
    local marker_contents
    local exit_status=0

    test_directory="$(_test_create_directory)" || return 1
    workspace="${test_directory}/workspace"

    daia_workspace_builder_init "${workspace}" || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder initialization should succeed" || {
        _test_remove_directory "${test_directory}"
        return 1
    }

    exit_status=0
    daia_workspace_builder_create >/dev/null || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder create should succeed" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    marker="$(_daia_workspace_builder_marker)"

    if [[ ! -f "${marker}" ]]; then
        _test_error "workspace ownership marker should exist"
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    fi

    marker_contents="$(<"${marker}")"

    _test_assert_equal \
        "${workspace}" \
        "${marker_contents}" \
        "workspace ownership marker should contain the workspace path" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    daia_workspace_builder_clear >/dev/null 2>&1 || true
    _test_remove_directory "${test_directory}"

    return 0
}

test_workspace_create_fails_if_workspace_exists() {
    local test_directory
    local workspace
    local exit_status=0

    test_directory="$(_test_create_directory)" || return 1
    workspace="${test_directory}/workspace"

    mkdir -- "${workspace}" || {
        _test_remove_directory "${test_directory}"
        return 1
    }

    daia_workspace_builder_init "${workspace}" || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder initialization should succeed" || {
        _test_remove_directory "${test_directory}"
        return 1
    }

    exit_status=0
    daia_workspace_builder_create >/dev/null 2>&1 || exit_status=$?

    if [[ ${exit_status} -eq 0 ]]; then
        _test_error \
            "workspace builder create should fail when the workspace already exists"
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    fi

    if [[ ! -d "${workspace}" ]]; then
        _test_error \
            "existing workspace directory should not be removed"
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    fi

    daia_workspace_builder_clear >/dev/null 2>&1 || true
    _test_remove_directory "${test_directory}"

    return 0
}

test_workspace_create_requires_initialization() {
    local test_directory
    local workspace
    local exit_status=0

    test_directory="$(_test_create_directory)" || return 1
    workspace="${test_directory}/workspace"

    exit_status=0
    daia_workspace_builder_create >/dev/null 2>&1 || exit_status=$?

    if [[ ${exit_status} -eq 0 ]]; then
        _test_error \
            "workspace builder create should fail before initialization"
        _test_remove_directory "${test_directory}"
        return 1
    fi

    if [[ -e "${workspace}" ]]; then
        _test_error \
            "workspace should not be created before initialization"
        _test_remove_directory "${test_directory}"
        return 1
    fi

    _test_remove_directory "${test_directory}"

    return 0
}
test_workspace_exists_requires_initialization() {
    local exit_status=0

    exit_status=0
    daia_workspace_builder_exists >/dev/null 2>&1 || exit_status=$?

    if [[ ${exit_status} -eq 0 ]]; then
        _test_error \
            "workspace builder exists should fail before initialization"
        return 1
    fi

    return 0
}

test_workspace_destroy_requires_initialization() {
    local exit_status=0

    daia_workspace_builder_destroy >/dev/null 2>&1 || exit_status=$?

    if [[ ${exit_status} -eq 0 ]]; then
        _test_error \
            "workspace builder destroy should fail before initialization"
        return 1
    fi

    return 0
}

test_workspace_destroy_fails_before_create() {
    local test_directory
    local workspace
    local exit_status=0

    test_directory="$(_test_create_directory)" || return 1
    workspace="${test_directory}/workspace"

    daia_workspace_builder_init "${workspace}" || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder initialization should succeed" || {
        _test_remove_directory "${test_directory}"
        return 1
    }

    exit_status=0
    daia_workspace_builder_destroy >/dev/null 2>&1 || exit_status=$?

    if [[ ${exit_status} -eq 0 ]]; then
        _test_error \
            "workspace builder destroy should fail before workspace creation"
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    fi

    if [[ -e "${workspace}" ]]; then
        _test_error \
            "workspace should not exist before creation"
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    fi

    daia_workspace_builder_clear >/dev/null 2>&1 || true
    _test_remove_directory "${test_directory}"

    return 0
}

test_workspace_destroy_fails_without_marker() {
    local test_directory
    local workspace
    local marker
    local exit_status=0

    test_directory="$(_test_create_directory)" || return 1
    workspace="${test_directory}/workspace"

    daia_workspace_builder_init "${workspace}" || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder initialization should succeed" || {
        _test_remove_directory "${test_directory}"
        return 1
    }

    exit_status=0
    daia_workspace_builder_create >/dev/null || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder create should succeed" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    marker="$(_daia_workspace_builder_marker)"

    rm -- "${marker}" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    exit_status=0
    daia_workspace_builder_destroy >/dev/null 2>&1 || exit_status=$?

    if [[ ${exit_status} -eq 0 ]]; then
        _test_error \
            "workspace builder destroy should fail without an ownership marker"
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    fi

    if [[ ! -d "${workspace}" ]]; then
        _test_error \
            "workspace without an ownership marker should not be removed"
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    fi

    daia_workspace_builder_clear >/dev/null 2>&1 || true
    _test_remove_directory "${test_directory}"

    return 0
}

test_workspace_destroy_fails_with_invalid_marker() {
    local test_directory
    local workspace
    local marker
    local exit_status=0

    test_directory="$(_test_create_directory)" || return 1
    workspace="${test_directory}/workspace"

    daia_workspace_builder_init "${workspace}" || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder initialization should succeed" || {
        _test_remove_directory "${test_directory}"
        return 1
    }

    exit_status=0
    daia_workspace_builder_create >/dev/null || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder create should succeed" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    marker="$(_daia_workspace_builder_marker)"

    printf '%s\n' "${test_directory}/different-workspace" >"${marker}" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    exit_status=0
    daia_workspace_builder_destroy >/dev/null 2>&1 || exit_status=$?

    if [[ ${exit_status} -eq 0 ]]; then
        _test_error \
            "workspace builder destroy should fail with an invalid ownership marker"
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    fi

    if [[ ! -d "${workspace}" ]]; then
        _test_error \
            "workspace with an invalid ownership marker should not be removed"
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    fi

    daia_workspace_builder_clear >/dev/null 2>&1 || true
    _test_remove_directory "${test_directory}"

    return 0
}


test_workspace_init_fails_when_already_initialized() {
    local first_test_directory
    local second_test_directory
    local first_workspace
    local second_workspace
    local exit_status=0

    first_test_directory="$(_test_create_directory)" || return 1
    second_test_directory="$(_test_create_directory)" || {
        _test_remove_directory "${first_test_directory}"
        return 1
    }

    first_workspace="${first_test_directory}/workspace"
    second_workspace="${second_test_directory}/workspace"

    daia_workspace_builder_init "${first_workspace}" || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "first workspace builder initialization should succeed" || {
        _test_remove_directory "${first_test_directory}"
        _test_remove_directory "${second_test_directory}"
        return 1
    }

    exit_status=0
    daia_workspace_builder_init "${second_workspace}" \
        >/dev/null 2>&1 || exit_status=$?

    if [[ ${exit_status} -eq 0 ]]; then
        _test_error \
            "workspace builder initialization should fail when already initialized"
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${first_test_directory}"
        _test_remove_directory "${second_test_directory}"
        return 1
    fi

    _test_assert_equal \
        "${first_workspace}/root" \
        "$(daia_workspace_builder_root)" \
        "failed reinitialization should preserve the original workspace" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${first_test_directory}"
        _test_remove_directory "${second_test_directory}"
        return 1
    }

    daia_workspace_builder_clear >/dev/null 2>&1 || true
    _test_remove_directory "${first_test_directory}"
    _test_remove_directory "${second_test_directory}"

    return 0
}

test_workspace_can_reinitialize_after_clear() {
    local first_test_directory
    local second_test_directory
    local first_workspace
    local second_workspace
    local exit_status=0

    first_test_directory="$(_test_create_directory)" || return 1
    second_test_directory="$(_test_create_directory)" || {
        _test_remove_directory "${first_test_directory}"
        return 1
    }

    first_workspace="${first_test_directory}/workspace"
    second_workspace="${second_test_directory}/workspace"

    daia_workspace_builder_init "${first_workspace}" || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "first workspace builder initialization should succeed" || {
        _test_remove_directory "${first_test_directory}"
        _test_remove_directory "${second_test_directory}"
        return 1
    }

    exit_status=0
    daia_workspace_builder_clear || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder clear should succeed" || {
        _test_remove_directory "${first_test_directory}"
        _test_remove_directory "${second_test_directory}"
        return 1
    }

    exit_status=0
    daia_workspace_builder_init "${second_workspace}" || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder should initialize again after clear" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${first_test_directory}"
        _test_remove_directory "${second_test_directory}"
        return 1
    }

    _test_assert_equal \
        "${second_workspace}/root" \
        "$(daia_workspace_builder_root)" \
        "reinitialized workspace root should use the second workspace" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${first_test_directory}"
        _test_remove_directory "${second_test_directory}"
        return 1
    }

    daia_workspace_builder_clear >/dev/null 2>&1 || true
    _test_remove_directory "${first_test_directory}"
    _test_remove_directory "${second_test_directory}"

    return 0
}

test_workspace_root_requires_initialization() {
    local exit_status=0

    daia_workspace_builder_root >/dev/null 2>&1 || exit_status=$?

    if [[ ${exit_status} -eq 0 ]]; then
        _test_error \
            "workspace builder root should fail before initialization"
        return 1
    fi

    return 0
}

test_workspace_clear_before_initialization_succeeds() {
    local exit_status=0

    daia_workspace_builder_clear >/dev/null 2>&1 || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder clear should succeed before initialization"
}


test_workspace_exists_fails_without_marker() {
    local test_directory
    local workspace
    local marker
    local exit_status=0

    test_directory="$(_test_create_directory)" || return 1
    workspace="${test_directory}/workspace"

    daia_workspace_builder_init "${workspace}" || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder initialization should succeed" || {
        _test_remove_directory "${test_directory}"
        return 1
    }

    exit_status=0
    daia_workspace_builder_create >/dev/null || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder create should succeed" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    marker="$(_daia_workspace_builder_marker)"
    rm -- "${marker}" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    if daia_workspace_builder_exists; then
        _test_error \
            "workspace without an ownership marker should not be recognized as existing"
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    fi

    daia_workspace_builder_clear >/dev/null 2>&1 || true
    _test_remove_directory "${test_directory}"

    return 0
}

test_workspace_exists_fails_with_invalid_marker() {
    local test_directory
    local workspace
    local marker
    local exit_status=0

    test_directory="$(_test_create_directory)" || return 1
    workspace="${test_directory}/workspace"

    daia_workspace_builder_init "${workspace}" || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder initialization should succeed" || {
        _test_remove_directory "${test_directory}"
        return 1
    }

    exit_status=0
    daia_workspace_builder_create >/dev/null || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder create should succeed" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    marker="$(_daia_workspace_builder_marker)"

    printf '%s\n' "${test_directory}/different-workspace" >"${marker}" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    if daia_workspace_builder_exists; then
        _test_error \
            "workspace with an invalid ownership marker should not be recognized as existing"
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    fi

    daia_workspace_builder_clear >/dev/null 2>&1 || true
    _test_remove_directory "${test_directory}"

    return 0
}

test_workspace_exists_fails_when_required_directory_is_missing() {
    local test_directory
    local workspace
    local exit_status=0

    test_directory="$(_test_create_directory)" || return 1
    workspace="${test_directory}/workspace"

    daia_workspace_builder_init "${workspace}" || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder initialization should succeed" || {
        _test_remove_directory "${test_directory}"
        return 1
    }

    exit_status=0
    daia_workspace_builder_create >/dev/null || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder create should succeed" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    rmdir -- "${workspace}/cache" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    if daia_workspace_builder_exists; then
        _test_error \
            "workspace missing a required directory should not be recognized as existing"
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    fi

    daia_workspace_builder_clear >/dev/null 2>&1 || true
    _test_remove_directory "${test_directory}"

    return 0
}


test_workspace_accessors_derive_expected_paths() {
    local test_directory
    local workspace
    local exit_status=0

    test_directory="$(_test_create_directory)" || return 1
    workspace="${test_directory}/workspace"

    daia_workspace_builder_init "${workspace}" || exit_status=$?

    _test_assert_success \
        "${exit_status}" \
        "workspace builder initialization should succeed" || {
        _test_remove_directory "${test_directory}"
        return 1
    }

    _test_assert_equal \
        "${workspace}/root" \
        "$(daia_workspace_builder_root)" \
        "workspace root accessor should derive the root path" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    _test_assert_equal \
        "${workspace}/work" \
        "$(daia_workspace_builder_work)" \
        "workspace work accessor should derive the work path" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    _test_assert_equal \
        "${workspace}/cache" \
        "$(daia_workspace_builder_cache)" \
        "workspace cache accessor should derive the cache path" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    _test_assert_equal \
        "${workspace}/logs" \
        "$(daia_workspace_builder_logs)" \
        "workspace logs accessor should derive the logs path" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    _test_assert_equal \
        "${workspace}/plan" \
        "$(daia_workspace_builder_plan)" \
        "workspace plan accessor should derive the plan path" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    _test_assert_equal \
        "${workspace}/state" \
        "$(daia_workspace_builder_state)" \
        "workspace state accessor should derive the state path" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    _test_assert_equal \
        "${workspace}/tmp" \
        "$(daia_workspace_builder_tmp)" \
        "workspace tmp accessor should derive the tmp path" || {
        daia_workspace_builder_clear >/dev/null 2>&1 || true
        _test_remove_directory "${test_directory}"
        return 1
    }

    daia_workspace_builder_clear >/dev/null 2>&1 || true
    _test_remove_directory "${test_directory}"

    return 0
}

###############################################################################
# Test execution
###############################################################################

main() {
      _test_run \
        "destroyed workspace is not recognized as existing" \
         test_destroyed_workspace_does_not_exist
     _test_run \
        "uncreated workspace is not recognized as existing" \
        test_workspace_does_not_exist_before_create

}
    _test_run \
        "workspace builder clear resets state" \
        test_clear_workspace
    _test_run \
        "created workspace is recognized as existing" \
    test_workspace_exists_after_create
     _test_run \
    "workspace create creates required directories" \
    test_workspace_create_creates_required_directories
     _test_run \
    "workspace create writes ownership marker" \
    test_workspace_create_writes_marker
    _test_run \
    "workspace create fails if workspace exists" \
    test_workspace_create_fails_if_workspace_exists



    _test_run \
    "workspace create requires initialization" \
    test_workspace_create_requires_initialization


    _test_run \
    "workspace exists requires initialization" \
    test_workspace_exists_requires_initialization

    _test_run \
    "workspace destroy requires initialization" \
    test_workspace_destroy_requires_initialization

   _test_run \
    "workspace destroy fails before create" \
    test_workspace_destroy_fails_before_create

  _test_run \
    "workspace destroy fails without ownership marker" \
    test_workspace_destroy_fails_without_marker



    _test_run \
    "workspace destroy fails with invalid ownership marker" \
    test_workspace_destroy_fails_with_invalid_marker

    _test_run \
    "workspace initialization fails when already initialized" \
    test_workspace_init_fails_when_already_initialized

   _test_run \
    "workspace can reinitialize after clear" \
    test_workspace_can_reinitialize_after_clear

    _test_run \
    "workspace root requires initialization" \
    test_workspace_root_requires_initialization


    _test_run \
    "workspace clear succeeds before initialization" \
    test_workspace_clear_before_initialization_succeeds

    _test_run \
    "workspace without ownership marker is not recognized as existing" \
    test_workspace_exists_fails_without_marker

    _test_run \
    "workspace with invalid ownership marker is not recognized as existing" \
    test_workspace_exists_fails_with_invalid_marker

     _test_run \
    "workspace missing required directory is not recognized as existing" \
    test_workspace_exists_fails_when_required_directory_is_missing


   _test_run \
    "workspace accessors derive expected paths" \
    test_workspace_accessors_derive_expected_paths


main "$@"
