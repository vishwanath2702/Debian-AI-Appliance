#!/usr/bin/env bash
#
# Unit tests for the DAIA Capability Synchronizer.
#

# Test doubles and test cases are invoked indirectly by the synchronizer and
# test harness, so ShellCheck cannot statically determine that every function
# is reachable.
# shellcheck disable=SC2317

set -u
set -o pipefail

TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_DIR

REPOSITORY_ROOT="$(cd -- "$TEST_DIR/../../.." && pwd)"
readonly REPOSITORY_ROOT

SYNCHRONIZER_FILE="$REPOSITORY_ROOT/installer/files/opt/daia/planner/capability-synchronizer.sh"
readonly SYNCHRONIZER_FILE

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

    [[ "$actual" == "$expected" ]]
}

###############################################################################
# Public API test doubles
###############################################################################

install_public_api_stubs() {
    __TEST_REGISTRATIONS=()
    __TEST_REGISTRY_INIT_COUNT=0

    daia_registry_adapter_plugin_ids() {
        printf '%s\n' \
            "desktop/xfce" \
            "runtime/ollama"
    }

    daia_registry_adapter_plugin_capabilities() {
        case "${1-}" in
            desktop/xfce)
                printf '%s\n' "desktop.environment"
                ;;
            runtime/ollama)
                printf '%s\n' "ai.runtime"
                ;;
            *)
                return 1
                ;;
        esac
    }

    daia_capability_registry_init() {
        __TEST_REGISTRY_INIT_COUNT=$((__TEST_REGISTRY_INIT_COUNT + 1))
        __TEST_REGISTRATIONS=()
    }

    daia_capability_registry_register() {
        __TEST_REGISTRATIONS+=("${1-}=${2-}")
    }
}

remove_public_api_stubs() {
    unset -f \
        daia_registry_adapter_plugin_ids \
        daia_registry_adapter_plugin_capabilities \
        daia_capability_registry_init \
        daia_capability_registry_register 2>/dev/null || true
}

###############################################################################
# Test cases
###############################################################################

test_synchronizes_plugin_capabilities() {
    local actual

    install_public_api_stubs
    daia_capability_synchronizer_sync || return 1

    actual="$(printf '%s\n' "${__TEST_REGISTRATIONS[@]}")"

    assert_equals \
        $'desktop/xfce=desktop.environment\nruntime/ollama=ai.runtime' \
        "$actual"
}

test_empty_plugin_registry_initializes_empty_capability_registry() {
    install_public_api_stubs

    daia_registry_adapter_plugin_ids() {
        return 0
    }

    daia_capability_synchronizer_sync || return 1

    [[ "$__TEST_REGISTRY_INIT_COUNT" -eq 1 && "${#__TEST_REGISTRATIONS[@]}" -eq 0 ]]
}

test_repeated_sync_is_idempotent() {
    local first_sync
    local second_sync

    install_public_api_stubs

    daia_capability_synchronizer_sync || return 1
    first_sync="$(printf '%s\n' "${__TEST_REGISTRATIONS[@]}")"

    daia_capability_synchronizer_sync || return 1
    second_sync="$(printf '%s\n' "${__TEST_REGISTRATIONS[@]}")"

    [[ "$__TEST_REGISTRY_INIT_COUNT" -eq 2 ]] &&
        assert_equals "$first_sync" "$second_sync"
}

test_multiple_capabilities_are_registered() {
    local actual

    install_public_api_stubs

    daia_registry_adapter_plugin_ids() {
        printf '%s\n' "runtime/ollama"
    }

    daia_registry_adapter_plugin_capabilities() {
        printf '%s\n' \
            "ai.chat" \
            "ai.runtime"
    }

    daia_capability_synchronizer_sync || return 1
    actual="$(printf '%s\n' "${__TEST_REGISTRATIONS[@]}")"

    assert_equals \
        $'runtime/ollama=ai.chat\nruntime/ollama=ai.runtime' \
        "$actual"
}

test_missing_required_api_fails() {
    install_public_api_stubs
    unset -f daia_registry_adapter_plugin_ids

    daia_capability_synchronizer_sync
}

test_sync_rejects_arguments() {
    install_public_api_stubs

    daia_capability_synchronizer_sync "extra"
}

test_plugin_enumeration_failure_is_propagated() {
    install_public_api_stubs

    daia_registry_adapter_plugin_ids() {
        return 1
    }

    daia_capability_synchronizer_sync
}

test_capability_lookup_failure_is_propagated() {
    install_public_api_stubs

    daia_registry_adapter_plugin_capabilities() {
        return 1
    }

    daia_capability_synchronizer_sync
}

test_empty_capability_set_is_rejected() {
    install_public_api_stubs

    daia_registry_adapter_plugin_capabilities() {
        return 0
    }

    daia_capability_synchronizer_sync
}

test_registry_is_unchanged_when_discovery_fails() {
    install_public_api_stubs
    __TEST_REGISTRATIONS=("existing/plugin=existing.capability")

    daia_registry_adapter_plugin_capabilities() {
        return 1
    }

    daia_capability_synchronizer_sync >/dev/null 2>&1 || true

    [[ "$__TEST_REGISTRY_INIT_COUNT" -eq 0 ]] &&
        [[ "${__TEST_REGISTRATIONS[0]}" == "existing/plugin=existing.capability" ]]
}

test_registry_initialization_failure_is_propagated() {
    install_public_api_stubs

    daia_capability_registry_init() {
        return 1
    }

    daia_capability_synchronizer_sync
}

test_registration_failure_is_propagated() {
    install_public_api_stubs

    daia_capability_registry_register() {
        return 1
    }

    daia_capability_synchronizer_sync
}

###############################################################################
# Test execution
###############################################################################

# shellcheck source-path=SCRIPTDIR
# shellcheck source=../../../installer/files/opt/daia/planner/capability-synchronizer.sh
source "$SYNCHRONIZER_FILE"

assert_success \
    "plugin capabilities are synchronized" \
    test_synchronizes_plugin_capabilities

assert_success \
    "an empty Plugin Registry produces an empty Capability Registry" \
    test_empty_plugin_registry_initializes_empty_capability_registry

assert_success \
    "repeated synchronization is idempotent" \
    test_repeated_sync_is_idempotent

assert_success \
    "multiple capabilities per plugin are synchronized" \
    test_multiple_capabilities_are_registered

assert_command_fails \
    "missing required public APIs are rejected" \
    test_missing_required_api_fails

assert_command_fails \
    "synchronization rejects arguments" \
    test_sync_rejects_arguments

assert_command_fails \
    "plugin enumeration failures are propagated" \
    test_plugin_enumeration_failure_is_propagated

assert_command_fails \
    "capability lookup failures are propagated" \
    test_capability_lookup_failure_is_propagated

assert_command_fails \
    "empty capability sets are rejected" \
    test_empty_capability_set_is_rejected

assert_success \
    "discovery failures do not reset the Capability Registry" \
    test_registry_is_unchanged_when_discovery_fails

assert_command_fails \
    "Capability Registry initialization failures are propagated" \
    test_registry_initialization_failure_is_propagated

assert_command_fails \
    "Capability Registry registration failures are propagated" \
    test_registration_failure_is_propagated

remove_public_api_stubs

printf '\nTests run: %d\n' "$TESTS_RUN"
printf 'Failures:  %d\n' "$TESTS_FAILED"

if [[ "$TESTS_FAILED" -ne 0 ]]; then
    exit 1
fi

exit 0
