#!/usr/bin/env bash

set -u

ROOT_DIR="$(
    cd "$(dirname "${BASH_SOURCE[0]}")/../.." &&
        pwd
)"

CONFIGURATION_MANAGER="$ROOT_DIR/installer/files/opt/daia/core/configuration.sh"

TEST_DIRECTORY="$(mktemp -d)"
TEST_CONFIG="$TEST_DIRECTORY/installation.conf"

cleanup() {
    rm -rf "$TEST_DIRECTORY"
}

trap cleanup EXIT


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


source "$CONFIGURATION_MANAGER"


if daia_configuration_loaded; then
    fail "configuration should not initially be marked as loaded"
fi


if daia_load_configuration "$TEST_DIRECTORY/missing.conf" 2>/dev/null; then
    fail "loading a missing configuration should fail"
fi


cat > "$TEST_CONFIG" <<'EOF'
DAIA_CONFIG_VERSION="1"
DAIA_INSTALL_PROFILE="standard"
DAIA_DESKTOP="xfce"
DAIA_AI_ENGINE="ollama"
DAIA_MODELS=("model-one" "model-two")
EOF


daia_load_configuration "$TEST_CONFIG" ||
    fail "valid configuration could not be loaded"


daia_configuration_loaded ||
    fail "configuration was not marked as loaded"


assert_equal \
    "standard" \
    "$(daia_get DAIA_INSTALL_PROFILE)" \
    "installation profile"


assert_equal \
    "xfce" \
    "$(daia_get DAIA_DESKTOP)" \
    "desktop environment"


assert_equal \
    "ollama" \
    "$(daia_get DAIA_AI_ENGINE)" \
    "AI engine"


assert_equal \
    "" \
    "$(daia_get DAIA_UNDEFINED_VALUE)" \
    "undefined configuration value"


if daia_get 'INVALID-NAME' >/dev/null 2>&1; then
    fail "invalid variable names should be rejected"
fi


printf 'PASS: Configuration Manager tests completed successfully\n'
