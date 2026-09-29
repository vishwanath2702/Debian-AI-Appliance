#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : runtime/modules/desktop.sh
# Purpose    : Install and verify the DAIA KDE Plasma desktop
#              foundation.
#
# Version    : 1.0.0
# Codename   : Pragna
# License    : GPL-3.0
#
# Responsibilities
# ----------------
# - Validate the desktop package manifest.
# - Verify packages declared by the desktop manifest are installed.
# - Configure SDDM for secure manual login.
# - Enable the SDDM display-manager service.
# - Verify all desktop-manifest packages are installed.
# - Verify the graphical login service is enabled.
#
# Non-Responsibilities
# --------------------
# - DAIA desktop branding.
# - KDE Plasma panel or theme customization.
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

_DESKTOP_MANIFEST_PATH="${_DESKTOP_RUNTIME_DIRECTORY}/manifests/desktop.lst"

_DESKTOP_SDDM_CONFIGURATION_DIRECTORY="/etc/sddm.conf.d"

_DESKTOP_SDDM_SECURITY_CONFIGURATION="${_DESKTOP_SDDM_CONFIGURATION_DIRECTORY}/99-daia-security.conf"

_DESKTOP_SPICE_AUTOSTART_DIRECTORY="/etc/xdg/autostart"
_DESKTOP_SPICE_AUTOSTART_CONFIGURATION="${_DESKTOP_SPICE_AUTOSTART_DIRECTORY}/spice-vdagent.desktop"

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
        package_manifest_validate
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
# Create an SDDM configuration override that explicitly
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
    local state_file="/var/lib/sddm/state.conf"

    require_root
    require_command install
    require_command mktemp

    if ! install \
        --directory \
        --owner=root \
        --group=root \
        --mode=0755 \
        "$_DESKTOP_SDDM_CONFIGURATION_DIRECTORY"
    then
        _desktop_log_error \
            "Failed to create the SDDM configuration directory."

        return 1
    fi

    temporary_file="$(mktemp)" || {
        _desktop_log_error \
            "Failed to create a temporary SDDM configuration file."

        return 1
    }

    if ! printf '%s\n' \
        '# ==========================================================' \
        '# DAIA SDDM security configuration' \
        '#' \
        '# Automatic login is intentionally disabled.' \
        '# Users must authenticate through the graphical login screen.' \
        '# ==========================================================' \
        '' \
        '[Autologin]' \
        'User=' \
        'Relogin=false' \
        > "$temporary_file"
    then
        rm -f -- "$temporary_file"

        _desktop_log_error \
            "Failed to prepare the SDDM security configuration."

        return 1
    fi

    if ! install \
        --owner=root \
        --group=root \
        --mode=0644 \
        "$temporary_file" \
        "$_DESKTOP_SDDM_SECURITY_CONFIGURATION"
    then
        rm -f -- "$temporary_file"

        _desktop_log_error \
            "Failed to install the SDDM security configuration."

        return 1
    fi

    if ! install \
        --directory \
        --owner=sddm \
        --group=sddm \
        --mode=0755 \
        "$(dirname "$state_file")"
    then
        rm -f -- "$temporary_file"

        _desktop_log_error \
            "Failed to create the SDDM state directory."

        return 1
    fi

    if ! printf '%s\n' \
        '[Last]' \
        'Session=/usr/share/xsessions/plasmax11.desktop' \
        > "$temporary_file"
    then
        rm -f -- "$temporary_file"

        _desktop_log_error \
            "Failed to prepare the SDDM session state."

        return 1
    fi

    if ! install \
        --owner=sddm \
        --group=sddm \
        --mode=0600 \
        "$temporary_file" \
        "$state_file"
    then
        rm -f -- "$temporary_file"

        _desktop_log_error \
            "Failed to install the SDDM session state."

        return 1
    fi

    rm -f -- "$temporary_file"

    _desktop_log_success \
        "SDDM has been configured for manual login with Plasma X11 preselected."
}

############################################################
# _desktop_configure_spice_vdagent
#
# Install the DAIA XDG autostart definition for the SPICE
# guest session agent. The Debian GNOME-specific WindowManager
# autostart phase is intentionally omitted so that the agent
# starts normally in the supported Plasma X11 session.
#
# Arguments:
#   None
#
# Returns:
#   0 on success.
#   1 on failure.
############################################################

_desktop_configure_spice_vdagent()
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
        "$_DESKTOP_SPICE_AUTOSTART_DIRECTORY"
    then
        _desktop_log_error \
            "Failed to create the desktop autostart directory."

        return 1
    fi

    temporary_file="$(mktemp)" || {
        _desktop_log_error \
            "Failed to create a temporary SPICE autostart file."

        return 1
    }

    if ! printf '%s\n' \
        '[Desktop Entry]' \
        'Name=Spice vdagent' \
        'Comment=Agent for Spice guests' \
        'Exec=/usr/bin/spice-vdagent' \
        'Terminal=false' \
        'Type=Application' \
        'Categories=' \
        'NoDisplay=true' \
        > "$temporary_file"
    then
        rm -f -- "$temporary_file"

        _desktop_log_error \
            "Failed to prepare the SPICE autostart configuration."

        return 1
    fi

    if ! install \
        --owner=root \
        --group=root \
        --mode=0644 \
        "$temporary_file" \
        "$_DESKTOP_SPICE_AUTOSTART_CONFIGURATION"
    then
        rm -f -- "$temporary_file"

        _desktop_log_error \
            "Failed to install the SPICE autostart configuration."

        return 1
    fi

    rm -f -- "$temporary_file"

    _desktop_log_success \
        "SPICE guest session agent configured for Plasma X11."
}

############################################################
# _desktop_verify_spice_vdagent
#
# Verify the DAIA SPICE guest-session autostart definition.
#
# Arguments:
#   None
#
# Returns:
#   0 when the configuration is correct.
#   1 otherwise.
############################################################

_desktop_verify_spice_vdagent()
{
    if [[ ! -f "$_DESKTOP_SPICE_AUTOSTART_CONFIGURATION" ]]
    then
        _desktop_log_error \
            "DAIA SPICE autostart configuration is missing."

        return 1
    fi

    if ! grep \
        --quiet \
        '^Exec=/usr/bin/spice-vdagent$' \
        "$_DESKTOP_SPICE_AUTOSTART_CONFIGURATION"
    then
        _desktop_log_error \
            "DAIA SPICE autostart command is incorrect."

        return 1
    fi

    if grep \
        --quiet \
        '^X-GNOME-Autostart-Phase=' \
        "$_DESKTOP_SPICE_AUTOSTART_CONFIGURATION"
    then
        _desktop_log_error \
            "DAIA SPICE autostart contains a GNOME-specific phase."

        return 1
    fi

    _desktop_log_success \
        "SPICE guest session agent configuration verified."
}

############################################################
# _desktop_enable_sddm
#
# Enable SDDM so that the graphical login screen starts
# during normal system boot.
#
# Arguments:
#   None
#
# Returns:
#   0 on success.
#   1 on failure.
############################################################

_desktop_enable_sddm()
{
    require_root
    require_command systemctl

    _desktop_log_info \
        "Enabling the SDDM display-manager service."

    if ! systemctl enable sddm.service
    then
        _desktop_log_error \
            "Failed to enable the SDDM service."

        return 1
    fi

    _desktop_log_success \
        "SDDM service enabled successfully."
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
# _desktop_verify_sddm_enabled
#
# Verify that SDDM is enabled for system startup.
#
# Arguments:
#   None
#
# Returns:
#   0 when enabled.
#   1 otherwise.
############################################################

_desktop_verify_sddm_enabled()
{
    require_command systemctl

    if ! systemctl is-enabled \
        --quiet \
        sddm.service
    then
        _desktop_log_error \
            "SDDM is not enabled for system startup."

        return 1
    fi

    _desktop_log_success \
        "SDDM is enabled for system startup."
}

############################################################
# _desktop_verify_manual_login
#
# Verify that the DAIA SDDM configuration explicitly
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
    local autologin_relogin_value
    local state_file="/var/lib/sddm/state.conf"
    local session_value

    if [[ ! -f "$_DESKTOP_SDDM_SECURITY_CONFIGURATION" ]]
    then
        _desktop_log_error \
            "DAIA SDDM security configuration is missing."

        return 1
    fi

    autologin_user_value="$(
        awk \
            -F= \
            '
                /^[[:space:]]*User[[:space:]]*=/ {
                    value = $0
                    sub(/^[^=]*=/, "", value)
                    gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
                    print value
                }
            ' \
            "$_DESKTOP_SDDM_SECURITY_CONFIGURATION" |
        tail -n 1
    )"

    autologin_relogin_value="$(
        awk \
            -F= \
            '
                /^[[:space:]]*Relogin[[:space:]]*=/ {
                    value = $0
                    sub(/^[^=]*=/, "", value)
                    gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
                    print value
                }
            ' \
            "$_DESKTOP_SDDM_SECURITY_CONFIGURATION" |
        tail -n 1
    )"

    if [[ -n "$autologin_user_value" ]]
    then
        _desktop_log_error \
            "SDDM automatic login is configured for a user."

        return 1
    fi

    if [[ "$autologin_relogin_value" != "false" ]]
    then
        _desktop_log_error \
            "SDDM automatic relogin is not disabled."

        return 1
    fi

    if [[ ! -f "$state_file" ]]
    then
        _desktop_log_error \
            "SDDM session state is missing."

        return 1
    fi

    session_value="$(
        awk \
            -F= \
            '
                /^[[:space:]]*Session[[:space:]]*=/ {
                    value = $0
                    sub(/^[^=]*=/, "", value)
                    gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
                    print value
                }
            ' \
            "$state_file" |
        tail -n 1
    )"

    if [[ "$session_value" != "/usr/share/xsessions/plasmax11.desktop" ]]
    then
        _desktop_log_error \
            "SDDM Plasma X11 session is not preselected."

        return 1
    fi

    _desktop_log_success \
        "SDDM manual-login policy verified."
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
    require_command grep

    package_manifest_validate \
        "$_DESKTOP_MANIFEST_PATH" ||
        return 1

    _desktop_log_success \
        "DAIA desktop module validation completed successfully."
}

############################################################
# desktop_install
#
# Verify the desktop package manifest, enforce manual login,
# and enable the SDDM service.
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
        "Configuring the DAIA desktop foundation."

    desktop_validate || return 1

    _desktop_verify_manifest_packages || return 1
    _desktop_configure_manual_login || return 1
    _desktop_configure_spice_vdagent || return 1
    _desktop_enable_sddm || return 1

    _desktop_log_success \
        "DAIA desktop foundation configured successfully."
}

############################################################
# desktop_verify
#
# Verify the desktop package installation, SDDM service,
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
    require_command grep

    package_manifest_validate \
        "$_DESKTOP_MANIFEST_PATH" ||
        return 1

    _desktop_verify_manifest_packages || return 1
    _desktop_verify_sddm_enabled || return 1
    _desktop_verify_manual_login || return 1
    _desktop_verify_spice_vdagent || return 1

    _desktop_log_success \
        "DAIA desktop foundation verified successfully."
}

############################################################
# End of File
############################################################
