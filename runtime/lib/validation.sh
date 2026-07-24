#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : runtime/lib/validation.sh
# Purpose    : Provide reusable validation helper functions.
#
# Version    : 1.0.0
# Codename   : Pragna
# License    : GPL-3.0
#
# This file is intended to be sourced by DAIA runtime scripts.
# It must not be executed directly.
# ==========================================================

############################################################
# validate_file
#
# Verify that a regular file exists.
#
# Arguments:
#   $1 - File path
#
# Returns:
#   0 when the file exists.
#   1 otherwise.
############################################################
validate_file()
{
    local file_path="$1"

    [[ -f "$file_path" ]]
}

############################################################
# validate_nonempty_file
#
# Verify that a regular file exists and is not empty.
#
# Arguments:
#   $1 - File path
#
# Returns:
#   0 when the file exists and contains data.
#   1 otherwise.
############################################################
validate_nonempty_file()
{
    local file_path="$1"

    [[ -s "$file_path" ]]
}

############################################################
# validate_directory
#
# Verify that a directory exists.
#
# Arguments:
#   $1 - Directory path
#
# Returns:
#   0 when the directory exists.
#   1 otherwise.
############################################################
validate_directory()
{
    local directory_path="$1"

    [[ -d "$directory_path" ]]
}

############################################################
# validate_executable
#
# Verify that a regular file exists and is executable.
#
# Arguments:
#   $1 - File path
#
# Returns:
#   0 when the file exists and is executable.
#   1 otherwise.
############################################################
validate_executable()
{
    local file_path="$1"

    [[ -f "$file_path" && -x "$file_path" ]]
}

############################################################
# validate_command
#
# Verify that a command is available.
#
# Arguments:
#   $1 - Command name
#
# Returns:
#   0 when available.
#   1 otherwise.
############################################################
validate_command()
{
    local command_name="$1"

    command -v "$command_name" >/dev/null 2>&1
}

############################################################
# validate_sha256
#
# Verify a file against an expected SHA-256 checksum.
#
# Arguments:
#   $1 - File path
#   $2 - Expected SHA-256 checksum
#
# Returns:
#   0 when the checksum matches.
#   1 otherwise.
############################################################
validate_sha256()
{
    local file_path="$1"
    local expected_checksum="$2"
    local actual_checksum

    if [[ ! -f "$file_path" ]]
    then
        return 1
    fi

    actual_checksum="$(
        sha256sum "$file_path" |
        awk '{print $1}'
    )"

    [[ "$actual_checksum" == "$expected_checksum" ]]
}

############################################################
# validate_minimum_free_space
#
# Verify that a filesystem has sufficient available space.
#
# Arguments:
#   $1 - Filesystem path
#   $2 - Required free space in kilobytes
#
# Returns:
#   0 when sufficient space is available.
#   1 otherwise.
############################################################
validate_minimum_free_space()
{
    local filesystem_path="$1"
    local required_kilobytes="$2"
    local available_kilobytes

    available_kilobytes="$(
        df --output=avail "$filesystem_path" |
        tail -n 1 |
        tr -d ' '
    )"

    [[ "$available_kilobytes" -ge "$required_kilobytes" ]]
}

############################################################
# End of File
############################################################
