#!/usr/bin/env bash

set -u

ROOT_DIR="$(
    cd "$(dirname "${BASH_SOURCE[0]}")/../.." &&
        pwd
)"

DESIRED_STATE_MANAGER="$ROOT_DIR/installer/files/opt/daia/core/desired-state.sh"


fail() {
    printf 'FAIL: %s\n' "$*" >&2
    exit 1
}


assert_equal() {
    local expected="$1"
    local actual="$2"
    local description="$3"

    if [[ "$expected" != "$actual" ]]; then
        fail "$description: expected '$expected', got '$actual'"
    fi
}


source "$DESIRED_STATE_MANAGER"


if daia_state_add_resource \
    "package-curl" "package" "present" "apt" 2>/dev/null
then
    fail "adding a resource before initialization should fail"
fi


daia_state_init


daia_state_add_resource \
    "package-curl" \
    "package" \
    "present" \
    "apt" ||
    fail "could not add package resource"


daia_state_set_property \
    "package-curl" \
    "name" \
    "curl" ||
    fail "could not set package name"


daia_state_add_resource \
    "service-ollama" \
    "service" \
    "running" \
    "systemd" ||
    fail "could not add service resource"


daia_state_set_property \
    "service-ollama" \
    "name" \
    "ollama" ||
    fail "could not set service name"


daia_state_set_property \
    "service-ollama" \
    "enabled" \
    "true" ||
    fail "could not set service enabled property"


daia_state_add_dependency \
    "service-ollama" \
    "package-curl" ||
    fail "could not add dependency"


assert_equal \
    "package" \
    "$(daia_state_get_field package-curl type)" \
    "package resource type"


assert_equal \
    "present" \
    "$(daia_state_get_field package-curl desired_state)" \
    "package desired state"


assert_equal \
    "apt" \
    "$(daia_state_get_field package-curl provider)" \
    "package provider"


assert_equal \
    "curl" \
    "$(daia_state_get_property package-curl name)" \
    "package name"


assert_equal \
    "true" \
    "$(daia_state_get_property service-ollama enabled)" \
    "service enabled property"


assert_equal \
    "package-curl" \
    "$(daia_state_dependencies service-ollama)" \
    "service dependency"


expected_resources=$'package-curl\nservice-ollama'

assert_equal \
    "$expected_resources" \
    "$(daia_state_resource_ids)" \
    "resource insertion order"


if daia_state_add_resource \
    "package-curl" "package" "present" "apt" 2>/dev/null
then
    fail "duplicate resource IDs should be rejected"
fi


if daia_state_add_dependency \
    "package-curl" "package-curl" 2>/dev/null
then
    fail "self-dependencies should be rejected"
fi


daia_state_seal ||
    fail "could not seal desired state"


daia_state_is_sealed ||
    fail "desired state was not marked as sealed"


if daia_state_set_property \
    "package-curl" "name" "wget" 2>/dev/null
then
    fail "sealed desired state should be immutable"
fi


printf 'PASS: Desired State Manager tests completed successfully\n'
