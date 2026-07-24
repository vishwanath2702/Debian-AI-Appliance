#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : runtime/lib/common.sh
# Purpose    : Provide common DAIA runtime helper functions.
#
# Version    : 1.1.0
# Codename   : Pragna
# License    : GPL-3.0
#
# Runtime API
# -----------
# The canonical DAIA runtime API uses concise, unprefixed
# function names:
#
#   die
#   require_root
#   command_exists
#   require_command
#   utc_timestamp
#   is_true
#
# Temporary daia_* compatibility aliases are provided at the
# end of this file so existing scripts continue to operate
# during the runtime API migration.
#
# This file is intended to be sourced by DAIA scripts.
# It must not be executed as a standalone program.
# ==========================================================

############################################################
# Prevent accidental direct execution
############################################################

if [[ "${BASH_SOURCE[0]}" == "$0" ]]
then
    printf 'ERROR: %s must be sourced, not executed.\n' \
        "${BASH_SOURCE[0]}" >&2

    exit 1
fi

############################################################
# die
#
# Print an error message and terminate the calling script.
#
# When the DAIA logging library has already been loaded,
# log_error is used. Otherwise, a plain error message is
# written to standard error.
#
# Arguments:
#   $1 - Error message
#   $2 - Optional exit code, default 1
#
# Returns:
#   Does not return.
############################################################
die()
{
    local message="$1"
    local exit_code="${2:-1}"

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
#   0 when running as root.
#   Terminates the calling script otherwise.
############################################################
require_root()
{
    if [[ "$(id -u)" -ne 0 ]]
    then
        die "This operation must be run as root."
    fi
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
#   0 when the command exists.
#   1 otherwise.
############################################################
command_exists()
{
    local command_name="$1"

    command -v "$command_name" >/dev/null 2>&1
}

############################################################
# require_command
#
# Verify that a required command is available.
#
# Arguments:
#   $1 - Command name
#
# Returns:
#   0 when the command exists.
#   Terminates the calling script otherwise.
############################################################
require_command()
{
    local command_name="$1"

    if ! command_exists "$command_name"
    then
        die "Required command is unavailable: $command_name"
    fi
}

############################################################
# utc_timestamp
#
# Return the current UTC timestamp in ISO-8601 format.
#
# Arguments:
#   None
#
# Returns:
#   Timestamp on standard output.
############################################################
utc_timestamp()
{
    date -u '+%Y-%m-%dT%H:%M:%SZ'
}

############################################################
# is_true
#
# Interpret a configuration value as a Boolean.
#
# Accepted true values, case-insensitively:
#
#   1
#   true
#   yes
#   on
#   enabled
#
# Arguments:
#   $1 - Value to evaluate
#
# Returns:
#   0 when the value represents true.
#   1 otherwise.
############################################################
is_true()
{
    local value="${1:-}"

    value="${value,,}"

    case "$value" in
        1|true|yes|on|enabled)
            return 0
            ;;

        *)
            return 1
            ;;
    esac
}

############################################################
# Compatibility API
#
# These aliases preserve compatibility with scripts written
# before the DAIA Runtime Coding Standard v1.0 was adopted.
#
# New code must use the canonical unprefixed functions above.
#
# The aliases can be removed after every existing script has
# been migrated and the full installation pipeline has passed
# regression testing.
############################################################

daia_die()
{
    die "$@"
}

daia_require_root()
{
    require_root "$@"
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
    utc_timestamp "$@"
}

daia_is_true()
{
    is_true "$@"
}

############################################################
# End of File
############################################################
