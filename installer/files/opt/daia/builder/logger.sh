#!/usr/bin/env bash
#
# DAIA Logger
#
# Provides structured, level-based logging for DAIA builder modules.
#
# This file is intended to be sourced.
#

declare -g __DAIA_LOGGER_INITIALIZED=0
declare -g __DAIA_LOGGER_LEVEL="INFO"
declare -g __DAIA_LOGGER_LOG_FILE=""
declare -g __DAIA_LOGGER_COLORS_ENABLED=0

declare -grA __DAIA_LOGGER_LEVEL_VALUES=(
    [DEBUG]=10
    [INFO]=20
    [WARN]=30
    [ERROR]=40
    [FATAL]=50
)

readonly __DAIA_LOGGER_COLOR_RESET=$'\033[0m'
readonly __DAIA_LOGGER_COLOR_DEBUG=$'\033[36m'
readonly __DAIA_LOGGER_COLOR_INFO=$'\033[32m'
readonly __DAIA_LOGGER_COLOR_WARN=$'\033[33m'
readonly __DAIA_LOGGER_COLOR_ERROR=$'\033[31m'
readonly __DAIA_LOGGER_COLOR_FATAL=$'\033[1;31m'

_daia_logger_error() {
    printf 'DAIA logger error: %s\n' "$*" >&2
}

_daia_logger_require_initialization() {
    if [[ "$__DAIA_LOGGER_INITIALIZED" -ne 1 ]]; then
        _daia_logger_error "logger is not initialized"
        return 1
    fi

    return 0
}

_daia_logger_normalize_level() {
    local level="${1-}"

    if [[ -z "$level" ]]; then
        _daia_logger_error "log level must not be empty"
        return 1
    fi

    level="${level^^}"

    if [[ ! -v "__DAIA_LOGGER_LEVEL_VALUES[$level]" ]]; then
        _daia_logger_error "unsupported log level: $level"
        return 1
    fi

    printf '%s\n' "$level"
}

_daia_logger_normalize_file() {
    local file="${1-}"
    local parent
    local name
    local normalized_parent

    if [[ -z "$file" ]]; then
        _daia_logger_error "log file must not be empty"
        return 1
    fi

    parent="$(dirname -- "$file")" || return 1
    name="$(basename -- "$file")" || return 1

    if [[ ! -d "$parent" ]]; then
        _daia_logger_error \
            "log file parent directory does not exist: $parent"
        return 1
    fi

    normalized_parent="$(
        cd -- "$parent" &&
            pwd -P
    )" || {
        _daia_logger_error \
            "could not normalize log file parent directory: $parent"
        return 1
    }

    printf '%s/%s\n' "$normalized_parent" "$name"
}

_daia_logger_level_enabled() {
    local level="${1-}"
    local configured_value
    local requested_value

    _daia_logger_require_initialization || return 1

    configured_value="${__DAIA_LOGGER_LEVEL_VALUES[$__DAIA_LOGGER_LEVEL]}"
    requested_value="${__DAIA_LOGGER_LEVEL_VALUES[$level]}"

    [[ "$requested_value" -ge "$configured_value" ]]
}

_daia_logger_level_color() {
    local level="${1-}"

    case "$level" in
        DEBUG)
            printf '%s' "$__DAIA_LOGGER_COLOR_DEBUG"
            ;;
        INFO)
            printf '%s' "$__DAIA_LOGGER_COLOR_INFO"
            ;;
        WARN)
            printf '%s' "$__DAIA_LOGGER_COLOR_WARN"
            ;;
        ERROR)
            printf '%s' "$__DAIA_LOGGER_COLOR_ERROR"
            ;;
        FATAL)
            printf '%s' "$__DAIA_LOGGER_COLOR_FATAL"
            ;;
        *)
            return 1
            ;;
    esac
}

_daia_logger_timestamp() {
    date -u '+%Y-%m-%dT%H:%M:%SZ'
}

_daia_logger_write_terminal() {
    local level="${1-}"
    local line="${2-}"
    local color=""

    if [[ "$__DAIA_LOGGER_COLORS_ENABLED" -eq 1 ]]; then
        color="$(_daia_logger_level_color "$level")" || return 1
    fi

    case "$level" in
        DEBUG|INFO)
            if [[ "$__DAIA_LOGGER_COLORS_ENABLED" -eq 1 ]]; then
                printf '%s%s%s\n' \
                    "$color" \
                    "$line" \
                    "$__DAIA_LOGGER_COLOR_RESET"
            else
                printf '%s\n' "$line"
            fi
            ;;
        WARN|ERROR|FATAL)
            if [[ "$__DAIA_LOGGER_COLORS_ENABLED" -eq 1 ]]; then
                printf '%s%s%s\n' \
                    "$color" \
                    "$line" \
                    "$__DAIA_LOGGER_COLOR_RESET" >&2
            else
                printf '%s\n' "$line" >&2
            fi
            ;;
        *)
            _daia_logger_error "unsupported log level: $level"
            return 1
            ;;
    esac

    return 0
}

_daia_logger_write_file() {
    local line="${1-}"

    if [[ -z "$__DAIA_LOGGER_LOG_FILE" ]]; then
        return 0
    fi

    if ! printf '%s\n' "$line" >>"$__DAIA_LOGGER_LOG_FILE"; then
        _daia_logger_error \
            "could not write to log file: $__DAIA_LOGGER_LOG_FILE"
        return 1
    fi

    return 0
}

_daia_logger_emit() {
    local level="${1-}"
    shift || true

    local message="$*"
    local timestamp
    local line

    _daia_logger_require_initialization || return 1

    if [[ -z "$message" ]]; then
        _daia_logger_error "log message must not be empty"
        return 1
    fi

    if ! _daia_logger_level_enabled "$level"; then
        return 0
    fi

    timestamp="$(_daia_logger_timestamp)" || {
        _daia_logger_error "could not generate timestamp"
        return 1
    }

    line="[$timestamp] [$level] $message"

    _daia_logger_write_terminal "$level" "$line" || return 1
    _daia_logger_write_file "$line" || return 1

    return 0
}

daia_logger_init() {
    if [[ "$__DAIA_LOGGER_INITIALIZED" -eq 1 ]]; then
        _daia_logger_error "logger is already initialized"
        return 1
    fi

    __DAIA_LOGGER_INITIALIZED=1
    __DAIA_LOGGER_LEVEL="INFO"
    __DAIA_LOGGER_LOG_FILE=""
    __DAIA_LOGGER_COLORS_ENABLED=0

    return 0
}

daia_logger_clear() {
    _daia_logger_require_initialization || return 1

    __DAIA_LOGGER_INITIALIZED=0
    __DAIA_LOGGER_LEVEL="INFO"
    __DAIA_LOGGER_LOG_FILE=""
    __DAIA_LOGGER_COLORS_ENABLED=0

    return 0
}

daia_logger_is_initialized() {
    [[ "$__DAIA_LOGGER_INITIALIZED" -eq 1 ]]
}

daia_logger_set_level() {
    local level="${1-}"

    _daia_logger_require_initialization || return 1

    level="$(_daia_logger_normalize_level "$level")" || return 1

    __DAIA_LOGGER_LEVEL="$level"

    return 0
}

daia_logger_set_log_file() {
    local log_file="${1-}"

    _daia_logger_require_initialization || return 1

    log_file="$(_daia_logger_normalize_file "$log_file")" ||
        return 1

    if ! : >>"$log_file"; then
        _daia_logger_error \
            "log file is not writable: $log_file"
        return 1
    fi

    __DAIA_LOGGER_LOG_FILE="$log_file"

    return 0
}

daia_logger_clear_log_file() {
    _daia_logger_require_initialization || return 1

    __DAIA_LOGGER_LOG_FILE=""

    return 0
}

daia_logger_enable_colors() {
    _daia_logger_require_initialization || return 1

    __DAIA_LOGGER_COLORS_ENABLED=1

    return 0
}

daia_logger_disable_colors() {
    _daia_logger_require_initialization || return 1

    __DAIA_LOGGER_COLORS_ENABLED=0

    return 0
}

daia_logger_level() {
    _daia_logger_require_initialization || return 1

    printf '%s\n' "$__DAIA_LOGGER_LEVEL"
}

daia_logger_log_file() {
    _daia_logger_require_initialization || return 1

    printf '%s\n' "$__DAIA_LOGGER_LOG_FILE"
}

daia_logger_colors_enabled() {
    _daia_logger_require_initialization || return 1

    [[ "$__DAIA_LOGGER_COLORS_ENABLED" -eq 1 ]]
}

daia_logger_debug() {
    _daia_logger_emit DEBUG "$@"
}

daia_logger_info() {
    _daia_logger_emit INFO "$@"
}

daia_logger_warn() {
    _daia_logger_emit WARN "$@"
}

daia_logger_error() {
    _daia_logger_emit ERROR "$@"
}

daia_logger_fatal() {
    _daia_logger_emit FATAL "$@" || return 1

    return 1
}
