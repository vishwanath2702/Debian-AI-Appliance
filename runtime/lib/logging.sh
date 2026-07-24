#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : runtime/lib/logging.sh
# Purpose    : Provide standardized runtime logging functions.
#
# Version    : 1.0.0
# Codename   : Pragna
# License    : GPL-3.0
#
# This file is intended to be sourced by DAIA runtime scripts.
# It must not be executed directly.
# ==========================================================

############################################################
# log_message
#
# Print a timestamped log message.
#
# Arguments:
#   $1 - Log level
#   $2 - Message text
#
# Returns:
#   0
############################################################
log_message()
{
    local level="$1"
    local message="$2"
    local timestamp

    timestamp="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"

    printf '%s [%s] %s\n' \
        "$timestamp" \
        "$level" \
        "$message"
}

############################################################
# log_header
#
# Print a major section header.
#
# Arguments:
#   $1 - Header text
#
# Returns:
#   0
############################################################
log_header()
{
    local message="$1"

    printf '\n'
    printf '%s\n' '============================================================'
    printf ' %s\n' "$message"
    printf '%s\n' '============================================================'
}

############################################################
# log_section
#
# Print a subsection heading.
#
# Arguments:
#   $1 - Section text
#
# Returns:
#   0
############################################################
log_section()
{
    local message="$1"

    printf '\n'
    printf '%s\n' '------------------------------------------------------------'
    printf ' %s\n' "$message"
    printf '%s\n' '------------------------------------------------------------'
}

############################################################
# log_info
#
# Print an informational message.
#
# Arguments:
#   $1 - Message text
#
# Returns:
#   0
############################################################
log_info()
{
    log_message "INFO" "$1"
}

############################################################
# log_success
#
# Print a successful-operation message.
#
# Arguments:
#   $1 - Message text
#
# Returns:
#   0
############################################################
log_success()
{
    log_message "OK" "$1"
}

############################################################
# log_warning
#
# Print a warning message to standard error.
#
# Arguments:
#   $1 - Message text
#
# Returns:
#   0
############################################################
log_warning()
{
    log_message "WARN" "$1" >&2
}

############################################################
# log_error
#
# Print an error message to standard error.
#
# Arguments:
#   $1 - Message text
#
# Returns:
#   0
############################################################
log_error()
{
    log_message "ERROR" "$1" >&2
}

############################################################
# End of File
############################################################
