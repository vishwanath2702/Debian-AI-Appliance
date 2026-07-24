#!/usr/bin/env bash
#
# DAIA Logger Unit Tests
#
# Tests the lifecycle, configuration, output routing, level filtering,
# file logging, color handling, and fatal logging behavior provided by:
#
#   installer/files/opt/daia/builder/logger.sh
#

set -u
set -o pipefail

declare -gi TESTS_RUN=0
declare -gi TEST_FAILURES=0

declare -g TEST_NAME=""
declare -g TEST_ROOT=""
declare -g TEST_STDOUT_FILE=""
declare -g TEST_STDERR_FILE=""
declare -g TEST_LOG_FILE=""

fail() {
    local message="${1-assertion failed}"

    printf 'FAIL: %s: %s\n' "$TEST_NAME" "$message" >&2
    TEST_FAILURES=$((TEST_FAILURES + 1))

    return 1
}

pass() {
    printf 'PASS: %s\n' "$TEST_NAME"
}

assert_success() {
    local status="${1-}"
    local message="${2-expected command to succeed}"

    TESTS_RUN=$((TESTS_RUN + 1))

    if [[ "$status" -ne 0 ]]; then
        fail "$message; status was $status"
        return 1
    fi

    return 0
}

assert_failure() {
    local status="${1-}"
    local message="${2-expected command to fail}"

    TESTS_RUN=$((TESTS_RUN + 1))

    if [[ "$status" -eq 0 ]]; then
        fail "$message; command succeeded"
        return 1
    fi

    return 0
}

assert_equals() {
    local expected="${1-}"
    local actual="${2-}"
    local message="${3-values are not equal}"

    TESTS_RUN=$((TESTS_RUN + 1))

    if [[ "$expected" != "$actual" ]]; then
        fail \
            "$message; expected '$expected', got '$actual'"
        return 1
    fi

    return 0
}

assert_not_equals() {
    local unexpected="${1-}"
    local actual="${2-}"
    local message="${3-values should not be equal}"

    TESTS_RUN=$((TESTS_RUN + 1))

    if [[ "$unexpected" == "$actual" ]]; then
        fail \
            "$message; both values were '$actual'"
        return 1
    fi

    return 0
}

assert_empty() {
    local actual="${1-}"
    local message="${2-expected value to be empty}"

    TESTS_RUN=$((TESTS_RUN + 1))

    if [[ -n "$actual" ]]; then
        fail "$message; got '$actual'"
        return 1
    fi

    return 0
}

assert_not_empty() {
    local actual="${1-}"
    local message="${2-expected value not to be empty}"

    TESTS_RUN=$((TESTS_RUN + 1))

    if [[ -z "$actual" ]]; then
        fail "$message"
        return 1
    fi

    return 0
}

assert_contains() {
    local haystack="${1-}"
    local needle="${2-}"
    local message="${3-expected value to contain substring}"

    TESTS_RUN=$((TESTS_RUN + 1))

    if [[ "$haystack" != *"$needle"* ]]; then
        fail \
            "$message; expected to find '$needle' in '$haystack'"
        return 1
    fi

    return 0
}

assert_not_contains() {
    local haystack="${1-}"
    local needle="${2-}"
    local message="${3-expected value not to contain substring}"

    TESTS_RUN=$((TESTS_RUN + 1))

    if [[ "$haystack" == *"$needle"* ]]; then
        fail \
            "$message; unexpectedly found '$needle' in '$haystack'"
        return 1
    fi

    return 0
}

assert_file_exists() {
    local path="${1-}"
    local message="${2-expected file to exist}"

    TESTS_RUN=$((TESTS_RUN + 1))

    if [[ ! -f "$path" ]]; then
        fail "$message: $path"
        return 1
    fi

    return 0
}

assert_file_empty() {
    local path="${1-}"
    local message="${2-expected file to be empty}"

    TESTS_RUN=$((TESTS_RUN + 1))

    if [[ ! -f "$path" ]]; then
        fail "$message; file does not exist: $path"
        return 1
    fi

    if [[ -s "$path" ]]; then
        fail "$message; file contains data: $path"
        return 1
    fi

    return 0
}

assert_file_contains() {
    local path="${1-}"
    local needle="${2-}"
    local message="${3-expected file to contain substring}"

    TESTS_RUN=$((TESTS_RUN + 1))

    if [[ ! -f "$path" ]]; then
        fail "$message; file does not exist: $path"
        return 1
    fi

    if ! grep -Fq -- "$needle" "$path"; then
        fail \
            "$message; expected to find '$needle' in '$path'"
        return 1
    fi

    return 0
}

assert_file_not_contains() {
    local path="${1-}"
    local needle="${2-}"
    local message="${3-expected file not to contain substring}"

    TESTS_RUN=$((TESTS_RUN + 1))

    if [[ ! -f "$path" ]]; then
        fail "$message; file does not exist: $path"
        return 1
    fi

    if grep -Fq -- "$needle" "$path"; then
        fail \
            "$message; unexpectedly found '$needle' in '$path'"
        return 1
    fi

    return 0
}

assert_matches() {
    local actual="${1-}"
    local pattern="${2-}"
    local message="${3-value does not match expected pattern}"

    TESTS_RUN=$((TESTS_RUN + 1))

    if [[ ! "$actual" =~ $pattern ]]; then
        fail \
            "$message; value '$actual' did not match '$pattern'"
        return 1
    fi

    return 0
}

run_test() {
    local test_function="${1-}"

    TEST_NAME="$test_function"

    if "$test_function"; then
        pass
    fi
}
create_test_environment() {
    TEST_ROOT="$(mktemp -d)" || {
        printf 'Could not create temporary test directory\n' >&2
        exit 1
    }

    mkdir -p \
        "$TEST_ROOT/logs" \
        "$TEST_ROOT/output"

    TEST_STDOUT_FILE="$TEST_ROOT/output/stdout.log"
    TEST_STDERR_FILE="$TEST_ROOT/output/stderr.log"
    TEST_LOG_FILE="$TEST_ROOT/logs/daia.log"
}

destroy_test_environment() {
    if [[ -n "${TEST_ROOT:-}" && -d "$TEST_ROOT" ]]; then
        rm -rf "$TEST_ROOT"
    fi
}

reset_logger() {
    __DAIA_LOGGER_INITIALIZED=0
    __DAIA_LOGGER_LEVEL="INFO"
    __DAIA_LOGGER_LOG_FILE=""
    __DAIA_LOGGER_COLORS_ENABLED=0

    : >"$TEST_STDOUT_FILE"
    : >"$TEST_STDERR_FILE"
}

initialize_logger() {
    daia_logger_init >/dev/null 2>&1
}

capture_stdout() {
    "$@" >"$TEST_STDOUT_FILE"
}

capture_stderr() {
    "$@" 2>"$TEST_STDERR_FILE"
}

capture_output() {
    "$@" \
        >"$TEST_STDOUT_FILE" \
        2>"$TEST_STDERR_FILE"
}

stdout_contents() {
    cat "$TEST_STDOUT_FILE"
}

stderr_contents() {
    cat "$TEST_STDERR_FILE"
}

logfile_contents() {
    cat "$TEST_LOG_FILE"
}

setup_test() {
    create_test_environment
    reset_logger
}

teardown_test() {
    destroy_test_environment
}

trap teardown_test EXIT

test_operations_require_initialization() {
    setup_test

    capture_output daia_logger_set_level DEBUG
    assert_failure $? \
        "set_level should require initialization"

    capture_output daia_logger_set_log_file "$TEST_LOG_FILE"
    assert_failure $? \
        "set_log_file should require initialization"

    capture_output daia_logger_clear_log_file
    assert_failure $? \
        "clear_log_file should require initialization"

    capture_output daia_logger_enable_colors
    assert_failure $? \
        "enable_colors should require initialization"

    capture_output daia_logger_disable_colors
    assert_failure $? \
        "disable_colors should require initialization"

    capture_output daia_logger_level
    assert_failure $? \
        "level getter should require initialization"

    capture_output daia_logger_log_file
    assert_failure $? \
        "log_file getter should require initialization"

    capture_output daia_logger_colors_enabled
    assert_failure $? \
        "colors_enabled should require initialization"

    capture_output daia_logger_info "message"
    assert_failure $? \
        "logging should require initialization"

    teardown_test

    return 0
}

test_initialization_succeeds() {
    setup_test

    daia_logger_init >/dev/null 2>&1
    assert_success $? \
        "logger initialization should succeed"

    daia_logger_is_initialized
    assert_success $? \
        "logger should report initialized"

    teardown_test

    return 0
}

test_double_initialization_rejected() {
    setup_test

    initialize_logger

    capture_output daia_logger_init
    assert_failure $? \
        "second initialization should fail"

    teardown_test

    return 0
}

test_clear_before_initialization_rejected() {
    setup_test

    capture_output daia_logger_clear
    assert_failure $? \
        "clear before initialization should fail"

    teardown_test

    return 0
}

test_clear_succeeds() {
    setup_test

    initialize_logger

    capture_output daia_logger_clear
    assert_success $? \
        "clear should succeed"

    daia_logger_is_initialized
    assert_failure $? \
        "logger should no longer be initialized"

    teardown_test

    return 0
}

test_default_configuration() {
    setup_test

    initialize_logger

    assert_equals \
        "INFO" \
        "$(daia_logger_level)" \
        "default log level"

    assert_equals \
        "" \
        "$(daia_logger_log_file)" \
        "default log file"

    daia_logger_colors_enabled
    assert_failure $? \
        "colors should be disabled by default"

    teardown_test

    return 0
}

test_valid_levels_accepted() {
    local level

    setup_test

    initialize_logger

    for level in \
        DEBUG \
        INFO \
        WARN \
        ERROR \
        FATAL
    do
        capture_output \
            daia_logger_set_level "$level"

        assert_success $? \
            "set_level $level"

        assert_equals \
            "$level" \
            "$(daia_logger_level)" \
            "configured level"
    done

    teardown_test

    return 0
}

test_lowercase_levels_normalized() {
    setup_test

    initialize_logger

    capture_output \
        daia_logger_set_level debug

    assert_success $? \
        "lowercase debug should succeed"

    assert_equals \
        "DEBUG" \
        "$(daia_logger_level)" \
        "level should be normalized"

    teardown_test

    return 0
}

test_invalid_level_rejected() {
    setup_test

    initialize_logger

    capture_output \
        daia_logger_set_level TRACE

    assert_failure $? \
        "invalid level should fail"

    assert_equals \
        "INFO" \
        "$(daia_logger_level)" \
        "configured level should remain unchanged"

    teardown_test

    return 0
}

test_debug_and_info_use_stdout() {
    local stdout
    local stderr

    setup_test
    initialize_logger

    daia_logger_set_level DEBUG >/dev/null 2>&1

    capture_output \
        daia_logger_debug "debug message"

    assert_success $? \
        "debug logging should succeed"

    stdout="$(stdout_contents)"
    stderr="$(stderr_contents)"

    assert_contains \
        "$stdout" \
        "[DEBUG] debug message" \
        "debug message should be written to stdout"

    assert_empty \
        "$stderr" \
        "debug message should not be written to stderr"

    capture_output \
        daia_logger_info "info message"

    assert_success $? \
        "info logging should succeed"

    stdout="$(stdout_contents)"
    stderr="$(stderr_contents)"

    assert_contains \
        "$stdout" \
        "[INFO] info message" \
        "info message should be written to stdout"

    assert_empty \
        "$stderr" \
        "info message should not be written to stderr"

    teardown_test

    return 0
}

test_warn_error_and_fatal_use_stderr() {
    local stdout
    local stderr

    setup_test
    initialize_logger

    capture_output \
        daia_logger_warn "warning message"

    assert_success $? \
        "warning logging should succeed"

    stdout="$(stdout_contents)"
    stderr="$(stderr_contents)"

    assert_empty \
        "$stdout" \
        "warning message should not be written to stdout"

    assert_contains \
        "$stderr" \
        "[WARN] warning message" \
        "warning message should be written to stderr"

    capture_output \
        daia_logger_error "error message"

    assert_success $? \
        "error logging should succeed"

    stdout="$(stdout_contents)"
    stderr="$(stderr_contents)"

    assert_empty \
        "$stdout" \
        "error message should not be written to stdout"

    assert_contains \
        "$stderr" \
        "[ERROR] error message" \
        "error message should be written to stderr"

    capture_output \
        daia_logger_fatal "fatal message"

    assert_failure $? \
        "fatal logging should return failure"

    stdout="$(stdout_contents)"
    stderr="$(stderr_contents)"

    assert_empty \
        "$stdout" \
        "fatal message should not be written to stdout"

    assert_contains \
        "$stderr" \
        "[FATAL] fatal message" \
        "fatal message should be written to stderr"

    teardown_test

    return 0
}

test_default_level_filters_debug() {
    local stdout
    local stderr

    setup_test
    initialize_logger

    capture_output \
        daia_logger_debug "filtered debug message"

    assert_success $? \
        "filtered logging should still succeed"

    stdout="$(stdout_contents)"
    stderr="$(stderr_contents)"

    assert_empty \
        "$stdout" \
        "debug should be suppressed at INFO level"

    assert_empty \
        "$stderr" \
        "suppressed debug should not use stderr"

    teardown_test

    return 0
}

test_debug_level_emits_all_levels() {
    local level
    local message
    local combined_output

    setup_test
    initialize_logger

    daia_logger_set_level DEBUG >/dev/null 2>&1

    for level in \
        DEBUG \
        INFO \
        WARN \
        ERROR
    do
        message="message for $level"

        case "$level" in
            DEBUG)
                capture_output \
                    daia_logger_debug "$message"
                ;;
            INFO)
                capture_output \
                    daia_logger_info "$message"
                ;;
            WARN)
                capture_output \
                    daia_logger_warn "$message"
                ;;
            ERROR)
                capture_output \
                    daia_logger_error "$message"
                ;;
        esac

        assert_success $? \
            "$level should be emitted at DEBUG threshold"

        combined_output="$(
            cat "$TEST_STDOUT_FILE" "$TEST_STDERR_FILE"
        )"

        assert_contains \
            "$combined_output" \
            "[$level] $message" \
            "$level output should contain its message"
    done

    teardown_test

    return 0
}

test_warn_level_filters_lower_levels() {
    local stdout
    local stderr

    setup_test
    initialize_logger

    daia_logger_set_level WARN >/dev/null 2>&1

    capture_output \
        daia_logger_debug "debug should be filtered"

    assert_success $? \
        "filtered debug should succeed"

    assert_empty \
        "$(stdout_contents)" \
        "debug should be suppressed at WARN level"

    assert_empty \
        "$(stderr_contents)" \
        "suppressed debug should not use stderr"

    capture_output \
        daia_logger_info "info should be filtered"

    assert_success $? \
        "filtered info should succeed"

    assert_empty \
        "$(stdout_contents)" \
        "info should be suppressed at WARN level"

    assert_empty \
        "$(stderr_contents)" \
        "suppressed info should not use stderr"

    capture_output \
        daia_logger_warn "warn should appear"

    assert_success $? \
        "warning should be emitted at WARN level"

    stdout="$(stdout_contents)"
    stderr="$(stderr_contents)"

    assert_empty \
        "$stdout" \
        "warning should not use stdout"

    assert_contains \
        "$stderr" \
        "[WARN] warn should appear" \
        "warning should be emitted"

    teardown_test

    return 0
}

test_error_level_filters_lower_levels() {
    setup_test
    initialize_logger

    daia_logger_set_level ERROR >/dev/null 2>&1

    capture_output \
        daia_logger_warn "warning should be filtered"

    assert_success $? \
        "filtered warning should succeed"

    assert_empty \
        "$(stdout_contents)" \
        "filtered warning should not use stdout"

    assert_empty \
        "$(stderr_contents)" \
        "warning should be suppressed at ERROR level"

    capture_output \
        daia_logger_error "error should appear"

    assert_success $? \
        "error should be emitted at ERROR level"

    assert_contains \
        "$(stderr_contents)" \
        "[ERROR] error should appear" \
        "error should be emitted"

    teardown_test

    return 0
}

test_fatal_level_filters_nonfatal_messages() {
    setup_test
    initialize_logger

    daia_logger_set_level FATAL >/dev/null 2>&1

    capture_output \
        daia_logger_error "error should be filtered"

    assert_success $? \
        "filtered error should succeed"

    assert_empty \
        "$(stdout_contents)" \
        "filtered error should not use stdout"

    assert_empty \
        "$(stderr_contents)" \
        "error should be suppressed at FATAL level"

    capture_output \
        daia_logger_fatal "fatal should appear"

    assert_failure $? \
        "fatal should return failure"

    assert_contains \
        "$(stderr_contents)" \
        "[FATAL] fatal should appear" \
        "fatal should be emitted"

    teardown_test

    return 0
}

test_log_output_contains_timestamp_and_level() {
    local output
    local timestamp_pattern

    setup_test
    initialize_logger

    capture_output \
        daia_logger_info "timestamp test"

    assert_success $? \
        "info logging should succeed"

    output="$(stdout_contents)"
    timestamp_pattern='^\[[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z\] \[INFO\] timestamp test$'

    assert_matches \
        "$output" \
        "$timestamp_pattern" \
        "output should contain an ISO-8601 UTC timestamp and level"

    teardown_test

    return 0
}

test_multiword_message_preserved() {
    local output

    setup_test
    initialize_logger

    capture_output \
        daia_logger_info \
            "workspace" \
            "preparation" \
            "completed"

    assert_success $? \
        "multiword logging should succeed"

    output="$(stdout_contents)"

    assert_contains \
        "$output" \
        "[INFO] workspace preparation completed" \
        "message arguments should be joined with spaces"

    teardown_test

    return 0
}

test_empty_message_rejected() {
    setup_test
    initialize_logger

    capture_output daia_logger_info

    assert_failure $? \
        "empty log message should fail"

    assert_empty \
        "$(stdout_contents)" \
        "empty message should not be written to stdout"

    assert_contains \
        "$(stderr_contents)" \
        "log message must not be empty" \
        "empty message error should be reported"

    teardown_test

    return 0
}

test_log_file_can_be_configured() {
    local configured_path
    local expected_path

    setup_test
    initialize_logger

    capture_output \
        daia_logger_set_log_file "$TEST_LOG_FILE"

    assert_success $? \
        "setting log file should succeed"

    expected_path="$(
        cd -- "$(dirname -- "$TEST_LOG_FILE")" &&
            printf '%s/%s\n' \
                "$(pwd -P)" \
                "$(basename -- "$TEST_LOG_FILE")"
    )"

    configured_path="$(daia_logger_log_file)"

    assert_equals \
        "$expected_path" \
        "$configured_path" \
        "log file path should be normalized"

    assert_file_exists \
        "$TEST_LOG_FILE" \
        "setting log file should create the file"

    teardown_test

    return 0
}

test_enabled_messages_written_to_log_file() {
    setup_test
    initialize_logger

    daia_logger_set_level DEBUG >/dev/null 2>&1
    daia_logger_set_log_file "$TEST_LOG_FILE" >/dev/null 2>&1

    capture_output \
        daia_logger_debug "file debug message"

    assert_success $? \
        "debug file logging should succeed"

    capture_output \
        daia_logger_info "file info message"

    assert_success $? \
        "info file logging should succeed"

    capture_output \
        daia_logger_warn "file warning message"

    assert_success $? \
        "warning file logging should succeed"

    capture_output \
        daia_logger_error "file error message"

    assert_success $? \
        "error file logging should succeed"

    assert_file_contains \
        "$TEST_LOG_FILE" \
        "[DEBUG] file debug message" \
        "log file should contain debug message"

    assert_file_contains \
        "$TEST_LOG_FILE" \
        "[INFO] file info message" \
        "log file should contain info message"

    assert_file_contains \
        "$TEST_LOG_FILE" \
        "[WARN] file warning message" \
        "log file should contain warning message"

    assert_file_contains \
        "$TEST_LOG_FILE" \
        "[ERROR] file error message" \
        "log file should contain error message"

    teardown_test

    return 0
}

test_filtered_messages_not_written_to_log_file() {
    setup_test
    initialize_logger

    daia_logger_set_log_file "$TEST_LOG_FILE" >/dev/null 2>&1

    capture_output \
        daia_logger_debug "filtered file message"

    assert_success $? \
        "filtered debug logging should succeed"

    assert_file_not_contains \
        "$TEST_LOG_FILE" \
        "filtered file message" \
        "filtered message should not be written to log file"

    assert_file_empty \
        "$TEST_LOG_FILE" \
        "log file should remain empty after filtered message"

    teardown_test

    return 0
}

test_log_file_accumulates_messages() {
    local line_count

    setup_test
    initialize_logger

    daia_logger_set_log_file "$TEST_LOG_FILE" >/dev/null 2>&1

    capture_output \
        daia_logger_info "first file message"

    assert_success $? \
        "first file message should succeed"

    capture_output \
        daia_logger_info "second file message"

    assert_success $? \
        "second file message should succeed"

    line_count="$(wc -l <"$TEST_LOG_FILE")"
    line_count="${line_count//[[:space:]]/}"

    assert_equals \
        "2" \
        "$line_count" \
        "log file should contain both messages"

    assert_file_contains \
        "$TEST_LOG_FILE" \
        "first file message" \
        "log file should retain first message"

    assert_file_contains \
        "$TEST_LOG_FILE" \
        "second file message" \
        "log file should contain second message"

    teardown_test

    return 0
}

test_clear_log_file_configuration() {
    setup_test
    initialize_logger

    daia_logger_set_log_file "$TEST_LOG_FILE" >/dev/null 2>&1

    capture_output daia_logger_clear_log_file

    assert_success $? \
        "clearing log file configuration should succeed"

    assert_equals \
        "" \
        "$(daia_logger_log_file)" \
        "log file getter should be empty after clear"

    capture_output \
        daia_logger_info "terminal only message"

    assert_success $? \
        "logging after clearing log file should succeed"

    assert_file_not_contains \
        "$TEST_LOG_FILE" \
        "terminal only message" \
        "message should not be written after log file is cleared"

    teardown_test

    return 0
}

test_empty_log_file_path_rejected() {
    setup_test
    initialize_logger

    capture_output \
        daia_logger_set_log_file ""

    assert_failure $? \
        "empty log file path should fail"

    assert_contains \
        "$(stderr_contents)" \
        "log file must not be empty" \
        "empty path error should be reported"

    assert_equals \
        "" \
        "$(daia_logger_log_file)" \
        "log file configuration should remain empty"

    teardown_test

    return 0
}

test_missing_log_file_parent_rejected() {
    local invalid_path

    setup_test
    initialize_logger

    invalid_path="$TEST_ROOT/missing/directory/daia.log"

    capture_output \
        daia_logger_set_log_file "$invalid_path"

    assert_failure $? \
        "log file with missing parent should fail"

    assert_contains \
        "$(stderr_contents)" \
        "log file parent directory does not exist" \
        "missing parent error should be reported"

    assert_equals \
        "" \
        "$(daia_logger_log_file)" \
        "invalid log file should not be configured"

    teardown_test

    return 0
}

test_colors_can_be_enabled_and_disabled() {
    setup_test
    initialize_logger

    capture_output daia_logger_enable_colors

    assert_success $? \
        "enabling colors should succeed"

    daia_logger_colors_enabled
    assert_success $? \
        "colors should report enabled"

    capture_output daia_logger_disable_colors

    assert_success $? \
        "disabling colors should succeed"

    daia_logger_colors_enabled
    assert_failure $? \
        "colors should report disabled"

    teardown_test

    return 0
}

test_colored_terminal_output_contains_escape_codes() {
    local output
    local escape_prefix

    setup_test
    initialize_logger

    daia_logger_enable_colors >/dev/null 2>&1

    capture_output \
        daia_logger_info "colored message"

    assert_success $? \
        "colored logging should succeed"

    output="$(stdout_contents)"
    escape_prefix=$'\033['

    assert_contains \
        "$output" \
        "$escape_prefix" \
        "colored terminal output should contain ANSI escape codes"

    assert_contains \
        "$output" \
        "[INFO] colored message" \
        "colored output should retain the formatted message"

    teardown_test

    return 0
}

test_disabled_colors_exclude_escape_codes() {
    local output
    local escape_prefix

    setup_test
    initialize_logger

    capture_output \
        daia_logger_info "plain message"

    assert_success $? \
        "plain logging should succeed"

    output="$(stdout_contents)"
    escape_prefix=$'\033['

    assert_not_contains \
        "$output" \
        "$escape_prefix" \
        "plain terminal output should not contain ANSI escape codes"

    teardown_test

    return 0
}

test_log_file_excludes_color_codes() {
    local escape_prefix

    setup_test
    initialize_logger

    daia_logger_set_log_file "$TEST_LOG_FILE" >/dev/null 2>&1
    daia_logger_enable_colors >/dev/null 2>&1

    capture_output \
        daia_logger_info "colored terminal and plain file"

    assert_success $? \
        "colored logging with file should succeed"

    escape_prefix=$'\033['

    assert_contains \
        "$(stdout_contents)" \
        "$escape_prefix" \
        "terminal output should contain ANSI escape codes"

    assert_file_contains \
        "$TEST_LOG_FILE" \
        "[INFO] colored terminal and plain file" \
        "log file should contain formatted message"

    assert_file_not_contains \
        "$TEST_LOG_FILE" \
        "$escape_prefix" \
        "log file should not contain ANSI escape codes"

    teardown_test

    return 0
}
test_fatal_returns_failure() {
    setup_test

    initialize_logger

    capture_output \
        daia_logger_fatal "fatal build failure"

    assert_failure $? \
        "fatal should return failure"

    assert_contains \
        "$(stderr_contents)" \
        "[FATAL] fatal build failure" \
        "fatal message should be written"

    teardown_test

    return 0
}

test_fatal_does_not_clear_logger() {
    setup_test

    initialize_logger

    capture_output \
        daia_logger_fatal "fatal event"

    assert_failure $? \
        "fatal should return failure"

    daia_logger_is_initialized
    assert_success $? \
        "logger should remain initialized"

    capture_output \
        daia_logger_info "logger still operational"

    assert_success $? \
        "logger should continue operating"

    assert_contains \
        "$(stdout_contents)" \
        "[INFO] logger still operational" \
        "logger should still emit messages"

    teardown_test

    return 0
}

test_fatal_written_to_log_file() {
    setup_test

    initialize_logger

    daia_logger_set_log_file \
        "$TEST_LOG_FILE" >/dev/null 2>&1

    capture_output \
        daia_logger_fatal "fatal file message"

    assert_failure $? \
        "fatal should return failure"

    assert_file_contains \
        "$TEST_LOG_FILE" \
        "[FATAL] fatal file message" \
        "fatal should be written to log file"

    teardown_test

    return 0
}

SCRIPT_DIR="$(
    cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &&
        pwd -P
)"

PROJECT_ROOT="$(
    cd -- "$SCRIPT_DIR/../../.." &&
        pwd -P
)"

source \
    "$PROJECT_ROOT/installer/files/opt/daia/builder/logger.sh"

run_test test_operations_require_initialization

run_test test_initialization_succeeds
run_test test_double_initialization_rejected
run_test test_clear_before_initialization_rejected
run_test test_clear_succeeds

run_test test_default_configuration

run_test test_valid_levels_accepted
run_test test_lowercase_levels_normalized
run_test test_invalid_level_rejected

run_test test_debug_and_info_use_stdout
run_test test_warn_error_and_fatal_use_stderr

run_test test_default_level_filters_debug
run_test test_debug_level_emits_all_levels
run_test test_warn_level_filters_lower_levels
run_test test_error_level_filters_lower_levels
run_test test_fatal_level_filters_nonfatal_messages

run_test test_log_output_contains_timestamp_and_level
run_test test_multiword_message_preserved
run_test test_empty_message_rejected

run_test test_log_file_can_be_configured
run_test test_enabled_messages_written_to_log_file
run_test test_filtered_messages_not_written_to_log_file
run_test test_log_file_accumulates_messages
run_test test_clear_log_file_configuration
run_test test_empty_log_file_path_rejected
run_test test_missing_log_file_parent_rejected

run_test test_colors_can_be_enabled_and_disabled
run_test test_colored_terminal_output_contains_escape_codes
run_test test_disabled_colors_exclude_escape_codes
run_test test_log_file_excludes_color_codes

run_test test_fatal_returns_failure
run_test test_fatal_does_not_clear_logger
run_test test_fatal_written_to_log_file

printf '\n'
printf 'Tests run: %d\n' "$TESTS_RUN"
printf 'Failures : %d\n' "$TEST_FAILURES"
printf '\n'

if [[ "$TEST_FAILURES" -eq 0 ]]; then
    printf 'All logger tests passed.\n'
    exit 0
fi

printf 'Logger tests failed.\n'

exit 1


test_fatal_returns_failure() {
    setup_test

    initialize_logger

    capture_output \
        daia_logger_fatal "fatal build failure"

    assert_failure $? \
        "fatal should return failure"

    assert_contains \
        "$(stderr_contents)" \
        "[FATAL] fatal build failure" \
        "fatal message should be written"

    teardown_test

    return 0
}

test_fatal_does_not_clear_logger() {
    setup_test

    initialize_logger

    capture_output \
        daia_logger_fatal "fatal event"

    assert_failure $? \
        "fatal should return failure"

    daia_logger_is_initialized
    assert_success $? \
        "logger should remain initialized"

    capture_output \
        daia_logger_info "logger still operational"

    assert_success $? \
        "logger should continue operating"

    assert_contains \
        "$(stdout_contents)" \
        "[INFO] logger still operational" \
        "logger should still emit messages"

    teardown_test

    return 0
}

test_fatal_written_to_log_file() {
    setup_test

    initialize_logger

    daia_logger_set_log_file \
        "$TEST_LOG_FILE" >/dev/null 2>&1

    capture_output \
        daia_logger_fatal "fatal file message"

    assert_failure $? \
        "fatal should return failure"

    assert_file_contains \
        "$TEST_LOG_FILE" \
        "[FATAL] fatal file message" \
        "fatal should be written to log file"

    teardown_test

    return 0
}

SCRIPT_DIR="$(
    cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &&
        pwd -P
)"

PROJECT_ROOT="$(
    cd -- "$SCRIPT_DIR/../../.." &&
        pwd -P
)"

source \
    "$PROJECT_ROOT/installer/files/opt/daia/builder/logger.sh"

run_test test_operations_require_initialization

run_test test_initialization_succeeds
run_test test_double_initialization_rejected
run_test test_clear_before_initialization_rejected
run_test test_clear_succeeds

run_test test_default_configuration

run_test test_valid_levels_accepted
run_test test_lowercase_levels_normalized
run_test test_invalid_level_rejected

run_test test_debug_and_info_use_stdout
run_test test_warn_error_and_fatal_use_stderr

run_test test_default_level_filters_debug
run_test test_debug_level_emits_all_levels
run_test test_warn_level_filters_lower_levels
run_test test_error_level_filters_lower_levels
run_test test_fatal_level_filters_nonfatal_messages

run_test test_log_output_contains_timestamp_and_level
run_test test_multiword_message_preserved
run_test test_empty_message_rejected

run_test test_log_file_can_be_configured
run_test test_enabled_messages_written_to_log_file
run_test test_filtered_messages_not_written_to_log_file
run_test test_log_file_accumulates_messages
run_test test_clear_log_file_configuration
run_test test_empty_log_file_path_rejected
run_test test_missing_log_file_parent_rejected

run_test test_colors_can_be_enabled_and_disabled
run_test test_colored_terminal_output_contains_escape_codes
run_test test_disabled_colors_exclude_escape_codes
run_test test_log_file_excludes_color_codes

run_test test_fatal_returns_failure
run_test test_fatal_does_not_clear_logger
run_test test_fatal_written_to_log_file

printf '\n'
printf 'Tests run: %d\n' "$TESTS_RUN"
printf 'Failures : %d\n' "$TEST_FAILURES"
printf '\n'

if [[ "$TEST_FAILURES" -eq 0 ]]; then
    printf 'All logger tests passed.\n'
    exit 0
fi

printf 'Logger tests failed.\n'

exit 1
