#!/usr/bin/env bash
#
# Unit tests for the DAIA Planner Capability Resolver.
#

set -u
set -o pipefail

###############################################################################
# Repository paths
###############################################################################

TEST_DIR="$(
    cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 || exit 1
    pwd -P
)"
readonly TEST_DIR

REPOSITORY_ROOT="$(
    cd -- "${TEST_DIR}/../../.." >/dev/null 2>&1 || exit 1
    pwd -P
)"
readonly REPOSITORY_ROOT

RESOLVER_FILE="${REPOSITORY_ROOT}/installer/files/opt/daia/planner/capability-resolver.sh"
readonly RESOLVER_FILE

###############################################################################
# Test counters
###############################################################################

TESTS_RUN=0
TESTS_FAILED=0

###############################################################################
# Assertions
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

assert_failure() {
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
    [[ "$1" == "$2" ]]
}

assert_function_exists() {
    declare -F "$1" >/dev/null
}

###############################################################################
# Registry Adapter stub state
###############################################################################

STUB_CAPABILITY_EXISTS_STATUS=0

STUB_PROVIDER_COUNT_STATUS=0
STUB_PROVIDER_COUNT_OUTPUT="1"

STUB_PROVIDERS_STATUS=0
STUB_PROVIDER_OVERRIDE=""
###############################################################################
# Registry Adapter stubs
###############################################################################

daia_registry_adapter_capability_exists() {
    return "$STUB_CAPABILITY_EXISTS_STATUS"
}

daia_registry_adapter_capability_provider_count() {
    printf '%s\n' "$STUB_PROVIDER_COUNT_OUTPUT"
    return "$STUB_PROVIDER_COUNT_STATUS"
}
daia_registry_adapter_capability_providers() {
    if [[ -n "$STUB_PROVIDER_OVERRIDE" ]]; then
        printf '%s\n' "$STUB_PROVIDER_OVERRIDE"
        return "$STUB_PROVIDERS_STATUS"
    fi

    case "$1" in
        desktop.environment)
            printf '%s\n' "desktop/xfce"
            ;;
        ai.runtime)
            printf '%s\n' "ai/ollama"
            ;;
        filesystem)
            printf '%s\n' "filesystem/base"
            ;;
        *)
            return "$STUB_PROVIDERS_STATUS"
            ;;
    esac

    return "$STUB_PROVIDERS_STATUS"
}
###############################################################################
# Stub lifecycle
###############################################################################

reset_stubs() {
    STUB_CAPABILITY_EXISTS_STATUS=0

    STUB_PROVIDER_COUNT_STATUS=0
    STUB_PROVIDER_COUNT_OUTPUT="1"

    STUB_PROVIDERS_STATUS=0
    STUB_PROVIDER_OVERRIDE=""
}



###############################################################################
# Public API tests
###############################################################################

test_public_api_exists() {
    assert_success \
        "resolve exported" \
        assert_function_exists \
        daia_capability_resolver_resolve

    assert_success \
        "resolve_many exported" \
        assert_function_exists \
        daia_capability_resolver_resolve_many
}

###############################################################################
# resolve() tests
###############################################################################

test_resolve_success() {
    local output

    reset_stubs

    STUB_PROVIDER_COUNT_OUTPUT="1"

    output="$(
        daia_capability_resolver_resolve \
            desktop.environment
    )"

    assert_success \
        "resolve returns provider" \
        assert_equals \
        "desktop/xfce" \
        "$output"
}

test_resolve_unknown_capability() {
    reset_stubs

    STUB_CAPABILITY_EXISTS_STATUS=1

    assert_failure \
        "resolve rejects unknown capability" \
        daia_capability_resolver_resolve \
        desktop.environment
}

test_resolve_zero_providers() {
    reset_stubs

    STUB_PROVIDER_COUNT_OUTPUT="0"

    assert_failure \
        "resolve rejects zero providers" \
        daia_capability_resolver_resolve \
        desktop.environment
}

test_resolve_invalid_provider_count() {
    reset_stubs

    STUB_PROVIDER_COUNT_OUTPUT="abc"

    assert_failure \
        "resolve rejects invalid provider count" \
        daia_capability_resolver_resolve \
        desktop.environment
}

test_resolve_invalid_provider() {
    reset_stubs

    STUB_PROVIDER_OVERRIDE="INVALID"

    assert_failure \
        "resolve rejects invalid provider id" \
        daia_capability_resolver_resolve \
        desktop.environment
}
test_resolve_provider_failure() {
    reset_stubs

    STUB_PROVIDERS_STATUS=42

    assert_failure \
        "resolve propagates provider failure" \
        daia_capability_resolver_resolve \
        desktop.environment
}

test_resolve_missing_argument() {
    assert_failure \
        "resolve rejects missing argument" \
        daia_capability_resolver_resolve
}

test_resolve_invalid_capability() {
    assert_failure \
        "resolve rejects invalid capability" \
        daia_capability_resolver_resolve \
        INVALID/CAPABILITY
}



###############################################################################
# resolve_many() tests
###############################################################################

test_resolve_many_single() {
    local output

    reset_stubs

    output="$(
        daia_capability_resolver_resolve_many \
            desktop.environment
    )"

    assert_success \
        "resolve_many single capability" \
        assert_equals \
        "desktop/xfce" \
        "$output"
}

test_resolve_many_duplicate_capabilities() {
    local output

    reset_stubs

    output="$(
        daia_capability_resolver_resolve_many \
            desktop.environment \
            desktop.environment
    )"

    assert_success \
        "resolve_many removes duplicate plugins" \
        assert_equals \
        "desktop/xfce" \
        "$output"
}
test_resolve_many_duplicate_provider() {
    local output

    reset_stubs

    output="$(
        daia_capability_resolver_resolve_many \
            desktop.environment \
            desktop.environment
    )"

    assert_success \
        "resolve_many suppresses duplicate providers" \
        assert_equals \
        "desktop/xfce" \
        "$output"
}
test_resolve_many_multiple() {
    local output

    reset_stubs

    output="$(
        daia_capability_resolver_resolve_many \
            desktop.environment \
            ai.runtime
    )"

    assert_success \
        "resolve_many resolves multiple capabilities" \
        assert_equals \
        $'desktop/xfce\nai/ollama' \
        "$output"
}
test_resolve_many_empty() {
    assert_failure \
        "resolve_many rejects empty list" \
        daia_capability_resolver_resolve_many
}

test_resolve_many_failure() {
    reset_stubs

    STUB_CAPABILITY_EXISTS_STATUS=1

    assert_failure \
        "resolve_many propagates failures" \
        daia_capability_resolver_resolve_many \
        desktop.environment
}

###############################################################################
# Main
###############################################################################

main() {
    if [[ ! -r "$RESOLVER_FILE" ]]; then
        printf 'ERROR: Capability Resolver not found: %s\n' \
            "$RESOLVER_FILE" >&2
        return 1
    fi

    reset_stubs

    # shellcheck source=/dev/null
    source "$RESOLVER_FILE"
test_public_api_exists

test_resolve_success
test_resolve_unknown_capability
test_resolve_zero_providers
test_resolve_invalid_provider_count
test_resolve_invalid_provider
test_resolve_provider_failure
test_resolve_missing_argument
test_resolve_invalid_capability
test_resolve_many_single
test_resolve_many_duplicate_capabilities
test_resolve_many_multiple
test_resolve_many_empty
test_resolve_many_failure
    printf '\nTests run: %d\n' "$TESTS_RUN"
    printf 'Failures:  %d\n' "$TESTS_FAILED"

    [[ "$TESTS_FAILED" -eq 0 ]]
}

main "$@"
