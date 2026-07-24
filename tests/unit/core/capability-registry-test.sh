#!/usr/bin/env bash
#
# Unit tests for the DAIA Capability Registry.
#

set -u
set -o pipefail

TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_DIR
REPOSITORY_ROOT="$(cd -- "$TEST_DIR/../../.." && pwd)"
readonly REPOSITORY_ROOT
REGISTRY_FILE="$REPOSITORY_ROOT/installer/files/opt/daia/planner/capability-registry.sh"
readonly REGISTRY_FILE

TESTS_RUN=0
TESTS_FAILED=0

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

    if [[ "$actual" == "$expected" ]]; then
        printf 'PASS: %s\n' "$description"
        return 0
    fi

    TESTS_FAILED=$((TESTS_FAILED + 1))
    printf 'FAIL: %s\n' "$description" >&2
    printf '  expected: %s\n' "$expected" >&2
    printf '  actual:   %s\n' "$actual" >&2
    return 1
}

test_registry_requires_initialization() {
    __DAIA_CAPABILITY_REGISTRY_INITIALIZED=0

    ! daia_capability_registry_register "container/runtime" "runtime.docker" \
        >/dev/null 2>&1
}

test_registry_initialization_succeeds() {
    daia_capability_registry_init

    [[ "$__DAIA_CAPABILITY_REGISTRY_INITIALIZED" -eq 1 ]]
}

test_capability_registration_succeeds() {
    daia_capability_registry_init
    daia_capability_registry_register "container/runtime" "runtime.docker"

    daia_capability_exists "runtime.docker"
}

test_provider_lookup_succeeds() {
    local actual

    daia_capability_registry_init
    daia_capability_registry_register "container/runtime" "runtime.docker"
    actual="$(daia_capability_get_providers "runtime.docker")"

    [[ "$actual" == "container/runtime" ]]
}
test_reverse_lookup_succeeds() {
    local actual

    daia_capability_registry_init || { echo "init failed"; return 1; }

    daia_capability_registry_register \
        "container/runtime" \
        "runtime.docker" || { echo "register failed"; return 1; }

    actual="$(daia_plugin_get_capabilities "container/runtime")" || {
        echo "lookup failed"
        return 1
    }

}
test_multiple_providers_are_sorted() {
    daia_capability_registry_init

    daia_capability_registry_register \
        "container/runtime" \
        "runtime.docker" || return 1

    daia_capability_registry_register \
        "container/network" \
        "runtime.docker" || return 1

    local actual
    local expected

    actual="$(daia_capability_get_providers "runtime.docker")"
    expected=$'container/network\ncontainer/runtime'

    [[ "$actual" == "$expected" ]]
}
test_plugin_capabilities_are_sorted() {
    daia_capability_registry_init

    daia_capability_registry_register \
        "container/runtime" \
        "runtime.podman" \
        "runtime.docker" || return 1

    local actual
    local expected

    actual="$(daia_plugin_get_capabilities "container/runtime")"
    expected=$'runtime.docker\nruntime.podman'

    [[ "$actual" == "$expected" ]]
}
test_duplicate_registration_is_rejected() {
    daia_capability_registry_init

    daia_capability_registry_register \
        "container/runtime" \
        "runtime.docker" || return 1

    ! daia_capability_registry_register \
        "container/runtime" \
        "runtime.docker" >/dev/null 2>&1 || return 1

    local providers
    local capabilities

    providers="$(daia_capability_get_providers "runtime.docker")"
    capabilities="$(daia_plugin_get_capabilities "container/runtime")"

    [[ "$providers" == "container/runtime" ]] &&
        [[ "$capabilities" == "runtime.docker" ]]
}
test_unknown_capability_has_no_providers() {
    local actual

    daia_capability_registry_init
    actual="$(daia_capability_get_providers "unknown.capability")"

    [[ -z "$actual" ]]
}

test_unknown_plugin_has_no_capabilities() {
    local actual

    daia_capability_registry_init
    actual="$(daia_plugin_get_capabilities "runtime/unknown")"

    [[ -z "$actual" ]]
}

test_empty_capability_is_rejected() {
    daia_capability_registry_init

    ! daia_capability_registry_register "" "runtime.docker" >/dev/null 2>&1
}

test_empty_plugin_is_rejected() {
    daia_capability_registry_init

    ! daia_capability_registry_register "container/runtime" "" >/dev/null 2>&1
}

test_initialization_clears_registry() {
    daia_capability_registry_init
    daia_capability_registry_register "container/runtime" "runtime.docker"
    daia_capability_registry_init

    ! daia_capability_exists "runtime.docker"
}

test_reverse_lookup() {
    daia_capability_registry_init

    daia_capability_registry_register \
        "container/runtime" \
        "runtime.docker" || return 1

    local actual
    actual="$(daia_plugin_get_capabilities "container/runtime")"

    [[ "$actual" == "runtime.docker" ]]
}

main() {
    if [[ ! -r "$REGISTRY_FILE" ]]; then
        fail "registry file not found: $REGISTRY_FILE"
        exit 1
    fi

    # shellcheck source=/dev/null
    source "$REGISTRY_FILE"

    assert_success \
        "registry operations require initialization" \
        test_registry_requires_initialization || true
    assert_success \
        "registry initialization succeeds" \
        test_registry_initialization_succeeds || true
    assert_success \
        "capability registration succeeds" \
        test_capability_registration_succeeds || true
    assert_success \
        "provider lookup succeeds" \
        test_provider_lookup_succeeds || true
    assert_success \
        "reverse lookup succeeds" \
        test_reverse_lookup_succeeds || true
    assert_success \
        "multiple providers are sorted" \
        test_multiple_providers_are_sorted || true
    assert_success \
        "plugin capabilities are sorted" \
        test_plugin_capabilities_are_sorted || true
    assert_success \
        "duplicate registration is rejected" \
        test_duplicate_registration_is_rejected || true
    assert_success \
        "unknown capability has no providers" \
        test_unknown_capability_has_no_providers || true
    assert_success \
        "unknown plugin has no capabilities" \
        test_unknown_plugin_has_no_capabilities || true
    assert_success \
        "empty capability is rejected" \
        test_empty_capability_is_rejected || true
    assert_success \
        "empty plugin is rejected" \
        test_empty_plugin_is_rejected || true
    assert_success \
        "initialization clears the registry" \
        test_initialization_clears_registry || true

    printf '\nTests run: %d\n' "$TESTS_RUN"
    printf 'Failures:  %d\n' "$TESTS_FAILED"

    [[ "$TESTS_FAILED" -eq 0 ]]
}

main "$@"
