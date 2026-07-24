#!/usr/bin/env bash

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

BUILD_CONTEXT="$REPOSITORY_ROOT/installer/files/opt/daia/builder/build-context.sh"

TEST_ROOT=""
TESTS_RUN=0
TESTS_FAILED=0

fail() {
    printf 'FAIL: %s\n' "$1" >&2
    TESTS_FAILED=$((TESTS_FAILED + 1))
}

assert_success() {
    local description="$1"
    shift

    TESTS_RUN=$((TESTS_RUN + 1))

    if ! "$@"; then
        fail "$description"
        return 1
    fi

    return 0
}

assert_failure() {
    local description="$1"
    shift

    TESTS_RUN=$((TESTS_RUN + 1))

    if "$@"; then
        fail "$description"
        return 1
    fi

    return 0
}

assert_equals() {
    local expected="$1"
    local actual="$2"
    local description="$3"

    TESTS_RUN=$((TESTS_RUN + 1))

    if [[ "$expected" != "$actual" ]]; then
        printf 'FAIL: %s\n' "$description" >&2
        printf 'Expected: %s\n' "$expected" >&2
        printf 'Actual:   %s\n' "$actual" >&2
        TESTS_FAILED=$((TESTS_FAILED + 1))
        return 1
    fi

    return 0
}

create_test_environment() {
    TEST_ROOT="$(mktemp -d)"

    mkdir -p \
        "$TEST_ROOT/workspace" \
        "$TEST_ROOT/plugins" \
        "$TEST_ROOT/profiles" \
        "$TEST_ROOT/plans" \
        "$TEST_ROOT/output"
}

destroy_test_environment() {
    if [[ -n "$TEST_ROOT" && -d "$TEST_ROOT" ]]; then
        rm -rf "$TEST_ROOT"
    fi

    TEST_ROOT=""
}

reset_build_context() {
    if daia_build_context_is_initialized 2>/dev/null; then
        daia_build_context_clear >/dev/null 2>&1 || true
    fi
}

setup_test() {
    destroy_test_environment
    create_test_environment
    reset_build_context
}

teardown_test() {
    reset_build_context
    destroy_test_environment
}

test_operations_require_initialization() {
    setup_test

    assert_failure \
        "workspace setter requires initialization" \
        daia_build_context_set_workspace \
        "$TEST_ROOT/workspace"

    assert_failure \
        "getter requires initialization" \
        daia_build_context_workspace

    assert_failure \
        "validate requires initialization" \
        daia_build_context_validate

    assert_failure \
        "clear requires initialization" \
        daia_build_context_clear

    teardown_test
}

test_initialization_succeeds() {
    setup_test

    assert_success \
        "build context initialization succeeds" \
        daia_build_context_init

    assert_success \
        "context reports initialized" \
        daia_build_context_is_initialized

    assert_failure \
        "context is not valid immediately after initialization" \
        daia_build_context_is_valid

    assert_failure \
        "context is not locked immediately after initialization" \
        daia_build_context_is_locked

    teardown_test
}

test_double_initialization_rejected() {
    setup_test

    daia_build_context_init >/dev/null

    assert_failure \
        "double initialization rejected" \
        daia_build_context_init

    teardown_test
}

test_clear_succeeds() {
    setup_test

    daia_build_context_init >/dev/null

    assert_success \
        "clear succeeds" \
        daia_build_context_clear

    assert_failure \
        "context no longer initialized after clear" \
        daia_build_context_is_initialized

    teardown_test
}

test_clear_before_initialization_rejected() {
    setup_test

    assert_failure \
        "clear before initialization rejected" \
        daia_build_context_clear

    teardown_test
}

test_context_initial_state() {
    setup_test

    daia_build_context_init >/dev/null

    assert_failure \
        "context is incomplete after initialization" \
        daia_build_context_is_complete

    assert_failure \
        "context is invalid after initialization" \
        daia_build_context_is_valid

    assert_failure \
        "context is unlocked after initialization" \
        daia_build_context_is_locked

    teardown_test
}


test_workspace_setter_and_getter() {
    setup_test

    daia_build_context_init >/dev/null

    assert_success \
        "workspace setter succeeds" \
        daia_build_context_set_workspace \
        "$TEST_ROOT/workspace"

    assert_equals \
        "$(cd "$TEST_ROOT/workspace" && pwd -P)" \
        "$(daia_build_context_workspace)" \
        "workspace getter returns normalized path"

    assert_failure \
        "workspace cannot be set twice" \
        daia_build_context_set_workspace \
        "$TEST_ROOT/workspace"

    teardown_test
}

test_plugin_root_setter_and_getter() {
    setup_test

    daia_build_context_init >/dev/null

    assert_success \
        "plugin root setter succeeds" \
        daia_build_context_set_plugin_root \
        "$TEST_ROOT/plugins"

    assert_equals \
        "$(cd "$TEST_ROOT/plugins" && pwd -P)" \
        "$(daia_build_context_plugin_root)" \
        "plugin root getter returns normalized path"

    assert_failure \
        "plugin root cannot be set twice" \
        daia_build_context_set_plugin_root \
        "$TEST_ROOT/plugins"

    teardown_test
}

test_profile_setter_and_getter() {
    setup_test

    daia_build_context_init >/dev/null

    assert_success \
        "profile setter succeeds" \
        daia_build_context_set_profile \
        "$TEST_ROOT/profiles/default.yaml"

    assert_equals \
        "$TEST_ROOT/profiles/default.yaml" \
        "$(daia_build_context_profile)" \
        "profile getter returns value"

    assert_failure \
        "profile cannot be set twice" \
        daia_build_context_set_profile \
        another.yaml

    teardown_test
}

test_execution_plan_setter_and_getter() {
    setup_test

    daia_build_context_init >/dev/null

    assert_success \
        "execution plan setter succeeds" \
        daia_build_context_set_execution_plan \
        "$TEST_ROOT/plans/plan.json"

    assert_equals \
        "$TEST_ROOT/plans/plan.json" \
        "$(daia_build_context_execution_plan)" \
        "execution plan getter returns value"

    assert_failure \
        "execution plan cannot be set twice" \
        daia_build_context_set_execution_plan \
        another.json

    teardown_test
}

test_architecture_setter_and_getter() {
    setup_test

    daia_build_context_init >/dev/null

    assert_success \
        "architecture setter succeeds" \
        daia_build_context_set_architecture \
        x86_64

    assert_equals \
        "x86_64" \
        "$(daia_build_context_architecture)" \
        "architecture getter returns value"

    assert_failure \
        "architecture cannot be set twice" \
        daia_build_context_set_architecture \
        aarch64

    teardown_test
}

test_output_directory_setter_and_getter() {
    setup_test

    daia_build_context_init >/dev/null

    assert_success \
        "output directory setter succeeds" \
        daia_build_context_set_output_directory \
        "$TEST_ROOT/output"

    assert_equals \
        "$(cd "$TEST_ROOT/output" && pwd -P)" \
        "$(daia_build_context_output_directory)" \
        "output directory getter returns normalized path"

    assert_failure \
        "output directory cannot be set twice" \
        daia_build_context_set_output_directory \
        "$TEST_ROOT/output"

    teardown_test
}

test_build_name_setter_and_getter() {
    setup_test

    daia_build_context_init >/dev/null

    assert_success \
        "build name setter succeeds" \
        daia_build_context_set_build_name \
        "ubuntu-server"

    assert_equals \
        "ubuntu-server" \
        "$(daia_build_context_build_name)" \
        "build name getter returns value"

    assert_failure \
        "build name cannot be set twice" \
        daia_build_context_set_build_name \
        another-build

    teardown_test
}

test_version_setter_and_getter() {
    setup_test

    daia_build_context_init >/dev/null

    assert_success \
        "version setter succeeds" \
        daia_build_context_set_version \
        "24.04.1"

    assert_equals \
        "24.04.1" \
        "$(daia_build_context_version)" \
        "version getter returns value"

    assert_failure \
        "version cannot be set twice" \
        daia_build_context_set_version \
        "25.04"

    teardown_test
}
test_context_is_complete() {
    initialize_build_context

    daia_build_context_set_workspace \
        "$TEST_ROOT/workspace" >/dev/null
    daia_build_context_set_plugin_root \
        "$TEST_ROOT/plugins" >/dev/null
    daia_build_context_set_profile \
        "$TEST_ROOT/profiles/default.yaml" >/dev/null
    daia_build_context_set_execution_plan \
        "$TEST_ROOT/plans/default.plan" >/dev/null
    daia_build_context_set_architecture \
        x86_64 >/dev/null
    daia_build_context_set_output_directory \
        "$TEST_ROOT/output" >/dev/null
    daia_build_context_set_build_name \
        ubuntu-server >/dev/null
    daia_build_context_set_version \
        24.04 >/dev/null

    assert_success \
        "context is complete" \
        daia_build_context_is_complete

    teardown_test
}

test_validation_rejects_incomplete_context() {
    initialize_build_context

    daia_build_context_set_workspace \
        "$TEST_ROOT/workspace" >/dev/null

    assert_failure \
        "validation rejects incomplete context" \
        daia_build_context_validate

    teardown_test
}

test_invalid_architecture_rejected() {
    initialize_build_context

    assert_failure \
        "invalid architecture rejected" \
        daia_build_context_set_architecture \
        riscv128

    teardown_test
}

test_empty_build_name_rejected() {
    initialize_build_context

    assert_failure \
        "empty build name rejected" \
        daia_build_context_set_build_name \
        ""

    teardown_test
}

test_empty_version_rejected() {
    initialize_build_context

    assert_failure \
        "empty version rejected" \
        daia_build_context_set_version \
        ""

    teardown_test
}

test_successful_validation() {
    initialize_build_context

    daia_build_context_set_workspace \
        "$TEST_ROOT/workspace" >/dev/null
    daia_build_context_set_plugin_root \
        "$TEST_ROOT/plugins" >/dev/null
    daia_build_context_set_profile \
        "$TEST_ROOT/profiles/default.yaml" >/dev/null
    daia_build_context_set_execution_plan \
        "$TEST_ROOT/plans/default.plan" >/dev/null
    daia_build_context_set_architecture \
        x86_64 >/dev/null
    daia_build_context_set_output_directory \
        "$TEST_ROOT/output" >/dev/null
    daia_build_context_set_build_name \
        ubuntu-server >/dev/null
    daia_build_context_set_version \
        24.04 >/dev/null

    assert_success \
        "validation succeeds" \
        daia_build_context_validate

    assert_success \
        "context reports valid" \
        daia_build_context_is_valid

    assert_success \
        "context reports locked" \
        daia_build_context_is_locked

    teardown_test
}

test_locked_context_rejects_modification() {
    initialize_build_context

    daia_build_context_set_workspace \
        "$TEST_ROOT/workspace" >/dev/null
    daia_build_context_set_plugin_root \
        "$TEST_ROOT/plugins" >/dev/null
    daia_build_context_set_profile \
        "$TEST_ROOT/profiles/default.yaml" >/dev/null
    daia_build_context_set_execution_plan \
        "$TEST_ROOT/plans/default.plan" >/dev/null
    daia_build_context_set_architecture \
        x86_64 >/dev/null
    daia_build_context_set_output_directory \
        "$TEST_ROOT/output" >/dev/null
    daia_build_context_set_build_name \
        ubuntu-server >/dev/null
    daia_build_context_set_version \
        24.04 >/dev/null

    daia_build_context_validate >/dev/null

    assert_failure \
        "locked context rejects modification" \
        daia_build_context_set_build_name \
        another-build

    teardown_test
}


initialize_build_context() {
    setup_test

    daia_build_context_init >/dev/null
}

populate_valid_context() {
    daia_build_context_set_workspace \
        "$TEST_ROOT/workspace" >/dev/null

    daia_build_context_set_plugin_root \
        "$TEST_ROOT/plugins" >/dev/null

    daia_build_context_set_profile \
        "$TEST_ROOT/profiles/default.yaml" >/dev/null

    daia_build_context_set_execution_plan \
        "$TEST_ROOT/plans/default.plan" >/dev/null

    daia_build_context_set_architecture \
        x86_64 >/dev/null

    daia_build_context_set_output_directory \
        "$TEST_ROOT/output" >/dev/null

    daia_build_context_set_build_name \
        ubuntu-server >/dev/null

    daia_build_context_set_version \
        24.04 >/dev/null
}

main() {
    source "$BUILD_CONTEXT"

    trap destroy_test_environment EXIT

    assert_success \
        "operations require initialization" \
        test_operations_require_initialization

    assert_success \
        "initialization succeeds" \
        test_initialization_succeeds

    assert_success \
        "double initialization rejected" \
        test_double_initialization_rejected

    assert_success \
        "clear succeeds" \
        test_clear_succeeds

    assert_success \
        "clear before initialization rejected" \
        test_clear_before_initialization_rejected

    assert_success \
        "initial state" \
        test_context_initial_state

    assert_success \
        "workspace setter/getter" \
        test_workspace_setter_and_getter

    assert_success \
        "plugin root setter/getter" \
        test_plugin_root_setter_and_getter

    assert_success \
        "profile setter/getter" \
        test_profile_setter_and_getter

    assert_success \
        "execution plan setter/getter" \
        test_execution_plan_setter_and_getter

    assert_success \
        "architecture setter/getter" \
        test_architecture_setter_and_getter

    assert_success \
        "output directory setter/getter" \
        test_output_directory_setter_and_getter

    assert_success \
        "build name setter/getter" \
        test_build_name_setter_and_getter

    assert_success \
        "version setter/getter" \
        test_version_setter_and_getter

    assert_success \
        "context completeness" \
        test_context_is_complete

    assert_success \
        "validation rejects incomplete context" \
        test_validation_rejects_incomplete_context

    assert_success \
        "invalid architecture rejected" \
        test_invalid_architecture_rejected

    assert_success \
        "empty build name rejected" \
        test_empty_build_name_rejected

    assert_success \
        "empty version rejected" \
        test_empty_version_rejected

    assert_success \
        "successful validation" \
        test_successful_validation

    assert_success \
        "locked context rejects modification" \
        test_locked_context_rejects_modification

    printf '\n'
    printf 'Tests run: %d\n' "$TESTS_RUN"
    printf 'Failures : %d\n' "$TESTS_FAILED"

    if [[ "$TESTS_FAILED" -ne 0 ]]; then
        return 1
    fi

    printf '\nAll build-context tests passed.\n'

    return 0
}

main "$@"
