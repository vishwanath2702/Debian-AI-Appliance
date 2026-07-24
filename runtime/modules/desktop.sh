#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : runtime/modules/desktop.sh
# Purpose    : Install and verify the DAIA XFCE desktop
#              foundation.
#
# Version    : 1.0.0
# Codename   : Pragna
# License    : GPL-3.0
#
# Responsibilities
# ----------------
# - Validate the desktop package manifest.
# - Install packages declared by the desktop manifest.
# - Configure LightDM for secure manual login.
# - Enable the LightDM display-manager service.
# - Verify all desktop-manifest packages are installed.
# - Verify the graphical login service is enabled.
#
# Non-Responsibilities
# --------------------
# - DAIA desktop branding.
# - XFCE panel or theme customization.
# - User-account creation.
# - Automatic login.
# - Docker installation.
# - Ollama installation.
# - Open WebUI installation.
#
# Public API
# ----------
# - desktop_validate
# - desktop_install
# - desktop_verify
#
# Dependencies
# ------------
# The following runtime libraries must be sourced before this
# module:
#
# - runtime/lib/common.sh
# - runtime/lib/logging.sh
# - runtime/lib/packages.sh
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
# Module paths
############################################################

_DESKTOP_MODULE_DIRECTORY="$(
    cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &&
    pwd
)"

_DESKTOP_RUNTIME_DIRECTORY="$(
    cd -- "${_DESKTOP_MODULE_DIRECTORY}/.." &&
    pwd
)"

_DESKTOP_PROJECT_ROOT="$(
    cd -- "${_DESKTOP_RUNTIME_DIRECTORY}/.." &&
    pwd
)"

_DESKTOP_MANIFEST_PATH="${_DESKTOP_PROJECT_ROOT}/payload/packages/manifests/desktop.lst"

_DESKTOP_LIGHTDM_CONFIGURATION_DIRECTORY="/etc/lightdm/lightdm.conf.d"

_DESKTOP_LIGHTDM_SECURITY_CONFIGURATION="${_DESKTOP_LIGHTDM_CONFIGURATION_DIRECTORY}/99-daia-security.conf"

############################################################
# Internal logging helpers
############################################################

_desktop_log_info()
{
    local message="$1"

    if declare -F log_info >/dev/null 2>&1
    then
        log_info "$message"
    else
        printf 'INFO: %s\n' "$message"
    fi
}

_desktop_log_error()
{
    local message="$1"

    if declare -F log_error >/dev/null 2>&1
    then
        log_error "$message"
    else
        printf 'ERROR: %s\n' "$message" >&2
    fi
}

_desktop_log_success()
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
# _desktop_require_function
#
# Verify that a required runtime function exists.
#
# Arguments:
#   $1 - Function name
#
# Returns:
#   0 when the function exists.
#   1 otherwise.
############################################################

_desktop_require_function()
{
    local function_name="${1:-}"

    if [[ -z "$function_name" ]]
    then
        _desktop_log_error \
            "_desktop_require_function requires a function name."

        return 1
    fi

    if ! declare -F "$function_name" >/dev/null 2>&1
    then
        _desktop_log_error \
            "Required runtime function is unavailable: $function_name"

        return 1
    fi
}

############################################################
# _desktop_require_runtime
#
# Verify that all shared runtime functions required by this
# module are available.
#
# Arguments:
#   None
#
# Returns:
#   0 when all dependencies are available.
#   1 otherwise.
############################################################

_desktop_require_runtime()
{
    local required_function

    for required_function in \
        require_root \
        require_command \
        package_is_installed \
        package_manifest_validate \
        package_manifest_install
    do
        _desktop_require_function "$required_function" ||
            return 1
    done
}

############################################################
# _desktop_manifest_packages
#
# Read active package entries from the desktop manifest.
#
# Blank lines and comments are ignored. The manifest must be
# validated before this helper is called.
#
# Arguments:
#   None
#
# Returns:
#   Package names on standard output.
############################################################

_desktop_manifest_packages()
{
    local raw_line
    local package_name

    while IFS= read -r raw_line || [[ -n "$raw_line" ]]
    do
        raw_line="${raw_line%$'\r'}"
        raw_line="${raw_line%%#*}"

        package_name="${raw_line#"${raw_line%%[![:space:]]*}"}"
        package_name="${package_name%"${package_name##*[![:space:]]}"}"

        if [[ -n "$package_name" ]]
        then
            printf '%s\n' "$package_name"
        fi
    done < "$_DESKTOP_MANIFEST_PATH"
}

############################################################
# _desktop_configure_manual_login
#
# Create a LightDM configuration override that explicitly
# disables automatic login.
#
# Arguments:
#   None
#
# Returns:
#   0 on success.
#   1 on failure.
############################################################

_desktop_configure_manual_login()
{
    local temporary_file

    require_root
    require_command install
    require_command mktemp

    if ! install \
        --directory \
        --owner=root \
        --group=root \
        --mode=0755 \
        "$_DESKTOP_LIGHTDM_CONFIGURATION_DIRECTORY"
    then
        _desktop_log_error \
            "Failed to create the LightDM configuration directory."

        return 1
    fi

    temporary_file="$(mktemp)" || {
        _desktop_log_error \
            "Failed to create a temporary LightDM configuration file."

        return 1
    }

    if ! printf '%s\n' \
        '# ==========================================================' \
        '# DAIA LightDM security configuration' \
        '#' \
        '# Automatic login is intentionally disabled.' \
        '# Users must authenticate through the graphical login screen.' \
        '# ==========================================================' \
        '' \
        '[Seat:*]' \
        'autologin-user=' \
        'autologin-user-timeout=0' \
        > "$temporary_file"
    then
        rm -f -- "$temporary_file"

        _desktop_log_error \
            "Failed to prepare the LightDM security configuration."

        return 1
    fi

    if ! install \
        --owner=root \
        --group=root \
        --mode=0644 \
        "$temporary_file" \
        "$_DESKTOP_LIGHTDM_SECURITY_CONFIGURATION"
    then
        rm -f -- "$temporary_file"

        _desktop_log_error \
            "Failed to install the LightDM security configuration."

        return 1
    fi

    rm -f -- "$temporary_file"

    _desktop_log_success \
        "LightDM has been configured for manual login."
}

############################################################
# _desktop_enable_lightdm
#
# Enable LightDM so that the graphical login screen starts
# during normal system boot.
#
# Arguments:
#   None
#
# Returns:
#   0 on success.
#   1 on failure.
############################################################

_desktop_enable_lightdm()
{
    require_root
    require_command systemctl

    _desktop_log_info \
        "Enabling the LightDM display-manager service."

    if ! systemctl enable lightdm.service
    then
        _desktop_log_error \
            "Failed to enable the LightDM service."

        return 1
    fi

    _desktop_log_success \
        "LightDM service enabled successfully."
}

############################################################
# _desktop_verify_manifest_packages
#
# Verify that every package declared by the desktop manifest
# is installed.
#
# Arguments:
#   None
#
# Returns:
#   0 when every package is installed.
#   1 when one or more packages are missing.
############################################################

_desktop_verify_manifest_packages()
{
    local package_name
    local missing_package_count=0

    while IFS= read -r package_name
    do
        if package_is_installed "$package_name"
        then
            _desktop_log_info \
                "Desktop package is installed: $package_name"
        else
            _desktop_log_error \
                "Desktop package is not installed: $package_name"

            missing_package_count=$((missing_package_count + 1))
        fi
    done < <(_desktop_manifest_packages)

    if [[ "$missing_package_count" -ne 0 ]]
    then
        _desktop_log_error \
            "$missing_package_count desktop package(s) failed verification."

        return 1
    fi

    _desktop_log_success \
        "All desktop-manifest packages are installed."
}

############################################################
# _desktop_verify_lightdm_enabled
#
# Verify that LightDM is enabled for system startup.
#
# Arguments:
#   None
#
# Returns:
#   0 when enabled.
#   1 otherwise.
############################################################

_desktop_verify_lightdm_enabled()
{
    require_command systemctl

    if ! systemctl is-enabled \
        --quiet \
        lightdm.service
    then
        _desktop_log_error \
            "LightDM is not enabled for system startup."

        return 1
    fi

    _desktop_log_success \
        "LightDM is enabled for system startup."
}

############################################################
# _desktop_verify_manual_login
#
# Verify that the DAIA LightDM configuration explicitly
# disables automatic login.
#
# Arguments:
#   None
#
# Returns:
#   0 when manual-login configuration is present.
#   1 otherwise.
############################################################

_desktop_verify_manual_login()
{
    local autologin_user_value
    local autologin_timeout_value

    if [[ ! -f "$_DESKTOP_LIGHTDM_SECURITY_CONFIGURATION" ]]
    then
        _desktop_log_error \
            "DAIA LightDM security configuration is missing."

        return 1
    fi

    autologin_user_value="$(
        awk \
            -F= \
            '
                /^[[:space:]]*autologin-user[[:space:]]*=/ {
                    value = $0
                    sub(/^[^=]*=/, "", value)
                    gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
                    print value
                }
            ' \
            "$_DESKTOP_LIGHTDM_SECURITY_CONFIGURATION" |
        tail -n 1
    )"

    autologin_timeout_value="$(
        awk \
            -F= \
            '
                /^[[:space:]]*autologin-user-timeout[[:space:]]*=/ {
                    value = $0
                    sub(/^[^=]*=/, "", value)
                    gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
                    print value
                }
            ' \
            "$_DESKTOP_LIGHTDM_SECURITY_CONFIGURATION" |
        tail -n 1
    )"

    if [[ -n "$autologin_user_value" ]]
    then
        _desktop_log_error \
            "LightDM automatic login is configured for a user."

        return 1
    fi

    if [[ "$autologin_timeout_value" != "0" ]]
    then
        _desktop_log_error \
            "LightDM automatic-login timeout is not disabled."

        return 1
    fi

    _desktop_log_success \
        "LightDM manual-login policy verified."
}

############################################################
# desktop_validate
#
# Validate the desktop module's runtime dependencies,
# commands, and package manifest.
#
# Arguments:
#   None
#
# Returns:
#   0 when validation succeeds.
#   1 otherwise.
############################################################

desktop_validate()
{
    _desktop_log_info \
        "Validating the DAIA desktop module."

    _desktop_require_runtime || return 1

    require_root
    require_command apt-get
    require_command dpkg-query
    require_command install
    require_command mktemp
    require_command systemctl
    require_command awk
    require_command tail

    package_manifest_validate \
        "$_DESKTOP_MANIFEST_PATH" ||
        return 1

    _desktop_log_success \
        "DAIA desktop module validation completed successfully."
}

############################################################
# desktop_install
#
# Install the desktop package manifest, enforce manual login,
# and enable the LightDM service.
#
# Arguments:
#   None
#
# Returns:
#   0 on success.
#   1 on failure.
############################################################

desktop_install()
{
    _desktop_log_info \
        "Installing the DAIA desktop foundation."

    desktop_validate || return 1

    package_manifest_install \
        "$_DESKTOP_MANIFEST_PATH" ||
        return 1

    _desktop_configure_manual_login || return 1
    _desktop_enable_lightdm || return 1

    _desktop_log_success \
        "DAIA desktop foundation installed successfully."
}

############################################################
# desktop_verify
#
# Verify the desktop package installation, LightDM service,
# and manual-login security policy.
#
# Arguments:
#   None
#
# Returns:
#   0 when verification succeeds.
#   1 otherwise.
############################################################

desktop_verify()
{
    _desktop_log_info \
        "Verifying the DAIA desktop foundation."

    _desktop_require_runtime || return 1

    require_root
    require_command systemctl
    require_command awk
    require_command tail

    package_manifest_validate \
        "$_DESKTOP_MANIFEST_PATH" ||
        return 1

    _desktop_verify_manifest_packages || return 1
    _desktop_verify_lightdm_enabled || return 1
    _desktop_verify_manual_login || return 1

    _desktop_log_success \
        "DAIA desktop foundation verified successfully."
}

############################################################
# End of File
############################################################
