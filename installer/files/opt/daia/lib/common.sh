#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : lib/common.sh
# Purpose    : Provide common runtime constants and reusable
#              shell helper functions for the DAIA installer.
#
# Version    : 1.0.0
# Codename   : Pragna
# License    : GPL-3.0
#
# Responsibilities
# ----------------
# - Define standard DAIA return codes.
# - Report fatal runtime errors.
# - Check whether the process is running as root.
# - Check whether commands are available.
# - Generate UTC timestamps.
# - Interpret Boolean configuration values.
#
# Non-Responsibilities
# --------------------
# - Logging configuration.
# - Package management.
# - Module lifecycle management.
# - systemd service management.
# - Component-specific installation.
#
# Public API
# ----------
# - daia_die
# - require_root
# - command_exists
# - require_command
# - utc_timestamp
# - is_true
#
# Compatibility API
# -----------------
# The following names are retained for existing DAIA scripts:
#
# - daia_require_root
# - daia_command_exists
# - daia_require_command
# - daia_utc_timestamp
# - daia_is_true
#
# This file must be sourced and must not be executed directly.
# ==========================================================

############################################################
# Prevent direct execution
############################################################

if [[ "${BASH_SOURCE[0]}" == "$0" ]]
then
    printf 'ERROR: %s must be sourced, not executed.\n' \
        "${BASH_SOURCE[0]}" >&2

    exit 1
fi

############################################################
# Standard DAIA return codes
############################################################

# Operation completed successfully.
readonly DAIA_SUCCESS=0

# General operation failure.
readonly DAIA_ERROR=1

# A required argument was missing or invalid.
readonly DAIA_INVALID_ARGUMENT=2

# A requested file, command, package, service, or resource
# could not be found.
readonly DAIA_NOT_FOUND=3

# A requested resource already exists.

# An installation or configuration verification failed.
# Public return code consumed by sourced DAIA libraries.
# shellcheck disable=SC2034
readonly DAIA_VERIFICATION_FAILED=5

############################################################
# daia_die
#
# Report a fatal error and terminate the calling process.
#
# The logging library is used when available. A standard-error
# fallback is provided so this function remains usable before
# logging.sh has been loaded.
#
# Arguments:
#   $1 - Error message
#   $2 - Optional exit code; defaults to DAIA_ERROR
#
# Returns:
#   Does not return.
############################################################

daia_die()
{
    local message="${1:-}"
    local exit_code="${2:-$DAIA_ERROR}"

    if [[ -z "$message" ]]
    then
        message="An unspecified fatal error occurred."
    fi

    if [[ ! "$exit_code" =~ ^[0-9]+$ ]] ||
        (( exit_code < 1 || exit_code > 255 ))
    then
        exit_code="$DAIA_ERROR"
    fi

    if declare -F log_error >/dev/null 2>&1
    then
        log_error "$message"
    else
        printf 'ERROR: %s\n' "$message" >&2
    fi

    exit "$exit_code"
}

############################################################
# require_root
#
# Verify that the current process is running as root.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when running as root.
#   Terminates the calling process otherwise.
############################################################

require_root()
{
    if (( EUID != 0 ))
    then
        daia_die \
            "This operation must be run as root." \
            "$DAIA_ERROR"
    fi

    return "$DAIA_SUCCESS"
}

############################################################
# command_exists
#
# Determine whether a command is available through PATH.
#
# Arguments:
#   $1 - Command name
#
# Returns:
#   DAIA_SUCCESS when the command exists.
#   DAIA_INVALID_ARGUMENT when no command was supplied.
#   DAIA_NOT_FOUND when the command is unavailable.
############################################################

command_exists()
{
    local command_name="${1:-}"

    if [[ -z "$command_name" ]]
    then
        return "$DAIA_INVALID_ARGUMENT"
    fi

    if command -v "$command_name" >/dev/null 2>&1
    then
        return "$DAIA_SUCCESS"
    fi

    return "$DAIA_NOT_FOUND"
}

############################################################
# require_command
#
# Verify that a required command is available through PATH.
#
# Arguments:
#   $1 - Command name
#
# Returns:
#   DAIA_SUCCESS when the command exists.
#   DAIA_INVALID_ARGUMENT when no command was supplied.
#   DAIA_NOT_FOUND when the command is unavailable.
#
# This function does not terminate the calling process. The
# caller may decide how the missing command should be handled.
############################################################

require_command()
{
    local command_name="${1:-}"
    local command_status

    if [[ -z "$command_name" ]]
    then
        if declare -F log_error >/dev/null 2>&1
        then
            log_error "require_command requires a command name."
        else
            printf '%s\n' \
                "ERROR: require_command requires a command name." \
                >&2
        fi

        return "$DAIA_INVALID_ARGUMENT"
    fi

    command_exists "$command_name"
    command_status=$?

    if (( command_status == DAIA_SUCCESS ))
    then
        return "$DAIA_SUCCESS"
    fi

    if declare -F log_error >/dev/null 2>&1
    then
        log_error "Required command is unavailable: ${command_name}"
    else
        printf 'ERROR: Required command is unavailable: %s\n' \
            "$command_name" >&2
    fi

    return "$DAIA_NOT_FOUND"
}

############################################################
# utc_timestamp
#
# Return the current UTC timestamp in ISO 8601 format.
#
# Arguments:
#   None
#
# Output:
#   Timestamp in the following format:
#
#   YYYY-MM-DDTHH:MM:SSZ
#
# Returns:
#   DAIA_SUCCESS
############################################################

utc_timestamp()
{
    date -u '+%Y-%m-%dT%H:%M:%SZ'

    return "$DAIA_SUCCESS"
}

############################################################
# is_true
#
# Interpret a configuration value as a Boolean.
#
# Accepted true values are case-insensitive:
#
# - 1
# - true
# - yes
# - on
# - enabled
#
# Arguments:
#   $1 - Value to evaluate
#
# Returns:
#   DAIA_SUCCESS when the value represents true.
#   DAIA_ERROR otherwise.
############################################################

is_true()
{
    local value="${1:-}"

    value="${value,,}"

    case "$value" in
        1|true|yes|on|enabled)
            return "$DAIA_SUCCESS"
            ;;

        *)
            return "$DAIA_ERROR"
            ;;
    esac
}

############################################################
# Compatibility functions
#
# These wrappers preserve the API used by existing DAIA
# scripts while allowing new installer code to use the shorter
# canonical function names.
############################################################

daia_require_root()
{
    require_root
}

daia_command_exists()
{
    command_exists "$@"
}

daia_require_command()
{
    require_command "$@"
}

daia_utc_timestamp()
{
    utc_timestamp
}

daia_is_true()
{
    is_true "$@"
}

############################################################
# End of file
############################################################
