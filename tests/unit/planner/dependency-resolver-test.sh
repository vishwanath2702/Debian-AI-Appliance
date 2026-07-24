#!/usr/bin/env bash
#
# Unit tests for the DAIA Planner Dependency Resolver.
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

RESOLVER_FILE="${REPOSITORY_ROOT}/installer/files/opt/daia/planner/dependency-resolver.sh"
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

STUB_PLUGIN_EXISTS_STATUS=0
STUB_DEPENDENCIES_STATUS=0

###############################################################################
# Registry Adapter stubs
###############################################################################

daia_registry_adapter_plugin_exists() {
    return "$STUB_PLUGIN_EXISTS_STATUS"
}

daia_registry_adapter_plugin_dependencies() {
    if [[ "$STUB_DEPENDENCIES_STATUS" -ne 0 ]]; then
        return "$STUB_DEPENDENCIES_STATUS"
    fi

    case "$1" in
        filesystem/base)
            ;;
        package-manager/apt)
            ;;
        display-manager/lightdm)
            ;;
        desktop/xfce)
            printf '%s\n' \
                filesystem/base \
                package-manager/apt \
                display-manager/lightdm
            ;;
        ai/ollama)
            printf '%s\n' filesystem/base
            ;;
        *)
            ;;
    esac
}

###############################################################################
# Stub lifecycle
###############################################################################

reset_stubs() {
    STUB_PLUGIN_EXISTS_STATUS=0
    STUB_DEPENDENCIES_STATUS=0
}
###############################################################################
# Public API tests
###############################################################################

test_public_api_exists() {
    assert_success \
        "resolve exported" \
        assert_function_exists \
        daia_dependency_resolver_resolve

    assert_success \
        "resolve_many exported" \
        assert_function_exists \
        daia_dependency_resolver_resolve_many
}

###############################################################################
# resolve() tests
###############################################################################

test_resolve_no_dependencies() {
    local output

    reset_stubs

    output="$(
        daia_dependency_resolver_resolve \
            filesystem/base
    )"

    assert_success \
        "resolve plugin without dependencies" \
        assert_equals \
        "filesystem/base" \
        "$output"
}

test_resolve_single_level_dependencies() {
    local output

    reset_stubs

    output="$(
        daia_dependency_resolver_resolve \
            desktop/xfce
    )"

    assert_success \
        "resolve emits dependency-first order" \
        assert_equals \
        $'filesystem/base\npackage-manager/apt\ndisplay-manager/lightdm\ndesktop/xfce' \
        "$output"
}

test_resolve_unknown_plugin() {
    reset_stubs

    STUB_PLUGIN_EXISTS_STATUS=1

    assert_failure \
        "resolve rejects unknown plugin" \
        daia_dependency_resolver_resolve \
        desktop/xfce
}

test_resolve_dependency_lookup_failure() {
    reset_stubs

    STUB_DEPENDENCIES_STATUS=42

    assert_failure \
        "resolve propagates dependency lookup failure" \
        daia_dependency_resolver_resolve \
        desktop/xfce
}

test_resolve_missing_argument() {
    assert_failure \
        "resolve rejects missing argument" \
        daia_dependency_resolver_resolve
}

test_resolve_invalid_plugin_id() {
    assert_failure \
        "resolve rejects invalid plugin ID" \
        daia_dependency_resolver_resolve \
        INVALID/PLUGIN
}
###############################################################################
# resolve_many() tests
###############################################################################

test_resolve_many_single() {
    local output

    reset_stubs

    output="$(
        daia_dependency_resolver_resolve_many \
            desktop/xfce
    )"

    assert_success \
        "resolve_many single plugin" \
        assert_equals \
        $'filesystem/base\npackage-manager/apt\ndisplay-manager/lightdm\ndesktop/xfce' \
        "$output"
}

test_resolve_many_multiple() {
    local output

    reset_stubs

    output="$(
        daia_dependency_resolver_resolve_many \
            desktop/xfce \
            ai/ollama
    )"

    assert_success \
        "resolve_many suppresses duplicate dependencies" \
        assert_equals \
        $'filesystem/base\npackage-manager/apt\ndisplay-manager/lightdm\ndesktop/xfce\nai/ollama' \
        "$output"
}

test_resolve_many_duplicate_plugins() {
    local output

    reset_stubs

    output="$(
        daia_dependency_resolver_resolve_many \
            desktop/xfce \
            desktop/xfce
    )"

    assert_success \
        "resolve_many suppresses duplicate plugins" \
        assert_equals \
        $'filesystem/base\npackage-manager/apt\ndisplay-manager/lightdm\ndesktop/xfce' \
        "$output"
}

test_resolve_many_empty() {
    assert_failure \
        "resolve_many rejects empty plugin list" \
        daia_dependency_resolver_resolve_many
}

test_resolve_many_failure() {
    reset_stubs

    STUB_PLUGIN_EXISTS_STATUS=1

    assert_failure \
        "resolve_many propagates failures" \
        daia_dependency_resolver_resolve_many \
        desktop/xfce
}

###############################################################################
# Repeated sourcing
###############################################################################

test_repeated_sourcing() {
    assert_success \
        "repeated sourcing succeeds" \
        bash -c "
            source '$RESOLVER_FILE'
            source '$RESOLVER_FILE'
        "
}

###############################################################################
# Main
###############################################################################

main() {
    if [[ ! -r "$RESOLVER_FILE" ]]; then
        printf 'ERROR: Dependency Resolver not found: %s\n' \
            "$RESOLVER_FILE" >&2
        return 1
    fi

    reset_stubs

    # shellcheck source=/dev/null
    source "$RESOLVER_FILE"

    test_public_api_exists

    test_resolve_no_dependencies
    test_resolve_single_level_dependencies
    test_resolve_unknown_plugin
    test_resolve_dependency_lookup_failure
    test_resolve_missing_argument
    test_resolve_invalid_plugin_id

    test_resolve_many_single
    test_resolve_many_multiple
    test_resolve_many_duplicate_plugins
    test_resolve_many_empty
    test_resolve_many_failure

    test_repeated_sourcing

    printf '\nTests run: %d\n' "$TESTS_RUN"
    printf 'Failures:  %d\n' "$TESTS_FAILED"

    [[ "$TESTS_FAILED" -eq 0 ]]
}

main "$@"
