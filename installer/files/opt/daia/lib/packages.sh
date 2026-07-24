#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : lib/packages.sh
# Purpose    : Provide reusable Debian package-management
#              functions for the DAIA installer.
#
# Version    : 1.0.0
# Codename   : Pragna
# License    : GPL-3.0
#
# Responsibilities
# ----------------
# - Refresh the APT package index.
# - Install Debian packages.
# - Remove Debian packages.
# - Determine whether packages are installed.
# - Read and validate package manifests.
# - Install packages declared in manifests.
# - Verify packages declared in manifests.
#
# Non-Responsibilities
# --------------------
# - Module lifecycle management.
# - systemd service management.
# - Repository configuration.
# - Component-specific configuration.
#
# Public API
# ----------
# - package_update
# - package_install
# - package_remove
# - package_is_installed
# - package_manifest_read
# - package_manifest_install
# - package_manifest_verify
#
# Manifest Format
# ---------------
# - UTF-8 plain text.
# - One Debian package name per line.
# - Blank lines are ignored.
# - Full-line comments beginning with "#" are ignored.
# - Inline comments beginning with "#" are ignored.
# - Duplicate package names are rejected.
# - Shell expressions and variables are not permitted.
#
# Runtime Dependencies
# --------------------
# The following libraries must be sourced first:
#
# - /opt/daia/lib/common.sh
# - /opt/daia/lib/logging.sh
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
# Internal state
############################################################

# Records whether apt-get update has completed successfully
# during the current shell process.
_PACKAGE_INDEX_UPDATED="${_PACKAGE_INDEX_UPDATED:-false}"

############################################################
# _package_log_info
#
# Write an informational package-management message.
#
# Arguments:
#   $1 - Message
#
# Returns:
#   DAIA_SUCCESS
############################################################

_package_log_info()
{
    local message="${1:-}"

    if declare -F log_info >/dev/null 2>&1
    then
        log_info "$message"
    else
        printf 'INFO: %s\n' "$message"
    fi

    return "$DAIA_SUCCESS"
}

############################################################
# _package_log_error
#
# Write a package-management error message.
#
# Arguments:
#   $1 - Message
#
# Returns:
#   DAIA_SUCCESS
############################################################

_package_log_error()
{
    local message="${1:-}"

    if declare -F log_error >/dev/null 2>&1
    then
        log_error "$message"
    else
        printf 'ERROR: %s\n' "$message" >&2
    fi

    return "$DAIA_SUCCESS"
}

############################################################
# _package_log_success
#
# Write a successful package-management message.
#
# Arguments:
#   $1 - Message
#
# Returns:
#   DAIA_SUCCESS
############################################################

_package_log_success()
{
    local message="${1:-}"

    if declare -F log_success >/dev/null 2>&1
    then
        log_success "$message"
    else
        printf 'SUCCESS: %s\n' "$message"
    fi

    return "$DAIA_SUCCESS"
}

############################################################
# _package_runtime_validate
#
# Confirm that the common runtime library has been loaded.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when the runtime is available.
#   DAIA_ERROR otherwise.
############################################################

_package_runtime_validate()
{
    local required_function
    local required_variable

    for required_function in \
        require_root \
        require_command
    do
        if ! declare -F "$required_function" >/dev/null 2>&1
        then
            _package_log_error \
                "Required runtime function is unavailable: ${required_function}"

            return 1
        fi
    done

    for required_variable in \
        DAIA_SUCCESS \
        DAIA_ERROR \
        DAIA_INVALID_ARGUMENT \
        DAIA_NOT_FOUND \
        DAIA_VERIFICATION_FAILED
    do
        if [[ -z "${!required_variable+x}" ]]
        then
            _package_log_error \
                "Required runtime constant is unavailable: ${required_variable}"

            return 1
        fi
    done

    return "$DAIA_SUCCESS"
}

############################################################
# _package_name_is_valid
#
# Validate a Debian binary package name.
#
# Accepted examples:
#
# - curl
# - libgtk-3-0
# - g++
# - libc6:amd64
#
# Arguments:
#   $1 - Package name
#
# Returns:
#   DAIA_SUCCESS when valid.
#   DAIA_INVALID_ARGUMENT otherwise.
############################################################

_package_name_is_valid()
{
    local package_name="${1:-}"

    if [[ "$package_name" =~ ^[a-z0-9][a-z0-9+.-]*(:[a-z0-9][a-z0-9-]*)?$ ]]
    then
        return "$DAIA_SUCCESS"
    fi

    return "$DAIA_INVALID_ARGUMENT"
}

############################################################
# _package_normalize_manifest_line
#
# Normalize one package-manifest line by removing:
#
# - A trailing carriage return.
# - Inline comments.
# - Leading whitespace.
# - Trailing whitespace.
#
# Arguments:
#   $1 - Raw manifest line
#
# Output:
#   Normalized line on standard output.
#
# Returns:
#   DAIA_SUCCESS
############################################################

_package_normalize_manifest_line()
{
    local line="${1:-}"

    line="${line%$'\r'}"
    line="${line%%#*}"

    line="${line#"${line%%[![:space:]]*}"}"
    line="${line%"${line##*[![:space:]]}"}"

    printf '%s\n' "$line"

    return "$DAIA_SUCCESS"
}

############################################################
# _package_arguments_validate
#
# Validate a list of package-name arguments.
#
# Arguments:
#   $@ - One or more package names
#
# Returns:
#   DAIA_SUCCESS when all names are valid.
#   DAIA_INVALID_ARGUMENT otherwise.
############################################################

_package_arguments_validate()
{
    local package_name

    if [[ "$#" -eq 0 ]]
    then
        _package_log_error \
            "At least one package name is required."

        return "$DAIA_INVALID_ARGUMENT"
    fi

    for package_name in "$@"
    do
        if ! _package_name_is_valid "$package_name"
        then
            _package_log_error \
                "Invalid Debian package name: ${package_name}"

            return "$DAIA_INVALID_ARGUMENT"
        fi
    done

    return "$DAIA_SUCCESS"
}

############################################################
# package_update
#
# Refresh the APT package index once during the current shell
# process.
#
# Repeated calls return successfully without refreshing the
# index again after the first successful update.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS on success.
#   DAIA_ERROR on failure.
############################################################

package_update()
{
    if ! _package_runtime_validate
    then
        return "$DAIA_ERROR"
    fi

    require_root

    if ! require_command apt-get
    then
        return "$DAIA_NOT_FOUND"
    fi

    if [[ "$_PACKAGE_INDEX_UPDATED" == "true" ]]
    then
        _package_log_info \
            "APT package index has already been refreshed."

        return "$DAIA_SUCCESS"
    fi

    _package_log_info \
        "Refreshing the APT package index."

    if ! DEBIAN_FRONTEND=noninteractive apt-get update
    then
        _package_log_error \
            "APT package index refresh failed."

        return "$DAIA_ERROR"
    fi

    _PACKAGE_INDEX_UPDATED="true"

    _package_log_success \
        "APT package index refreshed successfully."

    return "$DAIA_SUCCESS"
}

############################################################
# package_is_installed
#
# Determine whether a Debian package is fully installed.
#
# Arguments:
#   $1 - Package name
#
# Returns:
#   DAIA_SUCCESS when installed.
#   DAIA_INVALID_ARGUMENT when the name is invalid.
#   DAIA_NOT_FOUND when the package is not installed or cannot
#   be queried.
############################################################

package_is_installed()
{
    local package_name="${1:-}"
    local package_status

    if [[ -z "$package_name" ]]
    then
        _package_log_error \
            "package_is_installed requires a package name."

        return "$DAIA_INVALID_ARGUMENT"
    fi

    if ! _package_name_is_valid "$package_name"
    then
        _package_log_error \
            "Invalid Debian package name: ${package_name}"

        return "$DAIA_INVALID_ARGUMENT"
    fi

    if ! command -v dpkg-query >/dev/null 2>&1
    then
        _package_log_error \
            "Required command is unavailable: dpkg-query"

        return "$DAIA_NOT_FOUND"
    fi

    if ! package_status="$(
        dpkg-query \
            --show \
            --showformat='${db:Status-Abbrev}' \
            "$package_name" \
            2>/dev/null
    )"
    then
        return "$DAIA_NOT_FOUND"
    fi

    if [[ "$package_status" == "ii " ]]
    then
        return "$DAIA_SUCCESS"
    fi

    return "$DAIA_NOT_FOUND"
}

############################################################
# package_install
#
# Install one or more Debian packages.
#
# Packages that are already installed are skipped. The APT
# package index is refreshed automatically when installation
# is required.
#
# Arguments:
#   $@ - One or more package names
#
# Returns:
#   DAIA_SUCCESS when all requested packages are installed.
#   DAIA_INVALID_ARGUMENT when a package name is invalid.
#   DAIA_ERROR when installation fails.
#   DAIA_VERIFICATION_FAILED when post-install verification
#   fails.
############################################################

package_install()
{
    local package_name
    local -a packages_to_install=()

    if ! _package_runtime_validate
    then
        return "$DAIA_ERROR"
    fi

    require_root

    if ! require_command apt-get
    then
        return "$DAIA_NOT_FOUND"
    fi

    if ! require_command dpkg-query
    then
        return "$DAIA_NOT_FOUND"
    fi

    if ! _package_arguments_validate "$@"
    then
        return "$DAIA_INVALID_ARGUMENT"
    fi

    for package_name in "$@"
    do
        if package_is_installed "$package_name"
        then
            _package_log_info \
                "Package is already installed: ${package_name}"
        else
            packages_to_install+=("$package_name")
        fi
    done

    if [[ "${#packages_to_install[@]}" -eq 0 ]]
    then
        _package_log_success \
            "All requested packages are already installed."

        return "$DAIA_SUCCESS"
    fi

    if ! package_update
    then
        return "$DAIA_ERROR"
    fi

    _package_log_info \
        "Installing ${#packages_to_install[@]} package(s)."

    if ! DEBIAN_FRONTEND=noninteractive \
        apt-get install \
            --yes \
            --no-install-recommends \
            "${packages_to_install[@]}"
    then
        _package_log_error \
            "Package installation failed."

        return "$DAIA_ERROR"
    fi

    for package_name in "${packages_to_install[@]}"
    do
        if ! package_is_installed "$package_name"
        then
            _package_log_error \
                "Package verification failed after installation: ${package_name}"

            return "$DAIA_VERIFICATION_FAILED"
        fi
    done

    _package_log_success \
        "Requested packages installed successfully."

    return "$DAIA_SUCCESS"
}

############################################################
# package_remove
#
# Remove one or more Debian packages.
#
# Packages that are not installed are skipped. Configuration
# files are retained.
#
# Arguments:
#   $@ - One or more package names
#
# Returns:
#   DAIA_SUCCESS when all requested packages are absent.
#   DAIA_INVALID_ARGUMENT when a package name is invalid.
#   DAIA_ERROR when removal fails.
#   DAIA_VERIFICATION_FAILED when a package remains installed.
############################################################

package_remove()
{
    local package_name
    local -a packages_to_remove=()

    if ! _package_runtime_validate
    then
        return "$DAIA_ERROR"
    fi

    require_root

    if ! require_command apt-get
    then
        return "$DAIA_NOT_FOUND"
    fi

    if ! require_command dpkg-query
    then
        return "$DAIA_NOT_FOUND"
    fi

    if ! _package_arguments_validate "$@"
    then
        return "$DAIA_INVALID_ARGUMENT"
    fi

    for package_name in "$@"
    do
        if package_is_installed "$package_name"
        then
            packages_to_remove+=("$package_name")
        else
            _package_log_info \
                "Package is not installed; skipping: ${package_name}"
        fi
    done

    if [[ "${#packages_to_remove[@]}" -eq 0 ]]
    then
        _package_log_success \
            "None of the requested packages require removal."

        return "$DAIA_SUCCESS"
    fi

    _package_log_info \
        "Removing ${#packages_to_remove[@]} package(s)."

    if ! DEBIAN_FRONTEND=noninteractive \
        apt-get remove \
            --yes \
            "${packages_to_remove[@]}"
    then
        _package_log_error \
            "Package removal failed."

        return "$DAIA_ERROR"
    fi

    for package_name in "${packages_to_remove[@]}"
    do
        if package_is_installed "$package_name"
        then
            _package_log_error \
                "Package remains installed after removal: ${package_name}"

            return "$DAIA_VERIFICATION_FAILED"
        fi
    done

    _package_log_success \
        "Requested packages removed successfully."

    return "$DAIA_SUCCESS"
}

############################################################
# package_manifest_read
#
# Read and validate a package manifest in one pass.
#
# The destination array is cleared before entries are added.
#
# Validation confirms:
#
# - The manifest path is provided.
# - The manifest exists as a regular file.
# - The manifest is readable.
# - At least one active package entry exists.
# - Every active entry is a valid package name.
# - No duplicate package entries exist.
#
# Arguments:
#   $1 - Manifest path
#   $2 - Destination indexed-array variable name
#
# Returns:
#   DAIA_SUCCESS on success.
#   DAIA_INVALID_ARGUMENT for invalid arguments or entries.
#   DAIA_NOT_FOUND when the manifest does not exist.
#   DAIA_ERROR when the manifest cannot be read.
############################################################

package_manifest_read()
{
    local manifest_path="${1:-}"
    local destination_name="${2:-}"
    local raw_line
    local package_name
    local line_number=0
    local package_count=0

    declare -A discovered_packages=()

    if [[ -z "$manifest_path" ]]
    then
        _package_log_error \
            "package_manifest_read requires a manifest path."

        return "$DAIA_INVALID_ARGUMENT"
    fi

    if [[ -z "$destination_name" ]]
    then
        _package_log_error \
            "package_manifest_read requires a destination array name."

        return "$DAIA_INVALID_ARGUMENT"
    fi

    if [[ ! "$destination_name" =~ ^[a-zA-Z_][a-zA-Z0-9_]*$ ]]
    then
        _package_log_error \
            "Invalid destination array name: ${destination_name}"

        return "$DAIA_INVALID_ARGUMENT"
    fi

    if [[ ! -f "$manifest_path" ]]
    then
        _package_log_error \
            "Package manifest does not exist: ${manifest_path}"

        return "$DAIA_NOT_FOUND"
    fi

    if [[ ! -r "$manifest_path" ]]
    then
        _package_log_error \
            "Package manifest is not readable: ${manifest_path}"

        return "$DAIA_ERROR"
    fi

    local -n destination_array="$destination_name"

    destination_array=()

    while IFS= read -r raw_line || [[ -n "$raw_line" ]]
    do
        line_number=$((line_number + 1))

        package_name="$(
            _package_normalize_manifest_line "$raw_line"
        )"

        if [[ -z "$package_name" ]]
        then
            continue
        fi

        if ! _package_name_is_valid "$package_name"
        then
            _package_log_error \
                "Invalid package entry at ${manifest_path}:${line_number}: ${package_name}"

            return "$DAIA_INVALID_ARGUMENT"
        fi

        if [[ -n "${discovered_packages[$package_name]+exists}" ]]
        then
            _package_log_error \
                "Duplicate package at ${manifest_path}:${line_number}: ${package_name}"

            return "$DAIA_INVALID_ARGUMENT"
        fi

        discovered_packages["$package_name"]=1
        destination_array+=("$package_name")
        package_count=$((package_count + 1))
    done < "$manifest_path"

    if [[ "$package_count" -eq 0 ]]
    then
        _package_log_error \
            "Package manifest contains no package entries: ${manifest_path}"

        return "$DAIA_INVALID_ARGUMENT"
    fi

    _package_log_success \
        "Package manifest loaded: ${manifest_path} (${package_count} packages)"

    return "$DAIA_SUCCESS"
}

############################################################
# package_manifest_install
#
# Read a package manifest and install every package declared
# in it.
#
# Arguments:
#   $1 - Manifest path
#
# Returns:
#   DAIA_SUCCESS on success.
#   A package-manifest or package-installation return code on
#   failure.
############################################################

package_manifest_install()
{
    local manifest_path="${1:-}"
    local read_status
    local install_status
    local -a manifest_packages=()

    package_manifest_read \
        "$manifest_path" \
        manifest_packages

    read_status=$?

    if (( read_status != DAIA_SUCCESS ))
    then
        return "$read_status"
    fi

    _package_log_info \
        "Installing package manifest: ${manifest_path}"

    package_install "${manifest_packages[@]}"
    install_status=$?

    if (( install_status != DAIA_SUCCESS ))
    then
        return "$install_status"
    fi

    _package_log_success \
        "Package manifest installed successfully: ${manifest_path}"

    return "$DAIA_SUCCESS"
}

############################################################
# package_manifest_verify
#
# Verify that every package declared in a package manifest is
# installed.
#
# Arguments:
#   $1 - Manifest path
#
# Returns:
#   DAIA_SUCCESS when every package is installed.
#   A package-manifest return code when reading fails.
#   DAIA_VERIFICATION_FAILED when a required package is absent.
############################################################

package_manifest_verify()
{
    local manifest_path="${1:-}"
    local package_name
    local read_status
    local -a manifest_packages=()

    package_manifest_read \
        "$manifest_path" \
        manifest_packages

    read_status=$?

    if (( read_status != DAIA_SUCCESS ))
    then
        return "$read_status"
    fi

    _package_log_info \
        "Verifying package manifest: ${manifest_path}"

    for package_name in "${manifest_packages[@]}"
    do
        if ! package_is_installed "$package_name"
        then
            _package_log_error \
                "Required package is not installed: ${package_name}"

            return "$DAIA_VERIFICATION_FAILED"
        fi
    done

    _package_log_success \
        "Package manifest verification completed successfully: ${manifest_path}"

    return "$DAIA_SUCCESS"
}

############################################################
# End of file
############################################################
