#!/usr/bin/env bash
#
# Unit tests for the DAIA Planner Conflict Detector.
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

DETECTOR_FILE="${REPOSITORY_ROOT}/installer/files/opt/daia/planner/conflict-detector.sh"
readonly DETECTOR_FILE

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

assert_function_exists() {
    declare -F "$1" >/dev/null
}

###############################################################################
# Registry Adapter stub state
###############################################################################

STUB_PLUGIN_EXISTS_STATUS=0
STUB_PLUGIN_CONFLICTS_STATUS=0

###############################################################################
# Registry Adapter stubs
###############################################################################

daia_registry_adapter_plugin_exists() {
    local plugin_id="$1"

    if [[ "$STUB_PLUGIN_EXISTS_STATUS" -ne 0 ]]; then
        return "$STUB_PLUGIN_EXISTS_STATUS"
    fi

    case "$plugin_id" in
        filesystem/base | \
        desktop/xfce | \
        desktop/gnome | \
        desktop/kde | \
        display-manager/lightdm | \
        display-manager/gdm)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

daia_registry_adapter_plugin_conflicts() {
    local plugin_id="$1"

    if [[ "$STUB_PLUGIN_CONFLICTS_STATUS" -ne 0 ]]; then
        return "$STUB_PLUGIN_CONFLICTS_STATUS"
    fi

    case "$plugin_id" in
        desktop/xfce)
            printf '%s\n' \
                desktop/gnome \
                desktop/kde
            ;;
        display-manager/lightdm)
            printf '%s\n' display-manager/gdm
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
    STUB_PLUGIN_CONFLICTS_STATUS=0
}
###############################################################################
# Public API tests
###############################################################################

test_public_api_exists() {
    assert_success \
        "check exported" \
        assert_function_exists \
        daia_conflict_detector_check
}

###############################################################################
# Validation tests
###############################################################################

test_check_missing_argument() {
    assert_failure \
        "check rejects missing arguments" \
        daia_conflict_detector_check
}

test_check_invalid_plugin_id() {
    assert_failure \
        "check rejects invalid plugin ID" \
        daia_conflict_detector_check \
        INVALID/PLUGIN
}

test_check_unknown_plugin() {
    assert_failure \
        "check rejects unknown plugin" \
        daia_conflict_detector_check \
        unknown/plugin
}

###############################################################################
# Conflict detection tests
###############################################################################

test_check_no_conflicts() {
    reset_stubs

    assert_success \
        "check accepts compatible plugins" \
        daia_conflict_detector_check \
        filesystem/base \
        desktop/xfce \
        display-manager/lightdm
}

test_check_detects_conflict() {
    reset_stubs

    assert_failure \
        "check detects desktop conflict" \
        daia_conflict_detector_check \
        desktop/xfce \
        desktop/gnome
}

test_check_detects_reverse_order() {
    reset_stubs

    assert_failure \
        "check detects conflict regardless of argument order" \
        daia_conflict_detector_check \
        desktop/gnome \
        desktop/xfce
}

test_check_detects_second_conflict() {
    reset_stubs

    assert_failure \
        "check detects display manager conflict" \
        daia_conflict_detector_check \
        display-manager/lightdm \
        display-manager/gdm
}

test_check_duplicate_plugins() {
    reset_stubs

    assert_success \
        "duplicate plugins are ignored" \
        daia_conflict_detector_check \
        desktop/xfce \
        desktop/xfce
}

###############################################################################
# Registry Adapter failure propagation
###############################################################################

test_check_plugin_exists_failure() {
    reset_stubs

    STUB_PLUGIN_EXISTS_STATUS=42

    assert_failure \
        "plugin existence failure propagates" \
        daia_conflict_detector_check \
        desktop/xfce
}

test_check_conflict_lookup_failure() {
    reset_stubs

    STUB_PLUGIN_CONFLICTS_STATUS=42

    assert_failure \
        "conflict lookup failure propagates" \
        daia_conflict_detector_check \
        desktop/xfce
}
###############################################################################
# Repeated sourcing
###############################################################################

test_repeated_sourcing() {
    assert_success \
        "repeated sourcing succeeds" \
        bash -c "
            source '$DETECTOR_FILE'
            source '$DETECTOR_FILE'
        "
}

###############################################################################
# Main
###############################################################################

main() {
    if [[ ! -r "$DETECTOR_FILE" ]]; then
        printf 'ERROR: Conflict Detector not found: %s\n' \
            "$DETECTOR_FILE" >&2
        return 1
    fi

    reset_stubs

    # shellcheck source=/dev/null
    source "$DETECTOR_FILE"

    test_public_api_exists

    test_check_missing_argument
    test_check_invalid_plugin_id
    test_check_unknown_plugin

    test_check_no_conflicts
    test_check_detects_conflict
    test_check_detects_reverse_order
    test_check_detects_second_conflict
    test_check_duplicate_plugins

    test_check_plugin_exists_failure
    test_check_conflict_lookup_failure

    test_repeated_sourcing

    printf '\nTests run: %d\n' "$TESTS_RUN"
    printf 'Failures:  %d\n' "$TESTS_FAILED"

    [[ "$TESTS_FAILED" -eq 0 ]]
}

main "$@"
