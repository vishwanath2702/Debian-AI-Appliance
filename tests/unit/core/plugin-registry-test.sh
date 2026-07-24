#!/usr/bin/env bash
#
# Unit tests for the DAIA Plugin Registry.
#

set -u
set -o pipefail

TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_DIR
REPOSITORY_ROOT="$(cd -- "$TEST_DIR/../../.." && pwd)"
readonly REPOSITORY_ROOT
REGISTRY_FILE="$REPOSITORY_ROOT/installer/files/opt/daia/core/plugin-registry.sh"
readonly REGISTRY_FILE

TESTS_RUN=0
TESTS_FAILED=0
TEST_WORK_DIR=""

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

create_plugin() {
    local relative_path="$1"
    local metadata_body="$2"
    local plugin_file="$TEST_WORK_DIR/$relative_path/plugin.sh"

    mkdir -p -- "$(dirname -- "$plugin_file")"

    cat > "$plugin_file" <<EOF_PLUGIN
#!/usr/bin/env bash

daia_plugin_metadata() {
    cat <<'EOF_METADATA'
$metadata_body
EOF_METADATA
}
EOF_PLUGIN

    chmod 0755 "$plugin_file"
}

valid_metadata() {
    local plugin_id="${1:-runtime/ollama}"

    cat <<EOF_METADATA
id=$plugin_id
plugin_version=1.2.3
api_version=1
provider=daia
 description=ignored
EOF_METADATA
}

setup() {
    TEST_WORK_DIR="$(mktemp -d)"
    daia_registry_init
}

teardown() {
    rm -rf -- "$TEST_WORK_DIR"
}

metadata_fixture() {
    local plugin_id="${1:-runtime/ollama}"

    cat <<EOF_METADATA
id=$plugin_id
plugin_version=1.2.3
api_version=1
provider=daia
description=Local AI runtime
provides=ai.runtime
EOF_METADATA
}


replace_metadata_value() {
    local metadata="$1"
    local key="$2"
    local value="$3"

    printf '%s\n' "$metadata" |
        awk -F= -v key="$key" -v value="$value" '
            $1 == key {
                print key "=" value
                next
            }

            {
                print
            }
        '
}

test_valid_metadata_is_registered() {
    local actual

    setup
    create_plugin "runtime/ollama" "$(metadata_fixture)"

    daia_registry_discover "$TEST_WORK_DIR" || {
        teardown
        return 1
    }

    actual="$(daia_registry_plugin_metadata runtime/ollama plugin_version)"
    teardown

    [[ "$actual" == "1.2.3" ]]
}


test_accessor_plugin_path_round_trip() {
    local expected="/tmp/daia-plugin.sh"
    local actual

    daia_registry_init
    _daia_registry_set_plugin_path "runtime/test" "$expected"
    actual="$(_daia_registry_get_plugin_path "runtime/test")"

    [[ "$actual" == "$expected" ]]
}

test_accessor_metadata_round_trip() {
    local actual

    daia_registry_init
    _daia_registry_set_metadata "runtime/test" "provider" "daia"
    actual="$(_daia_registry_get_metadata "runtime/test" "provider")"

    [[ "$actual" == "daia" ]] &&
        _daia_registry_has_metadata "runtime/test" "provider"
}

test_accessor_raw_metadata_round_trip() {
    local expected
    local actual

    expected=$'id=runtime/test\nprovider=daia'

    daia_registry_init
    _daia_registry_set_raw_metadata "runtime/test" "$expected"
    actual="$(_daia_registry_get_raw_metadata "runtime/test")"

    [[ "$actual" == "$expected" ]] &&
        _daia_registry_has_raw_metadata "runtime/test"
}

test_invalid_plugin_id_is_rejected() {
    setup
    create_plugin "runtime/invalid" "$(metadata_fixture 'Runtime/Ollama')"

    if daia_registry_discover "$TEST_WORK_DIR" >/dev/null 2>&1; then
        teardown
        return 1
    fi

    teardown
}

test_empty_plugin_id_is_rejected() {
    local metadata

    metadata="$(replace_metadata_value "$(metadata_fixture)" id "")"

    setup
    create_plugin "runtime/empty-id" "$metadata"

    if daia_registry_discover "$TEST_WORK_DIR" >/dev/null 2>&1; then
        teardown
        return 1
    fi

    teardown
}


test_invalid_plugin_version_is_rejected() {
    local metadata

    metadata="$(metadata_fixture)"
    metadata="${metadata/plugin_version=1.2.3/plugin_version=1.2}"

    setup
    create_plugin "runtime/invalid-version" "$metadata"

    if daia_registry_discover "$TEST_WORK_DIR" >/dev/null 2>&1; then
        teardown
        return 1
    fi

    teardown
}

test_empty_plugin_version_is_rejected() {
    local metadata

    metadata="$(replace_metadata_value "$(metadata_fixture)" plugin_version "")"

    setup
    create_plugin "runtime/empty-version" "$metadata"

    if daia_registry_discover "$TEST_WORK_DIR" >/dev/null 2>&1; then
        teardown
        return 1
    fi

    teardown
}


test_unsupported_api_version_is_rejected() {
    local metadata

    metadata="$(metadata_fixture)"
    metadata="${metadata/api_version=1/api_version=2}"

    setup
    create_plugin "runtime/unsupported-api" "$metadata"

    if daia_registry_discover "$TEST_WORK_DIR" >/dev/null 2>&1; then
        teardown
        return 1
    fi

    teardown
}

test_empty_api_version_is_rejected() {
    local metadata

    metadata="$(replace_metadata_value "$(metadata_fixture)" api_version "")"

    setup
    create_plugin "runtime/empty-api" "$metadata"

    if daia_registry_discover "$TEST_WORK_DIR" >/dev/null 2>&1; then
        teardown
        return 1
    fi

    teardown
}


test_invalid_provider_is_rejected() {
    local metadata

    metadata="$(metadata_fixture)"
    metadata="${metadata/provider=daia/provider=DAIA Project}"

    setup
    create_plugin "runtime/invalid-provider" "$metadata"

    if daia_registry_discover "$TEST_WORK_DIR" >/dev/null 2>&1; then
        teardown
        return 1
    fi

    teardown
}

test_empty_provider_is_rejected() {
    local metadata

    metadata="$(replace_metadata_value "$(metadata_fixture)" provider "")"

    setup
    create_plugin "runtime/empty-provider" "$metadata"

    if daia_registry_discover "$TEST_WORK_DIR" >/dev/null 2>&1; then
        teardown
        return 1
    fi

    teardown
}


test_blank_description_is_rejected() {
    local metadata

    metadata="$(metadata_fixture)"
    metadata="${metadata/description=Local AI runtime/description=   }"

    setup
    create_plugin "runtime/blank-description" "$metadata"

    if daia_registry_discover "$TEST_WORK_DIR" >/dev/null 2>&1; then
        teardown
        return 1
    fi

    teardown
}

test_invalid_provides_is_rejected() {
    local metadata

    metadata="$(metadata_fixture)"
    metadata="${metadata/provides=ai.runtime/provides=ai runtime}"

    setup
    create_plugin "runtime/invalid-capability" "$metadata"

    if daia_registry_discover "$TEST_WORK_DIR" >/dev/null 2>&1; then
        teardown
        return 1
    fi

    teardown
}

test_empty_provides_is_rejected() {
    local metadata

    metadata="$(replace_metadata_value "$(metadata_fixture)" provides "")"

    setup
    create_plugin "runtime/empty-capability" "$metadata"

    if daia_registry_discover "$TEST_WORK_DIR" >/dev/null 2>&1; then
        teardown
        return 1
    fi

    teardown
}


test_unknown_metadata_key_is_rejected() {
    local metadata

    metadata="$(metadata_fixture)"
    metadata+=$'\nunknown=value'

    setup
    create_plugin "runtime/unknown-key" "$metadata"

    if daia_registry_discover "$TEST_WORK_DIR" >/dev/null 2>&1; then
        teardown
        return 1
    fi

    teardown
}

test_duplicate_metadata_key_is_rejected() {
    local metadata

    metadata="$(metadata_fixture)"
    metadata+=$'\nprovider=another-provider'

    setup
    create_plugin "runtime/duplicate-key" "$metadata"

    if daia_registry_discover "$TEST_WORK_DIR" >/dev/null 2>&1; then
        teardown
        return 1
    fi

    teardown
}

test_missing_required_metadata_is_rejected() {
    local metadata

    metadata="$(metadata_fixture)"
    metadata="$(printf '%s\n' "$metadata" | grep -v '^description=')"

    setup
    create_plugin "runtime/missing-description" "$metadata"

    if daia_registry_discover "$TEST_WORK_DIR" >/dev/null 2>&1; then
        teardown
        return 1
    fi

    teardown
}

test_duplicate_plugin_id_is_rejected() {
    setup
    create_plugin "runtime/first" "$(metadata_fixture 'runtime/shared')"
    create_plugin "runtime/second" "$(metadata_fixture 'runtime/shared')"

    if daia_registry_discover "$TEST_WORK_DIR" >/dev/null 2>&1; then
        teardown
        return 1
    fi

    teardown
}

test_group_writable_plugin_is_rejected() {
    setup
    create_plugin "runtime/unsafe" "$(metadata_fixture 'runtime/unsafe')"
    chmod 0775 "$TEST_WORK_DIR/runtime/unsafe/plugin.sh"

    if daia_registry_discover "$TEST_WORK_DIR" >/dev/null 2>&1; then
        teardown
        return 1
    fi

    teardown
}

main() {
    if [[ ! -r "$REGISTRY_FILE" ]]; then
        fail "registry file not found: $REGISTRY_FILE"
        exit 1
    fi

    # shellcheck source=/dev/null
    source "$REGISTRY_FILE"

    assert_success \
        "valid metadata is registered" \
        test_valid_metadata_is_registered || true
    assert_success \
        "plugin-path accessor round trip succeeds" \
        test_accessor_plugin_path_round_trip || true
    assert_success \
        "metadata accessor round trip succeeds" \
        test_accessor_metadata_round_trip || true
    assert_success \
        "raw-metadata accessor round trip succeeds" \
        test_accessor_raw_metadata_round_trip || true
    assert_success \
        "invalid plugin ID is rejected" \
        test_invalid_plugin_id_is_rejected || true
    assert_success \
        "empty plugin ID is rejected" \
        test_empty_plugin_id_is_rejected || true
    assert_success \
        "invalid plugin version is rejected" \
        test_invalid_plugin_version_is_rejected || true
    assert_success \
        "empty plugin version is rejected" \
        test_empty_plugin_version_is_rejected || true
    assert_success \
        "unsupported API version is rejected" \
        test_unsupported_api_version_is_rejected || true
    assert_success \
        "empty API version is rejected" \
        test_empty_api_version_is_rejected || true
    assert_success \
        "invalid provider is rejected" \
        test_invalid_provider_is_rejected || true
    assert_success \
        "empty provider is rejected" \
        test_empty_provider_is_rejected || true
    assert_success \
        "blank description is rejected" \
        test_blank_description_is_rejected || true
    assert_success \
        "invalid provided capability is rejected" \
        test_invalid_provides_is_rejected || true
    assert_success \
        "empty provided capability is rejected" \
        test_empty_provides_is_rejected || true
    assert_success \
        "unknown metadata key is rejected" \
        test_unknown_metadata_key_is_rejected || true
    assert_success \
        "duplicate metadata key is rejected" \
        test_duplicate_metadata_key_is_rejected || true
    assert_success \
        "missing required metadata is rejected" \
        test_missing_required_metadata_is_rejected || true
    assert_success \
        "duplicate plugin ID is rejected" \
        test_duplicate_plugin_id_is_rejected || true
    assert_success \
        "group-writable plugin is rejected" \
        test_group_writable_plugin_is_rejected || true

    printf '\nTests run: %d\n' "$TESTS_RUN"
    printf 'Failures:  %d\n' "$TESTS_FAILED"

    [[ "$TESTS_FAILED" -eq 0 ]]
}

main "$@"
