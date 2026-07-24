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
}

assert_equals() {
    local expected="$1"
    local actual="$2"

    [[ "$actual" == "$expected" ]]
}

register_fixture() {
    daia_capability_registry_init || return 1
    daia_capability_registry_register \
        "runtime/ollama" \
        "ai.runtime" \
        "model.runtime" || return 1
    daia_capability_registry_register \
        "runtime/llama-cpp" \
        "ai.runtime" || return 1
    daia_capability_registry_register \
        "desktop/xfce" \
        "desktop.environment"
}

test_init_rejects_arguments() {
    daia_capability_registry_init "extra"
}

test_empty_registry_enumeration() {
    local capabilities
    local plugins

    daia_capability_registry_init || return 1
    capabilities="$(daia_capability_registry_get_capabilities)" || return 1
    plugins="$(daia_capability_registry_get_plugins)" || return 1

    [[ -z "$capabilities" && -z "$plugins" ]]
}

test_capability_exists() {
    register_fixture || return 1
    daia_capability_exists "ai.runtime"
}

test_unknown_capability_does_not_exist() {
    register_fixture || return 1
    daia_capability_exists "network.service"
}

test_plugin_exists() {
    register_fixture || return 1
    daia_plugin_exists "runtime/ollama"
}

test_unknown_plugin_does_not_exist() {
    register_fixture || return 1
    daia_plugin_exists "service/openssh"
}

test_plugin_provides_capability() {
    register_fixture || return 1
    daia_plugin_provides_capability "runtime/ollama" "ai.runtime"
}

test_plugin_does_not_provide_capability() {
    register_fixture || return 1
    daia_plugin_provides_capability "desktop/xfce" "ai.runtime"
}

test_provider_lookup_is_sorted() {
    local actual

    register_fixture || return 1
    actual="$(daia_capability_get_providers "ai.runtime")" || return 1
    assert_equals $'runtime/llama-cpp\nruntime/ollama' "$actual"
}

test_provider_count() {
    local actual

    register_fixture || return 1
    actual="$(daia_capability_provider_count "ai.runtime")" || return 1
    assert_equals "2" "$actual"
}

test_plugin_capability_lookup_is_sorted() {
    local actual

    register_fixture || return 1
    actual="$(daia_plugin_get_capabilities "runtime/ollama")" || return 1
    assert_equals $'ai.runtime\nmodel.runtime' "$actual"
}

test_plugin_capability_count() {
    local actual

    register_fixture || return 1
    actual="$(daia_plugin_capability_count "runtime/ollama")" || return 1
    assert_equals "2" "$actual"
}

test_capability_enumeration_is_sorted() {
    local actual

    register_fixture || return 1
    actual="$(daia_capability_registry_get_capabilities)" || return 1
    assert_equals $'ai.runtime\ndesktop.environment\nmodel.runtime' "$actual"
}

test_plugin_enumeration_is_sorted() {
    local actual

    register_fixture || return 1
    actual="$(daia_capability_registry_get_plugins)" || return 1
    assert_equals $'desktop/xfce\nruntime/llama-cpp\nruntime/ollama' "$actual"
}

test_registry_capability_count() {
    local actual

    register_fixture || return 1
    actual="$(daia_capability_registry_capability_count)" || return 1
    assert_equals "3" "$actual"
}

test_registry_plugin_count() {
    local actual

    register_fixture || return 1
    actual="$(daia_capability_registry_plugin_count)" || return 1
    assert_equals "3" "$actual"
}

test_unknown_capability_provider_lookup_fails() {
    register_fixture || return 1
    daia_capability_get_providers "network.service"
}

test_unknown_capability_provider_count_fails() {
    register_fixture || return 1
    daia_capability_provider_count "network.service"
}

test_unknown_plugin_capability_lookup_fails() {
    register_fixture || return 1
    daia_plugin_get_capabilities "service/openssh"
}

test_unknown_plugin_capability_count_fails() {
    register_fixture || return 1
    daia_plugin_capability_count "service/openssh"
}

test_lookup_requires_initialization() {
    __DAIA_CAPABILITY_REGISTRY_INITIALIZED=0
    daia_capability_registry_get_capabilities
}

test_lookup_rejects_invalid_capability() {
    daia_capability_registry_init || return 1
    daia_capability_exists "invalid/capability"
}

test_lookup_rejects_invalid_plugin_id() {
    daia_capability_registry_init || return 1
    daia_plugin_exists "invalid-plugin"
}

test_relationship_lookup_rejects_invalid_capability() {
    daia_capability_registry_init || return 1
    daia_plugin_provides_capability "runtime/ollama" "invalid/capability"
}

test_relationship_lookup_rejects_invalid_plugin_id() {
    daia_capability_registry_init || return 1
    daia_plugin_provides_capability "invalid-plugin" "ai.runtime"
}

test_lookup_rejects_extra_arguments() {
    daia_capability_registry_init || return 1
    daia_capability_exists "ai.runtime" "extra"
}

test_enumeration_rejects_arguments() {
    daia_capability_registry_init || return 1
    daia_capability_registry_get_plugins "extra"
}

test_clear_resets_lookup_state() {
    local capability_count
    local plugin_count

    register_fixture || return 1
    daia_capability_registry_clear || return 1
    capability_count="$(daia_capability_registry_capability_count)" || return 1
    plugin_count="$(daia_capability_registry_plugin_count)" || return 1

    [[ "$capability_count" == "0" && "$plugin_count" == "0" ]]
}

test_duplicate_registration_is_rejected() {
    daia_capability_registry_init || return 1
    daia_capability_registry_register "runtime/ollama" "ai.runtime" || return 1
    daia_capability_registry_register "runtime/ollama" "ai.runtime"
}

test_failed_registration_is_transactional() {
    daia_capability_registry_init || return 1

    daia_capability_registry_register \
        "runtime/ollama" \
        "ai.runtime" \
        "invalid/capability" >/dev/null 2>&1 || true

    ! daia_plugin_exists "runtime/ollama" &&
        ! daia_capability_exists "ai.runtime"
}

main() {
    # shellcheck source=/dev/null
    source "$REGISTRY_FILE"

    assert_command_fails "init rejects arguments" test_init_rejects_arguments || true
    assert_success "empty registry enumerates no entries" test_empty_registry_enumeration || true
    assert_success "known capability exists" test_capability_exists || true
    assert_command_fails "unknown capability does not exist" test_unknown_capability_does_not_exist || true
    assert_success "known plugin exists" test_plugin_exists || true
    assert_command_fails "unknown plugin does not exist" test_unknown_plugin_does_not_exist || true
    assert_success "plugin-capability relationship exists" test_plugin_provides_capability || true
    assert_command_fails "missing plugin-capability relationship fails" test_plugin_does_not_provide_capability || true
    assert_success "provider lookup is sorted" test_provider_lookup_is_sorted || true
    assert_success "provider count is correct" test_provider_count || true
    assert_success "plugin capability lookup is sorted" test_plugin_capability_lookup_is_sorted || true
    assert_success "plugin capability count is correct" test_plugin_capability_count || true
    assert_success "capability enumeration is sorted" test_capability_enumeration_is_sorted || true
    assert_success "plugin enumeration is sorted" test_plugin_enumeration_is_sorted || true
    assert_success "registry capability count is correct" test_registry_capability_count || true
    assert_success "registry plugin count is correct" test_registry_plugin_count || true
    assert_command_fails "unknown capability provider lookup fails" test_unknown_capability_provider_lookup_fails || true
    assert_command_fails "unknown capability provider count fails" test_unknown_capability_provider_count_fails || true
    assert_command_fails "unknown plugin capability lookup fails" test_unknown_plugin_capability_lookup_fails || true
    assert_command_fails "unknown plugin capability count fails" test_unknown_plugin_capability_count_fails || true
    assert_command_fails "lookup requires initialization" test_lookup_requires_initialization || true
    assert_command_fails "lookup rejects invalid capability" test_lookup_rejects_invalid_capability || true
    assert_command_fails "lookup rejects invalid plugin ID" test_lookup_rejects_invalid_plugin_id || true
    assert_command_fails "relationship lookup rejects invalid capability" test_relationship_lookup_rejects_invalid_capability || true
    assert_command_fails "relationship lookup rejects invalid plugin ID" test_relationship_lookup_rejects_invalid_plugin_id || true
    assert_command_fails "lookup rejects extra arguments" test_lookup_rejects_extra_arguments || true
    assert_command_fails "enumeration rejects arguments" test_enumeration_rejects_arguments || true
    assert_success "clear resets lookup state" test_clear_resets_lookup_state || true
    assert_command_fails "duplicate registration is rejected" test_duplicate_registration_is_rejected || true
    assert_success "failed registration remains transactional" test_failed_registration_is_transactional || true

    printf '\nTests run: %d\n' "$TESTS_RUN"
    printf 'Failures:  %d\n' "$TESTS_FAILED"

    [[ "$TESTS_FAILED" -eq 0 ]]
}

main "$@"
