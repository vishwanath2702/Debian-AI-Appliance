#!/usr/bin/env bash
#
# Unit tests for the DAIA Profile Reader.
#

# Test doubles and test cases are invoked indirectly by the Profile Reader and
# test harness, so ShellCheck cannot statically determine that every function
# is reachable.
# shellcheck disable=SC2317

set -u
set -o pipefail

TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_DIR

REPOSITORY_ROOT="$(cd -- "$TEST_DIR/../../.." && pwd)"
readonly REPOSITORY_ROOT

PROFILE_READER_FILE="$REPOSITORY_ROOT/installer/files/opt/daia/planner/profile-reader.sh"
readonly PROFILE_READER_FILE

TESTS_RUN=0
TESTS_FAILED=0

###############################################################################
# Test assertions
###############################################################################

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

assert_command_fails() {
    local description="$1"
    shift

    TESTS_RUN=$((TESTS_RUN + 1))

    if "$@" >/dev/null 2>&1; then
        TESTS_FAILED=$((TESTS_FAILED + 1))
        printf 'FAIL: %s\n' "$description" >&2
        return 1
    fi

    printf 'PASS: %s\n' "$description"
    return 0
}

assert_equals() {
    local expected="$1"
    local actual="$2"

    if [[ "$actual" == "$expected" ]]; then
        return 0
    fi

    printf 'Expected:\n%s\n' "$expected" >&2
    printf 'Actual:\n%s\n' "$actual" >&2
    return 1
}

###############################################################################
# Profile Registry test doubles
###############################################################################

install_profile_registry_api_stubs() {
    daia_profile_registry_exists() {
        case "${1-}" in
            workstation|minimal)
                return 0
                ;;
            *)
                return 1
                ;;
        esac
    }

    daia_profile_registry_capabilities() {
        case "${1-}" in
            workstation)
                printf '%s\n' \
                    "desktop.environment" \
                    "ai.runtime"
                ;;
            minimal)
                return 0
                ;;
            *)
                return 1
                ;;
        esac
    }
}

remove_profile_registry_api_stubs() {
    unset -f daia_profile_registry_exists 2>/dev/null || true
    unset -f daia_profile_registry_capabilities 2>/dev/null || true
}

###############################################################################
# Test cases
###############################################################################

test_public_api_exists() {
    declare -F daia_profile_reader_read >/dev/null
}

test_known_profile_returns_capabilities() {
    local expected
    local actual

    install_profile_registry_api_stubs

    expected=$'desktop.environment\nai.runtime'
    actual="$(daia_profile_reader_read "workstation")" || return 1

    assert_equals "$expected" "$actual"
}

test_profile_with_no_capabilities_succeeds() {
    local actual

    install_profile_registry_api_stubs

    actual="$(daia_profile_reader_read "minimal")" || return 1

    assert_equals "" "$actual"
}

test_unknown_profile_is_rejected() {
    install_profile_registry_api_stubs

    daia_profile_reader_read "unknown"
}

test_empty_profile_id_is_rejected() {
    install_profile_registry_api_stubs

    daia_profile_reader_read ""
}

test_invalid_profile_id_is_rejected() {
    install_profile_registry_api_stubs

    daia_profile_reader_read "invalid/profile"
}

test_missing_profile_argument_is_rejected() {
    install_profile_registry_api_stubs

    daia_profile_reader_read
}

test_extra_arguments_are_rejected() {
    install_profile_registry_api_stubs

    daia_profile_reader_read "workstation" "extra"
}

test_missing_profile_exists_api_is_rejected() {
    install_profile_registry_api_stubs
    unset -f daia_profile_registry_exists

    daia_profile_reader_read "workstation"
}

test_missing_profile_capabilities_api_is_rejected() {
    install_profile_registry_api_stubs
    unset -f daia_profile_registry_capabilities

    daia_profile_reader_read "workstation"
}

test_profile_exists_failure_is_propagated() {
    install_profile_registry_api_stubs

    daia_profile_registry_exists() {
        return 2
    }

    daia_profile_reader_read "workstation"
}

test_profile_capabilities_failure_is_propagated() {
    install_profile_registry_api_stubs

    daia_profile_registry_capabilities() {
        return 2
    }

    daia_profile_reader_read "workstation"
}

test_module_can_be_sourced_repeatedly() {
    install_profile_registry_api_stubs

    # shellcheck source-path=SCRIPTDIR
    # shellcheck source=../../../installer/files/opt/daia/planner/profile-reader.sh
    source "$PROFILE_READER_FILE" || return 1

    daia_profile_reader_read "workstation" >/dev/null
}

###############################################################################
# Test execution
###############################################################################

# shellcheck source-path=SCRIPTDIR
# shellcheck source=../../../installer/files/opt/daia/planner/profile-reader.sh
source "$PROFILE_READER_FILE"

assert_success \
    "public Profile Reader API exists" \
    test_public_api_exists

assert_success \
    "a known profile returns its requested capabilities" \
    test_known_profile_returns_capabilities

assert_success \
    "a profile with no requested capabilities succeeds" \
    test_profile_with_no_capabilities_succeeds

assert_command_fails \
    "an unknown profile is rejected" \
    test_unknown_profile_is_rejected

assert_command_fails \
    "an empty profile ID is rejected" \
    test_empty_profile_id_is_rejected

assert_command_fails \
    "an invalid profile ID is rejected" \
    test_invalid_profile_id_is_rejected

assert_command_fails \
    "a missing profile argument is rejected" \
    test_missing_profile_argument_is_rejected

assert_command_fails \
    "extra Profile Reader arguments are rejected" \
    test_extra_arguments_are_rejected

assert_command_fails \
    "a missing profile-existence API is rejected" \
    test_missing_profile_exists_api_is_rejected

assert_command_fails \
    "a missing profile-capabilities API is rejected" \
    test_missing_profile_capabilities_api_is_rejected

assert_command_fails \
    "profile-existence failures are propagated" \
    test_profile_exists_failure_is_propagated

assert_command_fails \
    "profile-capability read failures are propagated" \
    test_profile_capabilities_failure_is_propagated

assert_success \
    "the Profile Reader module can be sourced repeatedly" \
    test_module_can_be_sourced_repeatedly

remove_profile_registry_api_stubs

printf '\nTests run: %d\n' "$TESTS_RUN"
printf 'Failures:  %d\n' "$TESTS_FAILED"

if [[ "$TESTS_FAILED" -ne 0 ]]; then
    exit 1
fi

exit 0
