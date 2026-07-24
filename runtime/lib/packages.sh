#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : runtime/lib/packages.sh
# Purpose    : Provide reusable APT package-management
#              functions for DAIA runtime modules.
#
# Version    : 1.0.0
# Codename   : Pragna
# License    : GPL-3.0
#
# Responsibilities
# ----------------
# - Refresh the APT package index.
# - Install one or more Debian packages.
# - Remove one or more Debian packages.
# - Determine whether a package is installed.
# - Validate DAIA package manifests.
# - Install packages declared in a manifest.
#
# Non-Responsibilities
# --------------------
# - Desktop configuration.
# - Display-manager configuration.
# - Docker configuration.
# - Ollama configuration.
# - Open WebUI configuration.
# - Distribution branding.
#
# Public API
# ----------
# - package_update
# - package_install
# - package_remove
# - package_is_installed
# - package_manifest_validate
# - package_manifest_install
#
# Manifest Format
# ---------------
# - UTF-8 plain text.
# - One package name per line.
# - Blank lines are ignored.
# - Lines beginning with # are comments.
# - Inline comments beginning with # are supported.
# - Shell expressions and variable expansion are forbidden.
# - Duplicate package entries are rejected.
#
# Dependencies
# ------------
# This library expects runtime/lib/common.sh to have been
# sourced before it is used.
#
# runtime/lib/logging.sh is optional. When logging functions
# are unavailable, this library falls back to printf.
#
# This file must be sourced. It must not be executed directly.
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
# Package-library state
############################################################

# Track whether apt-get update has already completed during
# the current shell process.
#
# Modules may safely call package_update independently without
# causing the package index to be refreshed repeatedly.
PACKAGE_INDEX_UPDATED="${PACKAGE_INDEX_UPDATED:-false}"

############################################################
# Internal logging helpers
############################################################

_package_log_info()
{
    local message="$1"

    if declare -F log_info >/dev/null 2>&1
    then
        log_info "$message"
    else
        printf 'INFO: %s\n' "$message"
    fi
}

_package_log_warning()
{
    local message="$1"

    if declare -F log_warning >/dev/null 2>&1
    then
        log_warning "$message"
    elif declare -F log_warn >/dev/null 2>&1
    then
        log_warn "$message"
    else
        printf 'WARNING: %s\n' "$message" >&2
    fi
}

_package_log_error()
{
    local message="$1"

    if declare -F log_error >/dev/null 2>&1
    then
        log_error "$message"
    else
        printf 'ERROR: %s\n' "$message" >&2
    fi
}

_package_log_success()
{
    local message="$1"

    if declare -F log_success >/dev/null 2>&1
    then
        log_success "$message"
    else
        printf 'SUCCESS: %s\n' "$message"
    fi
}

############################################################
# _package_require_runtime
#
# Verify that functions required from common.sh are available.
#
# Arguments:
#   None
#
# Returns:
#   0 when the runtime API is available.
#   1 otherwise.
############################################################

_package_require_runtime()
{
    local required_function

    for required_function in \
        die \
        require_root \
        require_command
    do
        if ! declare -F "$required_function" >/dev/null 2>&1
        then
            _package_log_error \
                "Required runtime function is unavailable: $required_function"

            return 1
        fi
    done
}

############################################################
# _package_name_is_valid
#
# Validate the syntax of a Debian binary package name.
#
# Architecture-qualified names such as:
#
#   libc6:amd64
#
# are accepted.
#
# Arguments:
#   $1 - Package name
#
# Returns:
#   0 when valid.
#   1 otherwise.
############################################################

_package_name_is_valid()
{
    local package_name="$1"

    [[ "$package_name" =~ ^[a-z0-9][a-z0-9+.-]*(:[a-z0-9][a-z0-9-]*)?$ ]]
}

############################################################
# _package_trim_line
#
# Remove comments, carriage returns, and surrounding
# whitespace from one manifest line.
#
# Arguments:
#   $1 - Raw manifest line
#
# Returns:
#   Normalized line on standard output.
############################################################

_package_trim_line()
{
    local line="$1"

    # Remove a trailing carriage return from CRLF files.
    line="${line%$'\r'}"

    # Remove comments.
    line="${line%%#*}"

    # Remove leading whitespace.
    line="${line#"${line%%[![:space:]]*}"}"

    # Remove trailing whitespace.
    line="${line%"${line##*[![:space:]]}"}"

    printf '%s\n' "$line"
}

############################################################
# _package_collect_manifest
#
# Parse a validated manifest and populate a caller-provided
# array with package names.
#
# Arguments:
#   $1 - Manifest path
#   $2 - Name of destination array variable
#
# Returns:
#   0 on success.
############################################################

_package_collect_manifest()
{
    local manifest_path="$1"
    local destination_array_name="$2"
    local raw_line
    local package_name

    # Bash nameref used to populate the caller's array.
    local -n destination_array="$destination_array_name"

    destination_array=()

    while IFS= read -r raw_line || [[ -n "$raw_line" ]]
    do
        package_name="$(_package_trim_line "$raw_line")"

        if [[ -z "$package_name" ]]
        then
            continue
        fi

        destination_array+=("$package_name")
    done < "$manifest_path"
}

############################################################
# package_update
#
# Refresh the APT package index once during the current shell
# process.
#
# Calling this function more than once is safe. Subsequent
# calls return successfully without running apt-get again.
#
# Arguments:
#   None
#
# Returns:
#   0 on success.
#   Non-zero when apt-get update fails.
############################################################

package_update()
{
    _package_require_runtime || return 1

    require_root
    require_command apt-get

    if [[ "$PACKAGE_INDEX_UPDATED" == "true" ]]
    then
        _package_log_info \
            "APT package index has already been refreshed."

        return 0
    fi

    _package_log_info "Refreshing the APT package index."

    if ! DEBIAN_FRONTEND=noninteractive \
        apt-get update
    then
        _package_log_error "APT package index refresh failed."
        return 1
    fi

    PACKAGE_INDEX_UPDATED="true"

    _package_log_success "APT package index refreshed successfully."
}

############################################################
# package_is_installed
#
# Determine whether a Debian package is currently installed.
#
# Arguments:
#   $1 - Package name
#
# Returns:
#   0 when installed.
#   1 when not installed or invalid.
############################################################

package_is_installed()
{
    local package_name="${1:-}"
    local package_status

    if [[ -z "$package_name" ]]
    then
        _package_log_error \
            "package_is_installed requires a package name."

        return 1
    fi

    if ! _package_name_is_valid "$package_name"
    then
        _package_log_error \
            "Invalid Debian package name: $package_name"

        return 1
    fi

    if ! command -v dpkg-query >/dev/null 2>&1
    then
        _package_log_error \
            "Required command is unavailable: dpkg-query"

        return 1
    fi

    package_status="$(
        dpkg-query \
            --show \
            --showformat='${db:Status-Status}' \
            "$package_name" \
            2>/dev/null ||
        true
    )"

    [[ "$package_status" == "installed" ]]
}

############################################################
# package_install
#
# Install one or more Debian packages.
#
# The package index is refreshed automatically when it has not
# already been refreshed during the current installation run.
#
# Arguments:
#   $@ - One or more package names
#
# Returns:
#   0 on success.
#   Non-zero on validation or installation failure.
############################################################

package_install()
{
    local package_name
    local -a packages_to_install=()

    _package_require_runtime || return 1

    require_root
    require_command apt-get

    if [[ "$#" -eq 0 ]]
    then
        _package_log_error \
            "package_install requires at least one package name."

        return 1
    fi

    for package_name in "$@"
    do
        if ! _package_name_is_valid "$package_name"
        then
            _package_log_error \
                "Invalid Debian package name: $package_name"

            return 1
        fi

        if package_is_installed "$package_name"
        then
            _package_log_info \
                "Package is already installed: $package_name"
        else
            packages_to_install+=("$package_name")
        fi
    done

    if [[ "${#packages_to_install[@]}" -eq 0 ]]
    then
        _package_log_success \
            "All requested packages are already installed."

        return 0
    fi

    package_update || return 1

    _package_log_info \
        "Installing ${#packages_to_install[@]} package(s)."

    if ! DEBIAN_FRONTEND=noninteractive \
        apt-get install \
            --yes \
            "${packages_to_install[@]}"
    then
        _package_log_error "Package installation failed."
        return 1
    fi

    for package_name in "${packages_to_install[@]}"
    do
        if ! package_is_installed "$package_name"
        then
            _package_log_error \
                "Package verification failed after installation: $package_name"

            return 1
        fi
    done

    _package_log_success \
        "Requested packages installed successfully."
}

############################################################
# package_remove
#
# Remove one or more Debian packages.
#
# Packages that are not currently installed are skipped.
#
# Arguments:
#   $@ - One or more package names
#
# Returns:
#   0 on success.
#   Non-zero on validation or removal failure.
############################################################

package_remove()
{
    local package_name
    local -a packages_to_remove=()

    _package_require_runtime || return 1

    require_root
    require_command apt-get

    if [[ "$#" -eq 0 ]]
    then
        _package_log_error \
            "package_remove requires at least one package name."

        return 1
    fi

    for package_name in "$@"
    do
        if ! _package_name_is_valid "$package_name"
        then
            _package_log_error \
                "Invalid Debian package name: $package_name"

            return 1
        fi

        if package_is_installed "$package_name"
        then
            packages_to_remove+=("$package_name")
        else
            _package_log_info \
                "Package is not installed; skipping: $package_name"
        fi
    done

    if [[ "${#packages_to_remove[@]}" -eq 0 ]]
    then
        _package_log_success \
            "None of the requested packages require removal."

        return 0
    fi

    _package_log_info \
        "Removing ${#packages_to_remove[@]} package(s)."

    if ! DEBIAN_FRONTEND=noninteractive \
        apt-get remove \
            --yes \
            "${packages_to_remove[@]}"
    then
        _package_log_error "Package removal failed."
        return 1
    fi

    for package_name in "${packages_to_remove[@]}"
    do
        if package_is_installed "$package_name"
        then
            _package_log_error \
                "Package remains installed after removal: $package_name"

            return 1
        fi
    done

    _package_log_success \
        "Requested packages removed successfully."
}

############################################################
# package_manifest_validate
#
# Validate a DAIA package manifest.
#
# Validation checks:
#
# - The manifest exists.
# - The manifest is a regular, non-empty file.
# - Every active entry is a valid Debian package name.
# - The manifest contains at least one package.
# - No package appears more than once.
#
# Arguments:
#   $1 - Manifest path
#
# Returns:
#   0 when valid.
#   1 otherwise.
############################################################

package_manifest_validate()
{
    local manifest_path="${1:-}"
    local raw_line
    local package_name
    local line_number=0
    local package_count=0

    declare -A discovered_packages=()

    if [[ -z "$manifest_path" ]]
    then
        _package_log_error \
            "package_manifest_validate requires a manifest path."

        return 1
    fi

    if [[ ! -f "$manifest_path" ]]
    then
        _package_log_error \
            "Package manifest does not exist: $manifest_path"

        return 1
    fi

    if [[ ! -s "$manifest_path" ]]
    then
        _package_log_error \
            "Package manifest is empty: $manifest_path"

        return 1
    fi

    while IFS= read -r raw_line || [[ -n "$raw_line" ]]
    do
        line_number=$((line_number + 1))
        package_name="$(_package_trim_line "$raw_line")"

        if [[ -z "$package_name" ]]
        then
            continue
        fi

        if ! _package_name_is_valid "$package_name"
        then
            _package_log_error \
                "Invalid package entry at $manifest_path:$line_number: $package_name"

            return 1
        fi

        if [[ -n "${discovered_packages[$package_name]+present}" ]]
        then
            _package_log_error \
                "Duplicate package at $manifest_path:$line_number: $package_name"

            return 1
        fi

        discovered_packages["$package_name"]=1
        package_count=$((package_count + 1))
    done < "$manifest_path"

    if [[ "$package_count" -eq 0 ]]
    then
        _package_log_error \
            "Package manifest contains no package entries: $manifest_path"

        return 1
    fi

    _package_log_success \
        "Package manifest is valid: $manifest_path ($package_count packages)"
}

############################################################
# package_manifest_install
#
# Validate a DAIA package manifest and install every package
# declared in it.
#
# Arguments:
#   $1 - Manifest path
#
# Returns:
#   0 on success.
#   Non-zero on validation or installation failure.
############################################################

package_manifest_install()
{
    local manifest_path="${1:-}"
    local -a manifest_packages=()

    if [[ -z "$manifest_path" ]]
    then
        _package_log_error \
            "package_manifest_install requires a manifest path."

        return 1
    fi

    package_manifest_validate "$manifest_path" || return 1

    _package_collect_manifest \
        "$manifest_path" \
        manifest_packages

    _package_log_info \
        "Installing package manifest: $manifest_path"

    package_install "${manifest_packages[@]}" || return 1

    _package_log_success \
        "Package manifest installed successfully: $manifest_path"
}

############################################################
# End of File
############################################################
