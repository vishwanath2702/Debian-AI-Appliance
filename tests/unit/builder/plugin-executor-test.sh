#!/usr/bin/env bash
#
# Unit tests for the DAIA Plugin Executor.
#

set -u
set -o pipefail

TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_DIR

REPOSITORY_ROOT="$(cd -- "$TEST_DIR/../../.." && pwd)"
readonly REPOSITORY_ROOT

EXECUTOR_FILE="$REPOSITORY_ROOT/installer/files/opt/daia/builder/plugin-executor.sh"
readonly EXECUTOR_FILE

TESTS_RUN=0
TESTS_FAILED=0
TEST_WORKSPACE=""

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
    printf '  expected: <%s>\n' "$expected" >&2
    printf '  actual:   <%s>\n' "$actual" >&2
    return 1
}

create_workspace() {
    TEST_WORKSPACE="$(mktemp -d)" || return 1

    mkdir -p \
        "$TEST_WORKSPACE/plugins" \
        "$TEST_WORKSPACE/target"
}

remove_workspace() {
    if [[ -n "$TEST_WORKSPACE" && -d "$TEST_WORKSPACE" ]]; then
        rm -rf -- "$TEST_WORKSPACE"
    fi

    TEST_WORKSPACE=""
}

reset_test_environment() {
    daia_plugin_executor_clear
    remove_workspace
    create_workspace
}

write_file() {
    local path="$1"
    shift

    mkdir -p -- "$(dirname -- "$path")" || return 1
    printf '%s\n' "$@" > "$path"
}

create_plugin() {
    local plugin_id="$1"
    shift

    local plugin_directory="$TEST_WORKSPACE/plugins/$plugin_id"
    local install_script="$plugin_directory/install.sh"

    mkdir -p -- "$plugin_directory" || return 1

    write_file \
        "$install_script" \
        '#!/usr/bin/env bash' \
        "$@" || return 1

    chmod +x "$install_script"
}

create_success_plugin() {
    local plugin_id="$1"

    create_plugin \
        "$plugin_id" \
        'printf "%s\n" "$DAIA_PLUGIN_ID" >> "$DAIA_TARGET_ROOT/executed.txt"'
}

initialize_executor() {
    daia_plugin_executor_init \
        "$TEST_WORKSPACE/plugins" \
        "$TEST_WORKSPACE/target"
}

test_executor_requires_initialization() {
    daia_plugin_executor_clear

    ! daia_plugin_executor_validate_plugin "system/base" \
        >/dev/null 2>&1
}

test_executor_initialization_succeeds() {
    reset_test_environment || return 1

    initialize_executor || return 1

    [[ "$__DAIA_PLUGIN_EXECUTOR_INITIALIZED" -eq 1 ]] &&
        [[ "$__DAIA_PLUGIN_EXECUTOR_PLUGIN_ROOT" == \
            "$TEST_WORKSPACE/plugins" ]] &&
        [[ "$__DAIA_PLUGIN_EXECUTOR_TARGET_ROOT" == \
            "$TEST_WORKSPACE/target" ]]
}

test_missing_plugin_root_is_rejected() {
    reset_test_environment || return 1

    ! daia_plugin_executor_init \
        "$TEST_WORKSPACE/missing-plugins" \
        "$TEST_WORKSPACE/target" \
        >/dev/null 2>&1
}

test_missing_target_root_is_rejected() {
    reset_test_environment || return 1

    ! daia_plugin_executor_init \
        "$TEST_WORKSPACE/plugins" \
        "$TEST_WORKSPACE/missing-target" \
        >/dev/null 2>&1
}

test_empty_plugin_root_is_rejected() {
    reset_test_environment || return 1

    ! daia_plugin_executor_init \
        "" \
        "$TEST_WORKSPACE/target" \
        >/dev/null 2>&1
}

test_empty_target_root_is_rejected() {
    reset_test_environment || return 1

    ! daia_plugin_executor_init \
        "$TEST_WORKSPACE/plugins" \
        "" \
        >/dev/null 2>&1
}

test_valid_plugin_executes() {
    local actual

    reset_test_environment || return 1
    create_success_plugin "system/base" || return 1
    initialize_executor || return 1

    daia_plugin_executor_execute_plugin "system/base" \
        >/dev/null || return 1

    actual="$(cat "$TEST_WORKSPACE/target/executed.txt")"

    [[ "$actual" == "system/base" ]]
}

test_plugin_environment_is_exported() {
    local actual
    local expected

    reset_test_environment || return 1

    create_plugin \
        "system/environment-check" \
        'printf "id=%s\n" "$DAIA_PLUGIN_ID" > "$DAIA_TARGET_ROOT/environment.txt"' \
        'printf "dir=%s\n" "$DAIA_PLUGIN_DIR" >> "$DAIA_TARGET_ROOT/environment.txt"' \
        'printf "target=%s\n" "$DAIA_TARGET_ROOT" >> "$DAIA_TARGET_ROOT/environment.txt"' \
        || return 1

    initialize_executor || return 1

    daia_plugin_executor_execute_plugin "system/environment-check" \
        >/dev/null || return 1

    actual="$(cat "$TEST_WORKSPACE/target/environment.txt")"

    expected="$(
        printf 'id=%s\n' "system/environment-check"
        printf 'dir=%s\n' \
            "$TEST_WORKSPACE/plugins/system/environment-check"
        printf 'target=%s\n' "$TEST_WORKSPACE/target"
    )"

    [[ "$actual" == "$expected" ]]
}

test_plan_executes_plugins_in_order() {
    local actual
    local expected

    reset_test_environment || return 1

    create_success_plugin "system/base" || return 1
    create_success_plugin "desktop/window-manager" || return 1
    create_success_plugin "desktop/environment" || return 1

    write_file \
        "$TEST_WORKSPACE/plan.txt" \
        "system/base" \
        "desktop/window-manager" \
        "desktop/environment" \
        || return 1

    initialize_executor || return 1

    daia_plugin_executor_execute_plan "$TEST_WORKSPACE/plan.txt" \
        >/dev/null || return 1

    actual="$(cat "$TEST_WORKSPACE/target/executed.txt")"
    expected=$'system/base\ndesktop/window-manager\ndesktop/environment'

    [[ "$actual" == "$expected" ]]
}

test_missing_plugin_is_rejected() {
    reset_test_environment || return 1
    initialize_executor || return 1

    ! daia_plugin_executor_execute_plugin "system/missing" \
        >/dev/null 2>&1
}

test_missing_install_script_is_rejected() {
    reset_test_environment || return 1

    mkdir -p \
        "$TEST_WORKSPACE/plugins/system/without-installer" \
        || return 1

    initialize_executor || return 1

    ! daia_plugin_executor_validate_plugin \
        "system/without-installer" \
        >/dev/null 2>&1
}

test_invalid_plugin_id_is_rejected() {
    reset_test_environment || return 1
    initialize_executor || return 1

    ! daia_plugin_executor_validate_plugin \
        "Invalid Plugin" \
        >/dev/null 2>&1
}

test_empty_plugin_id_is_rejected() {
    reset_test_environment || return 1
    initialize_executor || return 1

    ! daia_plugin_executor_validate_plugin "" \
        >/dev/null 2>&1
}

test_missing_plan_file_is_rejected() {
    reset_test_environment || return 1
    initialize_executor || return 1

    ! daia_plugin_executor_execute_plan \
        "$TEST_WORKSPACE/missing-plan.txt" \
        >/dev/null 2>&1
}

test_empty_plan_path_is_rejected() {
    reset_test_environment || return 1
    initialize_executor || return 1

    ! daia_plugin_executor_execute_plan "" \
        >/dev/null 2>&1
}

test_duplicate_plan_entry_is_rejected() {
    reset_test_environment || return 1

    create_success_plugin "system/base" || return 1

    write_file \
        "$TEST_WORKSPACE/plan.txt" \
        "system/base" \
        "system/base" \
        || return 1

    initialize_executor || return 1

    ! daia_plugin_executor_execute_plan "$TEST_WORKSPACE/plan.txt" \
        >/dev/null 2>&1
}

test_invalid_plan_entry_is_rejected() {
    reset_test_environment || return 1

    write_file \
        "$TEST_WORKSPACE/plan.txt" \
        "invalid plugin ID" \
        || return 1

    initialize_executor || return 1

    ! daia_plugin_executor_execute_plan "$TEST_WORKSPACE/plan.txt" \
        >/dev/null 2>&1
}

test_plugin_failure_is_propagated() {
    local status

    reset_test_environment || return 1

    create_plugin \
        "system/failing" \
        'exit 23' \
        || return 1

    initialize_executor || return 1

    daia_plugin_executor_execute_plugin "system/failing" \
        >/dev/null 2>&1

    status=$?

    [[ "$status" -eq 23 ]]
}

test_plan_stops_after_plugin_failure() {
    local actual

    reset_test_environment || return 1

    create_success_plugin "system/first" || return 1

    create_plugin \
        "system/failing" \
        'printf "%s\n" "$DAIA_PLUGIN_ID" >> "$DAIA_TARGET_ROOT/executed.txt"' \
        'exit 9' \
        || return 1

    create_success_plugin "system/last" || return 1

    write_file \
        "$TEST_WORKSPACE/plan.txt" \
        "system/first" \
        "system/failing" \
        "system/last" \
        || return 1

    initialize_executor || return 1

    ! daia_plugin_executor_execute_plan "$TEST_WORKSPACE/plan.txt" \
        >/dev/null 2>&1 || return 1

    actual="$(cat "$TEST_WORKSPACE/target/executed.txt")"

    [[ "$actual" == $'system/first\nsystem/failing' ]]
}

test_blank_lines_and_comments_are_ignored() {
    local actual
    local expected

    reset_test_environment || return 1

    create_success_plugin "system/base" || return 1
    create_success_plugin "desktop/environment" || return 1

    write_file \
        "$TEST_WORKSPACE/plan.txt" \
        "# Generated execution plan" \
        "" \
        "system/base" \
        "" \
        "# Desktop plugins" \
        "desktop/environment" \
        "" \
        || return 1

    initialize_executor || return 1

    daia_plugin_executor_execute_plan "$TEST_WORKSPACE/plan.txt" \
        >/dev/null || return 1

    actual="$(cat "$TEST_WORKSPACE/target/executed.txt")"
    expected=$'system/base\ndesktop/environment'

    [[ "$actual" == "$expected" ]]
}

test_clear_resets_executor() {
    reset_test_environment || return 1
    initialize_executor || return 1

    daia_plugin_executor_clear

    [[ "$__DAIA_PLUGIN_EXECUTOR_INITIALIZED" -eq 0 ]] &&
        [[ -z "$__DAIA_PLUGIN_EXECUTOR_PLUGIN_ROOT" ]] &&
        [[ -z "$__DAIA_PLUGIN_EXECUTOR_TARGET_ROOT" ]] &&
        ! daia_plugin_executor_validate_plugin "system/base" \
            >/dev/null 2>&1
}

main() {
    if [[ ! -r "$EXECUTOR_FILE" ]]; then
        fail "plugin executor file not found: $EXECUTOR_FILE"
        exit 1
    fi

    # shellcheck source=/dev/null
    source "$EXECUTOR_FILE"

    trap remove_workspace EXIT

    assert_success \
        "executor operations require initialization" \
        test_executor_requires_initialization || true

    assert_success \
        "executor initialization succeeds" \
        test_executor_initialization_succeeds || true

    assert_success \
        "missing plugin root is rejected" \
        test_missing_plugin_root_is_rejected || true

    assert_success \
        "missing target root is rejected" \
        test_missing_target_root_is_rejected || true

    assert_success \
        "empty plugin root is rejected" \
        test_empty_plugin_root_is_rejected || true

    assert_success \
        "empty target root is rejected" \
        test_empty_target_root_is_rejected || true

    assert_success \
        "valid plugin executes" \
        test_valid_plugin_executes || true

    assert_success \
        "plugin environment is exported" \
        test_plugin_environment_is_exported || true

    assert_success \
        "execution plan preserves plugin order" \
        test_plan_executes_plugins_in_order || true

    assert_success \
        "missing plugin is rejected" \
        test_missing_plugin_is_rejected || true

    assert_success \
        "missing install script is rejected" \
        test_missing_install_script_is_rejected || true

    assert_success \
        "invalid plugin ID is rejected" \
        test_invalid_plugin_id_is_rejected || true

    assert_success \
        "empty plugin ID is rejected" \
        test_empty_plugin_id_is_rejected || true

    assert_success \
        "missing execution plan is rejected" \
        test_missing_plan_file_is_rejected || true

    assert_success \
        "empty execution plan path is rejected" \
        test_empty_plan_path_is_rejected || true

    assert_success \
        "duplicate execution plan entry is rejected" \
        test_duplicate_plan_entry_is_rejected || true

    assert_success \
        "invalid execution plan entry is rejected" \
        test_invalid_plan_entry_is_rejected || true

    assert_success \
        "plugin failure status is propagated" \
        test_plugin_failure_is_propagated || true

    assert_success \
        "execution plan stops after plugin failure" \
        test_plan_stops_after_plugin_failure || true

    assert_success \
        "blank lines and comments are ignored" \
        test_blank_lines_and_comments_are_ignored || true

    assert_success \
        "clear resets the executor" \
        test_clear_resets_executor || true

    remove_workspace

    printf '\nTests run: %d\n' "$TESTS_RUN"
    printf 'Failures:  %d\n' "$TESTS_FAILED"

    [[ "$TESTS_FAILED" -eq 0 ]]
}

main "$@"
