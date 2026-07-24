#!/usr/bin/env bash

set -u

ROOT_DIR="$(
    cd "$(dirname "${BASH_SOURCE[0]}")/../.." &&
        pwd
)"

PLUGIN_REGISTRY="$ROOT_DIR/installer/files/opt/daia/core/plugin-registry.sh"

TEST_DIRECTORY="$(mktemp -d)"
VALID_ROOT="$TEST_DIRECTORY/valid"
DUPLICATE_ROOT="$TEST_DIRECTORY/duplicate"
UNSUPPORTED_ROOT="$TEST_DIRECTORY/unsupported"
MALFORMED_ROOT="$TEST_DIRECTORY/malformed"
MISSING_FUNCTION_ROOT="$TEST_DIRECTORY/missing-function"


cleanup() {
    rm -rf "$TEST_DIRECTORY"
}


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


create_plugin() {
    local root="$1"
    local category="$2"
    local name="$3"
    local api_version="$4"
    local plugin_directory="$root/$category/$name"

    mkdir -p "$plugin_directory"

    cat > "$plugin_directory/plugin.sh" <<EOF
#!/usr/bin/env bash

daia_plugin_metadata() {
    cat <<'METADATA'
id=$category/$name
category=$category
name=$name
version=1
api_version=$api_version
description=Test plugin for $category/$name
provider=daia
METADATA
}
EOF

    chmod 0644 "$plugin_directory/plugin.sh"
}


trap cleanup EXIT

source "$PLUGIN_REGISTRY"


if daia_registry_discover "$VALID_ROOT" 2>/dev/null; then
    fail "discovery before registry initialization should fail"
fi


create_plugin "$VALID_ROOT" "desktop" "xfce" "1"
create_plugin "$VALID_ROOT" "ai-engine" "ollama" "1"


daia_registry_init

daia_registry_discover "$VALID_ROOT" ||
    fail "valid plugins could not be discovered"


daia_registry_plugin_exists "desktop/xfce" ||
    fail "desktop/xfce was not registered"


daia_registry_plugin_exists "ai-engine/ollama" ||
    fail "ai-engine/ollama was not registered"


expected_ids=$'ai-engine/ollama\ndesktop/xfce'

assert_equal \
    "$expected_ids" \
    "$(daia_registry_plugin_ids)" \
    "registered plugin IDs"


expected_xfce_path="$(
    realpath "$VALID_ROOT/desktop/xfce/plugin.sh"
)"

assert_equal \
    "$expected_xfce_path" \
    "$(daia_registry_plugin_path desktop/xfce)" \
    "desktop plugin path"


assert_equal \
    "1" \
    "$(daia_registry_plugin_metadata desktop/xfce api_version)" \
    "desktop plugin API version"


assert_equal \
    "xfce" \
    "$(daia_registry_plugin_metadata desktop/xfce name)" \
    "desktop plugin name"


if daia_registry_plugin_path "desktop/missing" >/dev/null 2>&1; then
    fail "lookup of an unknown plugin should fail"
fi


###############################################################################
# Duplicate ID
###############################################################################

create_plugin "$DUPLICATE_ROOT" "desktop" "xfce" "1"

if daia_registry_discover "$DUPLICATE_ROOT" >/dev/null 2>&1; then
    fail "duplicate plugin IDs should be rejected"
fi


###############################################################################
# Unsupported API version
###############################################################################

create_plugin "$UNSUPPORTED_ROOT" "desktop" "kde" "99"

daia_registry_init

if daia_registry_discover "$UNSUPPORTED_ROOT" >/dev/null 2>&1; then
    fail "unsupported Plugin API versions should be rejected"
fi


###############################################################################
# Missing required metadata
###############################################################################

mkdir -p "$MALFORMED_ROOT/desktop/mate"

cat > "$MALFORMED_ROOT/desktop/mate/plugin.sh" <<'EOF'
#!/usr/bin/env bash

daia_plugin_metadata() {
    cat <<'METADATA'
id=desktop/mate
category=desktop
name=mate
api_version=1
METADATA
}
EOF

chmod 0644 "$MALFORMED_ROOT/desktop/mate/plugin.sh"

daia_registry_init

if daia_registry_discover "$MALFORMED_ROOT" >/dev/null 2>&1; then
    fail "plugins with missing required metadata should be rejected"
fi


###############################################################################
# Missing metadata function
###############################################################################

mkdir -p "$MISSING_FUNCTION_ROOT/desktop/lxqt"

cat > "$MISSING_FUNCTION_ROOT/desktop/lxqt/plugin.sh" <<'EOF'
#!/usr/bin/env bash

some_other_function() {
    :
}
EOF

chmod 0644 "$MISSING_FUNCTION_ROOT/desktop/lxqt/plugin.sh"

daia_registry_init

if daia_registry_discover "$MISSING_FUNCTION_ROOT" >/dev/null 2>&1; then
    fail "plugins without daia_plugin_metadata should be rejected"
fi


printf 'PASS: Plugin Registry tests completed successfully\n'
