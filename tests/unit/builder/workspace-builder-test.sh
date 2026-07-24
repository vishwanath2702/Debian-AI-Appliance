#!/usr/bin/env bash
#
# Unit tests for the DAIA Workspace Builder.
#

set -u
set -o pipefail

TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_DIR

REPOSITORY_ROOT="$(cd -- "$TEST_DIR/../../.." && pwd)"
readonly REPOSITORY_ROOT

WORKSPACE_BUILDER_FILE="$REPOSITORY_ROOT/installer/files/opt/daia/builder/workspace-builder.sh"
readonly WORKSPACE_BUILDER_FILE

TESTS_RUN=0
TESTS_FAILED=0

TEST_WORKSPACE=""
TEST_PARENT=""
TEST_WORKSPACE_PATH=""

fail() {
    printf 'FAIL: %s\n' "$*" >&2
    return 1
}

assert_success() {
    local description="$1"
    shift

    TESTS_RUN=$((TESTS_RUN + 1))

    if "$@"; then
        printf 'PASS: %s\n' "$description"
        return 0
    fi

    TESTS_FAILED=$((TESTS_FAILED + 1))
    printf 'FAIL: %s\n' "$description" >&2
    return 1
}

assert_failure() {
    local description="$1"
    shift

    TESTS_RUN=$((TESTS_RUN + 1))

    if "$@"; then
        TESTS_FAILED=$((TESTS_FAILED + 1))
        printf 'FAIL: %s\n' "$description" >&2
        return 1
    fi

    printf 'PASS: %s\n' "$description"
}

assert_equals() {
    local description="$1"
    local expected="$2"
    local actual="$3"

    TESTS_RUN=$((TESTS_RUN + 1))

    if [[ "$expected" == "$actual" ]]; then
        printf 'PASS: %s\n' "$description"
        return 0
    fi

    TESTS_FAILED=$((TESTS_FAILED + 1))

    printf 'FAIL: %s\n' "$description" >&2
    printf '  expected: %s\n' "$expected" >&2
    printf '  actual:   %s\n' "$actual" >&2

    return 1
}

create_test_workspace() {
    TEST_PARENT="$(mktemp -d)" || return 1

    TEST_WORKSPACE_PATH="$TEST_PARENT/workspace"
}

destroy_test_workspace() {
    if [[ -n "$TEST_PARENT" && -d "$TEST_PARENT" ]]; then
        rm -rf -- "$TEST_PARENT"
    fi

    TEST_PARENT=""
    TEST_WORKSPACE_PATH=""
}

reset_test_environment() {
    daia_workspace_builder_clear >/dev/null 2>&1 || true

    destroy_test_workspace
    create_test_workspace
}

workspace_directory_exists() {
    [[ -d "$TEST_WORKSPACE_PATH" ]]
}

workspace_marker_exists() {
    [[ -f "$TEST_WORKSPACE_PATH/.daia-workspace" ]]
}

workspace_subdirectory_exists() {
    local directory="$1"

    [[ -d "$TEST_WORKSPACE_PATH/$directory" ]]
}

test_operations_require_initialization() {
    daia_workspace_builder_clear

    ! daia_workspace_builder_create >/dev/null 2>&1 &&
        ! daia_workspace_builder_destroy >/dev/null 2>&1 &&
        ! daia_workspace_builder_exists >/dev/null 2>&1 &&
        ! daia_workspace_builder_root >/dev/null 2>&1 &&
        ! daia_workspace_builder_work >/dev/null 2>&1 &&
        ! daia_workspace_builder_cache >/dev/null 2>&1 &&
        ! daia_workspace_builder_logs >/dev/null 2>&1 &&
        ! daia_workspace_builder_plan >/dev/null 2>&1 &&
        ! daia_workspace_builder_state >/dev/null 2>&1 &&
        ! daia_workspace_builder_tmp >/dev/null 2>&1
}

test_initialization_succeeds() {
    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1

    [[ "$__DAIA_WORKSPACE_INITIALIZED" -eq 1 ]] &&
        [[ "$__DAIA_WORKSPACE" == "$TEST_WORKSPACE_PATH" ]]
}

test_empty_workspace_rejected() {
    daia_workspace_builder_clear

    ! daia_workspace_builder_init "" >/dev/null 2>&1
}

test_root_workspace_rejected() {
    daia_workspace_builder_clear

    ! daia_workspace_builder_init "/" >/dev/null 2>&1
}

test_double_initialization_rejected() {
    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1

    ! daia_workspace_builder_init "$TEST_WORKSPACE_PATH" \
        >/dev/null 2>&1
}

test_workspace_path_is_normalized() {
    local expected

    reset_test_environment || return 1

    mkdir -p "$TEST_PARENT/subdir" || return 1

    expected="$(cd "$TEST_PARENT" && pwd -P)/workspace"

    daia_workspace_builder_init \
        "$TEST_PARENT/subdir/../workspace" || return 1

    [[ "$__DAIA_WORKSPACE" == "$expected" ]]
}

test_workspace_exists_before_creation() {
    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1

    ! daia_workspace_builder_exists >/dev/null 2>&1
}

test_workspace_creation_succeeds() {
    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1

    daia_workspace_builder_create >/dev/null || return 1

    workspace_directory_exists
}

test_workspace_exists_after_creation() {
    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1
    daia_workspace_builder_create >/dev/null || return 1

    daia_workspace_builder_exists
}

test_workspace_creation_rejected_when_exists() {
    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1
    daia_workspace_builder_create >/dev/null || return 1

    ! daia_workspace_builder_create >/dev/null 2>&1
}

test_required_directories_created() {
    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1
    daia_workspace_builder_create >/dev/null || return 1

    workspace_subdirectory_exists cache &&
        workspace_subdirectory_exists logs &&
        workspace_subdirectory_exists plan &&
        workspace_subdirectory_exists root &&
        workspace_subdirectory_exists state &&
        workspace_subdirectory_exists tmp &&
        workspace_subdirectory_exists work
}

test_workspace_marker_created() {
    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1
    daia_workspace_builder_create >/dev/null || return 1

    workspace_marker_exists
}


test_root_getter() {
    local expected

    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1

    expected="$TEST_WORKSPACE_PATH/root"

    [[ "$(daia_workspace_builder_root)" == "$expected" ]]
}

test_work_getter() {
    local expected

    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1

    expected="$TEST_WORKSPACE_PATH/work"

    [[ "$(daia_workspace_builder_work)" == "$expected" ]]
}

test_cache_getter() {
    local expected

    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1

    expected="$TEST_WORKSPACE_PATH/cache"

    [[ "$(daia_workspace_builder_cache)" == "$expected" ]]
}

test_logs_getter() {
    local expected

    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1

    expected="$TEST_WORKSPACE_PATH/logs"

    [[ "$(daia_workspace_builder_logs)" == "$expected" ]]
}

test_plan_getter() {
    local expected

    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1

    expected="$TEST_WORKSPACE_PATH/plan"

    [[ "$(daia_workspace_builder_plan)" == "$expected" ]]
}

test_state_getter() {
    local expected

    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1

    expected="$TEST_WORKSPACE_PATH/state"

    [[ "$(daia_workspace_builder_state)" == "$expected" ]]
}

test_tmp_getter() {
    local expected

    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1

    expected="$TEST_WORKSPACE_PATH/tmp"

    [[ "$(daia_workspace_builder_tmp)" == "$expected" ]]
}


test_destroy_succeeds() {
    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1
    daia_workspace_builder_create >/dev/null || return 1

    daia_workspace_builder_destroy || return 1

    [[ ! -d "$TEST_WORKSPACE_PATH" ]]
}

test_destroy_before_create_rejected() {
    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1

    ! daia_workspace_builder_destroy >/dev/null 2>&1
}

test_destroy_twice_rejected() {
    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1
    daia_workspace_builder_create >/dev/null || return 1

    daia_workspace_builder_destroy || return 1

    ! daia_workspace_builder_destroy >/dev/null 2>&1
}

test_missing_marker_is_rejected() {
    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1
    daia_workspace_builder_create >/dev/null || return 1

    rm -f "$TEST_WORKSPACE_PATH/.daia-workspace"

    ! daia_workspace_builder_destroy >/dev/null 2>&1
}

test_missing_required_directory_is_rejected() {
    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1
    daia_workspace_builder_create >/dev/null || return 1

    rm -rf "$TEST_WORKSPACE_PATH/cache"

    ! daia_workspace_builder_exists >/dev/null 2>&1
}

test_nonexistent_workspace_is_rejected() {
    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1

    mkdir -p "$TEST_WORKSPACE_PATH" || return 1
    rm -rf "$TEST_WORKSPACE_PATH"

    ! daia_workspace_builder_destroy >/dev/null 2>&1
}

test_clear_resets_builder() {
    reset_test_environment || return 1

    daia_workspace_builder_init "$TEST_WORKSPACE_PATH" || return 1

    daia_workspace_builder_clear

    [[ "$__DAIA_WORKSPACE_INITIALIZED" -eq 0 ]] &&
        [[ -z "$__DAIA_WORKSPACE" ]]
}

main() {
    if [[ ! -r "$WORKSPACE_BUILDER_FILE" ]]; then
        fail "workspace builder file not found: $WORKSPACE_BUILDER_FILE"
        exit 1
    fi

    # shellcheck source=/dev/null
    source "$WORKSPACE_BUILDER_FILE"

    trap destroy_test_workspace EXIT

    assert_success \
        "operations require initialization" \
        test_operations_require_initialization || true

    assert_success \
        "initialization succeeds" \
        test_initialization_succeeds || true

    assert_success \
        "empty workspace rejected" \
        test_empty_workspace_rejected || true

    assert_success \
        "root workspace rejected" \
        test_root_workspace_rejected || true

    assert_success \
        "double initialization rejected" \
        test_double_initialization_rejected || true

    assert_success \
        "workspace path normalized" \
        test_workspace_path_is_normalized || true

    assert_success \
        "workspace absent before creation" \
        test_workspace_exists_before_creation || true

    assert_success \
        "workspace creation succeeds" \
        test_workspace_creation_succeeds || true

    assert_success \
        "workspace exists after creation" \
        test_workspace_exists_after_creation || true

    assert_success \
        "workspace creation rejected when exists" \
        test_workspace_creation_rejected_when_exists || true

    assert_success \
        "required directories created" \
        test_required_directories_created || true

    assert_success \
        "workspace marker created" \
        test_workspace_marker_created || true

    assert_success \
        "root getter" \
        test_root_getter || true

    assert_success \
        "work getter" \
        test_work_getter || true

    assert_success \
        "cache getter" \
        test_cache_getter || true

    assert_success \
        "logs getter" \
        test_logs_getter || true

    assert_success \
        "plan getter" \
        test_plan_getter || true

    assert_success \
        "state getter" \
        test_state_getter || true

    assert_success \
        "tmp getter" \
        test_tmp_getter || true

    assert_success \
        "destroy succeeds" \
        test_destroy_succeeds || true

    assert_success \
        "destroy before create rejected" \
        test_destroy_before_create_rejected || true

    assert_success \
        "destroy twice rejected" \
        test_destroy_twice_rejected || true

    assert_success \
        "missing marker rejected" \
        test_missing_marker_is_rejected || true

    assert_success \
        "missing required directory rejected" \
        test_missing_required_directory_is_rejected || true

    assert_success \
        "nonexistent workspace rejected" \
        test_nonexistent_workspace_is_rejected || true

    assert_success \
        "clear resets builder" \
        test_clear_resets_builder || true

    destroy_test_workspace

    printf '\nTests run: %d\n' "$TESTS_RUN"
    printf 'Failures:  %d\n' "$TESTS_FAILED"

    [[ "$TESTS_FAILED" -eq 0 ]]
}

main "$@"
