#!/usr/bin/env bash

set -u

###############################################################################
# Test configuration
###############################################################################

TEST_DIRECTORY="$(
    cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd
)" || exit 1

PROJECT_ROOT="$(
    cd -- "$TEST_DIRECTORY/../../.." && pwd
)" || exit 1

PLANNER_FILE="$PROJECT_ROOT/installer/files/opt/daia/planner/planner.sh"
EXECUTION_PLAN_BUILDER_FILE="$PROJECT_ROOT/installer/files/opt/daia/planner/execution-plan-builder.sh"

TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

###############################################################################
# Test framework
###############################################################################

fail() {
    printf 'FAIL: %s\n' "$1" >&2
    return 1
}

assert_success() {
    local status="$1"
    local message="$2"

    if ((status != 0)); then
        fail "$message: expected success, received status $status"
        return 1
    fi
}

assert_failure() {
    local status="$1"
    local message="$2"

    if ((status == 0)); then
        fail "$message: expected failure"
        return 1
    fi
}

assert_equals() {
    local expected="$1"
    local actual="$2"
    local message="$3"

    if [[ "$actual" != "$expected" ]]; then
        printf 'FAIL: %s\n' "$message" >&2
        printf '  expected: %q\n' "$expected" >&2
        printf '  actual:   %q\n' "$actual" >&2
        return 1
    fi
}

assert_function_exists() {
    local function_name="$1"

    if ! declare -F "$function_name" >/dev/null; then
        fail "expected function to exist: $function_name"
        return 1
    fi
}

run_test() {
    local test_name="$1"

    ((TESTS_RUN += 1))

    if "$test_name"; then
        ((TESTS_PASSED += 1))
        printf 'PASS: %s\n' "$test_name"
    else
        ((TESTS_FAILED += 1))
    fi
}

###############################################################################
# Isolated invocation helper
###############################################################################

run_planner_shell() {
    local test_code="$1"

    PLANNER_FILE="$PLANNER_FILE" \
    EXECUTION_PLAN_BUILDER_FILE="$EXECUTION_PLAN_BUILDER_FILE" \
        bash -c '
            set -u

            # Prevent planner.sh from loading the real builder. This allows
            # each unit test to inject the exact collaborator it needs.
            source() {
                if [[ "$1" == "$EXECUTION_PLAN_BUILDER_FILE" ]]; then
                    return 0
                fi

                builtin source "$@"
            }

            builtin source "$PLANNER_FILE"

            eval "$1"
        ' bash "$test_code"
}

###############################################################################
# Tests
###############################################################################

test_public_api_exists() {
    run_planner_shell '
        declare -F daia_planner_build_execution_plan >/dev/null
    '
}

test_missing_argument_is_rejected() {
    local status

    run_planner_shell '
        daia_execution_plan_builder_build() {
            return 0
        }

        daia_planner_build_execution_plan
    ' >/dev/null 2>&1
    status=$?

    assert_failure "$status" \
        "planner should reject a missing profile ID"
}

test_extra_argument_is_rejected() {
    local status

    run_planner_shell '
        daia_execution_plan_builder_build() {
            return 0
        }

        daia_planner_build_execution_plan workstation extra
    ' >/dev/null 2>&1
    status=$?

    assert_failure "$status" \
        "planner should reject extra arguments"
}

test_missing_builder_api_is_rejected() {
    local status

    run_planner_shell '
        unset -f daia_execution_plan_builder_build 2>/dev/null || true
        daia_planner_build_execution_plan workstation
    ' >/dev/null 2>&1
    status=$?

    assert_failure "$status" \
        "planner should reject an unavailable Execution Plan Builder API"
}

test_delegates_profile_id_to_builder() {
    local output
    local status

    output="$(
        run_planner_shell '
            daia_execution_plan_builder_build() {
                printf "received:%s\n" "$1"
            }

            daia_planner_build_execution_plan workstation
        '
    )"
    status=$?

    assert_success "$status" \
        "planner delegation should succeed" || return 1

    assert_equals \
        "received:workstation" \
        "$output" \
        "planner should pass the profile ID unchanged"
}

test_returns_builder_output_unchanged() {
    local expected
    local output
    local status

    expected=$'filesystem/base\npackage-manager/apt\ndesktop/xfce'

    output="$(
        run_planner_shell '
            daia_execution_plan_builder_build() {
                printf "%s\n" \
                    filesystem/base \
                    package-manager/apt \
                    desktop/xfce
            }

            daia_planner_build_execution_plan workstation
        '
    )"
    status=$?

    assert_success "$status" \
        "planner should return builder output" || return 1

    assert_equals \
        "$expected" \
        "$output" \
        "planner should preserve the execution plan"
}

test_propagates_builder_failure() {
    local status

    run_planner_shell '
        daia_execution_plan_builder_build() {
            return 37
        }

        daia_planner_build_execution_plan workstation
    ' >/dev/null 2>&1
    status=$?

    assert_equals \
        "37" \
        "$status" \
        "planner should propagate the builder exit status"
}

test_repeated_sourcing_is_safe() {
    local status

    PLANNER_FILE="$PLANNER_FILE" bash -c '
        set -u

        source "$PLANNER_FILE"
        source "$PLANNER_FILE"

        declare -F daia_planner_build_execution_plan >/dev/null
    ' >/dev/null 2>&1
    status=$?

    assert_success "$status" \
        "planner should support repeated sourcing"
}

###############################################################################
# Test runner
###############################################################################

main() {
    run_test test_public_api_exists
    run_test test_missing_argument_is_rejected
    run_test test_extra_argument_is_rejected
    run_test test_missing_builder_api_is_rejected
    run_test test_delegates_profile_id_to_builder
    run_test test_returns_builder_output_unchanged
    run_test test_propagates_builder_failure
    run_test test_repeated_sourcing_is_safe

    printf '\nTests: %d, Passed: %d, Failed: %d\n' \
        "$TESTS_RUN" \
        "$TESTS_PASSED" \
        "$TESTS_FAILED"

    ((TESTS_FAILED == 0))
}

main
