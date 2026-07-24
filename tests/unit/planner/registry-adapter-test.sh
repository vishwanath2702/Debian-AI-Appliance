#!/usr/bin/env bash
#
# DAIA Planner Registry Adapter unit tests
#
# Registry mock functions are invoked indirectly by the sourced adapter.
# shellcheck disable=SC2317

set -u
set -o pipefail

###############################################################################
# Paths
###############################################################################

TEST_DIR="$(
    cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 || exit 1
    pwd -P
)"

REPOSITORY_ROOT="$(
    cd -- "${TEST_DIR}/../../.." >/dev/null 2>&1 || exit 1
    pwd -P
)"

ADAPTER_FILE="${REPOSITORY_ROOT}/installer/files/opt/daia/planner/registry-adapter.sh"

if [[ ! -r "$ADAPTER_FILE" ]]; then
    printf 'Test setup error: adapter is not readable: %s\n' \
        "$ADAPTER_FILE" >&2
    exit 1
fi

# shellcheck source=/dev/null
source "$ADAPTER_FILE"

###############################################################################
# Test counters
###############################################################################

TESTS_RUN=0
TESTS_FAILED=0

###############################################################################
# Mock state
###############################################################################

MOCK_PLUGIN_IDS_OUTPUT=$'desktop/xfce\nruntime/ollama'
MOCK_PLUGIN_IDS_STATUS=0

MOCK_PLUGIN_EXISTS_EXPECTED='desktop/xfce'
MOCK_PLUGIN_EXISTS_STATUS=0
MOCK_PLUGIN_EXISTS_ACTUAL=''

MOCK_METADATA_EXPECTED_PLUGIN='desktop/xfce'
MOCK_METADATA_EXPECTED_KEY='provides'
MOCK_METADATA_OUTPUT='desktop.environment'
MOCK_METADATA_STATUS=0
MOCK_METADATA_ACTUAL_PLUGIN=''
MOCK_METADATA_ACTUAL_KEY=''

MOCK_CAPABILITY_EXISTS_EXPECTED='desktop.environment'
MOCK_CAPABILITY_EXISTS_STATUS=0
MOCK_CAPABILITY_EXISTS_ACTUAL=''

MOCK_PROVIDERS_EXPECTED='desktop.environment'
MOCK_PROVIDERS_OUTPUT=$'desktop/xfce\ndesktop/gnome'
MOCK_PROVIDERS_STATUS=0
MOCK_PROVIDERS_ACTUAL=''

MOCK_PROVIDER_COUNT_EXPECTED='desktop.environment'
MOCK_PROVIDER_COUNT_OUTPUT='2'
MOCK_PROVIDER_COUNT_STATUS=0
MOCK_PROVIDER_COUNT_ACTUAL=''

MOCK_DEPENDENCIES_EXPECTED='desktop/xfce'
MOCK_DEPENDENCIES_OUTPUT=$'base/system\nsession/dbus'
MOCK_DEPENDENCIES_STATUS=0
MOCK_DEPENDENCIES_ACTUAL=''

###############################################################################
# Assertions
###############################################################################

test_pass() {
    local description="$1"

    TESTS_RUN=$((TESTS_RUN + 1))
    printf 'ok %d - %s\n' "$TESTS_RUN" "$description"
}

test_fail() {
    local description="$1"
    local details="$2"

    TESTS_RUN=$((TESTS_RUN + 1))
    TESTS_FAILED=$((TESTS_FAILED + 1))

    printf 'not ok %d - %s\n' "$TESTS_RUN" "$description"
    printf '  %s\n' "$details"
}

assert_status() {
    local description="$1"
    local expected="$2"
    local actual="$3"

    if [[ "$actual" -eq "$expected" ]]; then
        test_pass "$description"
    else
        test_fail \
            "$description" \
            "expected status $expected; received $actual"
    fi
}

assert_equals() {
    local description="$1"
    local expected="$2"
    local actual="$3"

    if [[ "$actual" == "$expected" ]]; then
        test_pass "$description"
    else
        test_fail \
            "$description" \
            "expected [$expected]; received [$actual]"
    fi
}

assert_contains() {
    local description="$1"
    local output="$2"
    local expected_text="$3"

    if [[ "$output" == *"$expected_text"* ]]; then
        test_pass "$description"
    else
        test_fail \
            "$description" \
            "expected output to contain [$expected_text]; received [$output]"
    fi
}

assert_function_exists() {
    local function_name="$1"

    if declare -F "$function_name" >/dev/null; then
        test_pass "API exists: $function_name"
    else
        test_fail \
            "API exists: $function_name" \
            "function is not defined"
    fi
}

###############################################################################
# Top-level Registry mocks
#
# These are intentionally top-level functions. This avoids ShellCheck SC2317
# warnings caused by dynamically defining nested mock functions.
###############################################################################

daia_registry_plugin_ids() {
    printf '%s\n' "$MOCK_PLUGIN_IDS_OUTPUT"
    return "$MOCK_PLUGIN_IDS_STATUS"
}

daia_registry_plugin_exists() {
    MOCK_PLUGIN_EXISTS_ACTUAL="${1-}"

    if [[ "$MOCK_PLUGIN_EXISTS_ACTUAL" != "$MOCK_PLUGIN_EXISTS_EXPECTED" ]]; then
        return 1
    fi

    return "$MOCK_PLUGIN_EXISTS_STATUS"
}

daia_registry_plugin_metadata() {
    MOCK_METADATA_ACTUAL_PLUGIN="${1-}"
    MOCK_METADATA_ACTUAL_KEY="${2-}"

    if [[ "$MOCK_METADATA_ACTUAL_PLUGIN" != "$MOCK_METADATA_EXPECTED_PLUGIN" ]]; then
        return 1
    fi

    if [[ "$MOCK_METADATA_ACTUAL_KEY" != "$MOCK_METADATA_EXPECTED_KEY" ]]; then
        return 1
    fi

    if [[ "$MOCK_METADATA_STATUS" -ne 0 ]]; then
        return "$MOCK_METADATA_STATUS"
    fi

    printf '%s\n' "$MOCK_METADATA_OUTPUT"
}

daia_capability_exists() {
    MOCK_CAPABILITY_EXISTS_ACTUAL="${1-}"

    if [[ "$MOCK_CAPABILITY_EXISTS_ACTUAL" != "$MOCK_CAPABILITY_EXISTS_EXPECTED" ]]; then
        return 1
    fi

    return "$MOCK_CAPABILITY_EXISTS_STATUS"
}

daia_capability_get_providers() {
    MOCK_PROVIDERS_ACTUAL="${1-}"

    if [[ "$MOCK_PROVIDERS_ACTUAL" != "$MOCK_PROVIDERS_EXPECTED" ]]; then
        return 1
    fi

    if [[ "$MOCK_PROVIDERS_STATUS" -ne 0 ]]; then
        return "$MOCK_PROVIDERS_STATUS"
    fi

    printf '%s\n' "$MOCK_PROVIDERS_OUTPUT"
}

daia_capability_provider_count() {
    MOCK_PROVIDER_COUNT_ACTUAL="${1-}"

    if [[ "$MOCK_PROVIDER_COUNT_ACTUAL" != "$MOCK_PROVIDER_COUNT_EXPECTED" ]]; then
        return 1
    fi

    if [[ "$MOCK_PROVIDER_COUNT_STATUS" -ne 0 ]]; then
        return "$MOCK_PROVIDER_COUNT_STATUS"
    fi

    printf '%s\n' "$MOCK_PROVIDER_COUNT_OUTPUT"
}

daia_plugin_registry_plugin_dependencies() {
    MOCK_DEPENDENCIES_ACTUAL="${1-}"

    if [[ "$MOCK_DEPENDENCIES_ACTUAL" != "$MOCK_DEPENDENCIES_EXPECTED" ]]; then
        return 1
    fi

    if [[ "$MOCK_DEPENDENCIES_STATUS" -ne 0 ]]; then
        return "$MOCK_DEPENDENCIES_STATUS"
    fi

    printf '%s\n' "$MOCK_DEPENDENCIES_OUTPUT"
}

###############################################################################
# Mock reset
###############################################################################

reset_mocks() {
    MOCK_PLUGIN_IDS_OUTPUT=$'desktop/xfce\nruntime/ollama'
    MOCK_PLUGIN_IDS_STATUS=0

    MOCK_PLUGIN_EXISTS_EXPECTED='desktop/xfce'
    MOCK_PLUGIN_EXISTS_STATUS=0
    MOCK_PLUGIN_EXISTS_ACTUAL=''

    MOCK_METADATA_EXPECTED_PLUGIN='desktop/xfce'
    MOCK_METADATA_EXPECTED_KEY='provides'
    MOCK_METADATA_OUTPUT='desktop.environment'
    MOCK_METADATA_STATUS=0
    MOCK_METADATA_ACTUAL_PLUGIN=''
    MOCK_METADATA_ACTUAL_KEY=''

    MOCK_CAPABILITY_EXISTS_EXPECTED='desktop.environment'
    MOCK_CAPABILITY_EXISTS_STATUS=0
    MOCK_CAPABILITY_EXISTS_ACTUAL=''

    MOCK_PROVIDERS_EXPECTED='desktop.environment'
    MOCK_PROVIDERS_OUTPUT=$'desktop/xfce\ndesktop/gnome'
    MOCK_PROVIDERS_STATUS=0
    MOCK_PROVIDERS_ACTUAL=''

    MOCK_PROVIDER_COUNT_EXPECTED='desktop.environment'
    MOCK_PROVIDER_COUNT_OUTPUT='2'
    MOCK_PROVIDER_COUNT_STATUS=0
    MOCK_PROVIDER_COUNT_ACTUAL=''

    MOCK_DEPENDENCIES_EXPECTED='desktop/xfce'
    MOCK_DEPENDENCIES_OUTPUT=$'base/system\nsession/dbus'
    MOCK_DEPENDENCIES_STATUS=0
    MOCK_DEPENDENCIES_ACTUAL=''
}

###############################################################################
# Public API tests
###############################################################################

test_public_apis() {
    assert_function_exists daia_registry_adapter_plugin_ids
    assert_function_exists daia_registry_adapter_plugin_exists
    assert_function_exists daia_registry_adapter_plugin_capabilities
    assert_function_exists daia_registry_adapter_capability_exists
    assert_function_exists daia_registry_adapter_capability_providers
    assert_function_exists daia_registry_adapter_capability_provider_count
    assert_function_exists daia_registry_adapter_plugin_dependencies
}

test_plugin_ids() {
    local output
    local status

    reset_mocks

    output="$(daia_registry_adapter_plugin_ids)"
    status=$?

    assert_status \
        "plugin IDs succeeds" \
        0 \
        "$status"

    assert_equals \
        "plugin IDs preserves Registry stdout" \
        "$MOCK_PLUGIN_IDS_OUTPUT" \
        "$output"

    daia_registry_adapter_plugin_ids unexpected >/dev/null 2>&1
    status=$?

    assert_status \
        "plugin IDs rejects unexpected arguments" \
        1 \
        "$status"

    MOCK_PLUGIN_IDS_STATUS=42

    daia_registry_adapter_plugin_ids >/dev/null 2>&1
    status=$?

    assert_status \
        "plugin IDs preserves Registry failure status" \
        42 \
        "$status"
}

test_plugin_exists() {
    local status

    reset_mocks

    daia_registry_adapter_plugin_exists desktop/xfce >/dev/null 2>&1
    status=$?

    assert_status \
        "plugin exists succeeds" \
        0 \
        "$status"

    assert_equals \
        "plugin exists delegates plugin ID" \
        "desktop/xfce" \
        "$MOCK_PLUGIN_EXISTS_ACTUAL"

    daia_registry_adapter_plugin_exists >/dev/null 2>&1
    status=$?

    assert_status \
        "plugin exists rejects missing argument" \
        1 \
        "$status"

    daia_registry_adapter_plugin_exists \
        "Desktop/XFCE" >/dev/null 2>&1
    status=$?

    assert_status \
        "plugin exists rejects invalid plugin ID" \
        1 \
        "$status"

    MOCK_PLUGIN_EXISTS_STATUS=42

    daia_registry_adapter_plugin_exists desktop/xfce >/dev/null 2>&1
    status=$?

    assert_status \
        "plugin exists preserves Registry failure status" \
        42 \
        "$status"
}

test_plugin_capabilities() {
    local output
    local status
    local error_output

    reset_mocks

    output="$(
        daia_registry_adapter_plugin_capabilities desktop/xfce
    )"
    status=$?

    assert_status \
        "plugin capabilities succeeds" \
        0 \
        "$status"

    assert_equals \
        "plugin capabilities prints provided capability" \
        "desktop.environment" \
        "$output"


    reset_mocks
    MOCK_PLUGIN_EXISTS_STATUS=1

    error_output="$(
        daia_registry_adapter_plugin_capabilities \
            desktop/xfce 2>&1 >/dev/null
    )"
    status=$?

    assert_status \
        "plugin capabilities rejects unregistered plugin" \
        1 \
        "$status"

    assert_contains \
        "plugin capabilities reports unregistered plugin" \
        "$error_output" \
        "plugin is not registered: desktop/xfce"

    reset_mocks
    MOCK_METADATA_STATUS=42

    daia_registry_adapter_plugin_capabilities \
        desktop/xfce >/dev/null 2>&1
    status=$?

    assert_status \
        "plugin capabilities reports metadata failure" \
        1 \
        "$status"

    reset_mocks
    MOCK_METADATA_OUTPUT="Invalid/Capability"

    daia_registry_adapter_plugin_capabilities \
        desktop/xfce >/dev/null 2>&1
    status=$?

    assert_status \
        "plugin capabilities rejects invalid Registry capability" \
        1 \
        "$status"

    reset_mocks
    MOCK_METADATA_OUTPUT=''

    daia_registry_adapter_plugin_capabilities \
        desktop/xfce >/dev/null 2>&1
    status=$?

    assert_status \
        "plugin capabilities rejects empty Registry capability" \
        1 \
        "$status"
}

test_capability_exists() {
    local status

    reset_mocks

    daia_registry_adapter_capability_exists \
        desktop.environment >/dev/null 2>&1
    status=$?

    assert_status \
        "capability exists succeeds" \
        0 \
        "$status"

    assert_equals \
        "capability exists delegates capability" \
        "desktop.environment" \
        "$MOCK_CAPABILITY_EXISTS_ACTUAL"

    daia_registry_adapter_capability_exists >/dev/null 2>&1
    status=$?

    assert_status \
        "capability exists rejects missing argument" \
        1 \
        "$status"

    daia_registry_adapter_capability_exists \
        "Desktop Environment" >/dev/null 2>&1
    status=$?

    assert_status \
        "capability exists rejects invalid capability" \
        1 \
        "$status"

    MOCK_CAPABILITY_EXISTS_STATUS=42

    daia_registry_adapter_capability_exists \
        desktop.environment >/dev/null 2>&1
    status=$?

    assert_status \
        "capability exists preserves Registry failure status" \
        42 \
        "$status"
}

test_capability_providers() {
    local output
    local status

    reset_mocks

    output="$(
        daia_registry_adapter_capability_providers \
            desktop.environment
    )"
    status=$?

    assert_status \
        "capability providers succeeds" \
        0 \
        "$status"

    assert_equals \
        "capability providers preserves Registry stdout" \
        "$MOCK_PROVIDERS_OUTPUT" \
        "$output"


    MOCK_PROVIDERS_STATUS=42

    daia_registry_adapter_capability_providers \
        desktop.environment >/dev/null 2>&1
    status=$?

    assert_status \
        "capability providers preserves Registry failure status" \
        42 \
        "$status"
}

test_capability_provider_count() {
    local output
    local status

    reset_mocks

    output="$(
        daia_registry_adapter_capability_provider_count \
            desktop.environment
    )"
    status=$?

    assert_status \
        "provider count succeeds" \
        0 \
        "$status"

    assert_equals \
        "provider count preserves Registry stdout" \
        "2" \
        "$output"


    MOCK_PROVIDER_COUNT_STATUS=42

    daia_registry_adapter_capability_provider_count \
        desktop.environment >/dev/null 2>&1
    status=$?

    assert_status \
        "provider count preserves Registry failure status" \
        42 \
        "$status"
}

test_plugin_dependencies() {
    local output
    local status

    reset_mocks

    output="$(
        daia_registry_adapter_plugin_dependencies desktop/xfce
    )"
    status=$?

    assert_status \
        "plugin dependencies succeeds" \
        0 \
        "$status"

    assert_equals \
        "plugin dependencies preserves Registry stdout" \
        "$MOCK_DEPENDENCIES_OUTPUT" \
        "$output"


    daia_registry_adapter_plugin_dependencies >/dev/null 2>&1
    status=$?

    assert_status \
        "plugin dependencies rejects missing argument" \
        1 \
        "$status"

    daia_registry_adapter_plugin_dependencies \
        "Desktop/XFCE" >/dev/null 2>&1
    status=$?

    assert_status \
        "plugin dependencies rejects invalid plugin ID" \
        1 \
        "$status"

    MOCK_DEPENDENCIES_STATUS=42

    daia_registry_adapter_plugin_dependencies \
        desktop/xfce >/dev/null 2>&1
    status=$?

    assert_status \
        "plugin dependencies preserves Registry failure status" \
        42 \
        "$status"
}

###############################################################################
# Missing API tests
###############################################################################

test_missing_plugin_ids_api() {
    local saved_definition
    local status

    saved_definition="$(declare -f daia_registry_plugin_ids)"
    unset -f daia_registry_plugin_ids

    daia_registry_adapter_plugin_ids >/dev/null 2>&1
    status=$?

    assert_status \
        "plugin IDs rejects missing Registry API" \
        1 \
        "$status"

    eval "$saved_definition"
}

test_missing_plugin_metadata_api() {
    local saved_definition
    local status

    saved_definition="$(declare -f daia_registry_plugin_metadata)"
    unset -f daia_registry_plugin_metadata

    daia_registry_adapter_plugin_capabilities \
        desktop/xfce >/dev/null 2>&1
    status=$?

    assert_status \
        "plugin capabilities rejects missing metadata API" \
        1 \
        "$status"

    eval "$saved_definition"
}

test_missing_capability_providers_api() {
    local saved_definition
    local status

    saved_definition="$(declare -f daia_capability_get_providers)"
    unset -f daia_capability_get_providers

    daia_registry_adapter_capability_providers \
        desktop.environment >/dev/null 2>&1
    status=$?

    assert_status \
        "capability providers rejects missing Registry API" \
        1 \
        "$status"

    eval "$saved_definition"
}

test_missing_dependencies_api() {
    local saved_definition
    local status

    saved_definition="$(
        declare -f daia_plugin_registry_plugin_dependencies
    )"

    unset -f daia_plugin_registry_plugin_dependencies

    daia_registry_adapter_plugin_dependencies \
        desktop/xfce >/dev/null 2>&1
    status=$?

    assert_status \
        "plugin dependencies rejects missing Registry API" \
        1 \
        "$status"

    eval "$saved_definition"
}

###############################################################################
# Module guard
###############################################################################

test_repeated_sourcing() {
    local status

    # shellcheck source=/dev/null
    source "$ADAPTER_FILE"
    status=$?

    assert_status \
        "repeated sourcing succeeds" \
        0 \
        "$status"
}

###############################################################################
# Driver
###############################################################################

main() {
    test_public_apis
    test_plugin_ids
    test_plugin_exists
    test_plugin_capabilities
    test_capability_exists
    test_capability_providers
    test_capability_provider_count
    test_plugin_dependencies
    test_missing_plugin_ids_api
    test_missing_plugin_metadata_api
    test_missing_capability_providers_api
    test_missing_dependencies_api
    test_repeated_sourcing

    printf '\nTests run: %d\n' "$TESTS_RUN"
    printf 'Failures: %d\n' "$TESTS_FAILED"

    if [[ "$TESTS_FAILED" -ne 0 ]]; then
        return 1
    fi
}

main "$@"
