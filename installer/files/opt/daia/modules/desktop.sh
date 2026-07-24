#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : modules/desktop.sh
# Purpose    : Install and configure the DAIA XFCE desktop
#              environment.
#
# Version    : 1.0.0
# Codename   : Pragna
# License    : GPL-3.0
#
# Responsibilities
# ----------------
# - Declare metadata for the desktop module.
# - Validate desktop-specific runtime requirements.
# - Install the desktop package manifest.
# - Enable and start the LightDM display manager.
# - Verify desktop packages and services.
#
# Non-Responsibilities
# --------------------
# - Parsing the enabled-module configuration.
# - Loading installer libraries.
# - Managing the complete installer lifecycle.
# - Implementing generic package management.
# - Implementing generic systemd service management.
#
# Module Lifecycle
# ----------------
# The bootstrap orchestrator calls:
#
#   framework_validate
#   module_validate
#   module_pre_install
#   module_install
#   module_post_install
#   module_verify
#   module_cleanup
#   framework_cleanup
#
# Runtime Dependencies
# --------------------
# The following files must be sourced before this module:
#
# - /opt/daia/config/daia.conf
# - /opt/daia/lib/common.sh
# - /opt/daia/lib/logging.sh
# - /opt/daia/lib/packages.sh
# - /opt/daia/lib/module.sh
#
# This file must be sourced and must not be executed directly.
# ==========================================================

############################################################
# Module metadata
############################################################

# Public module metadata consumed by lib/module.sh after this file is sourced.
# shellcheck disable=SC2034

MODULE_NAME="desktop"

MODULE_DESCRIPTION="XFCE desktop environment"

MODULE_VERSION="1.0.0"

MODULE_MANIFEST="${DAIA_MANIFEST_DIR}/desktop.lst"

MODULE_SERVICES=(
    "lightdm"
)

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
# _desktop_log_info
#
# Write an informational desktop-module message.
#
# Arguments:
#   $1 - Message
#
# Returns:
#   DAIA_SUCCESS
############################################################

_desktop_log_info()
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
# _desktop_log_error
#
# Write a desktop-module error message.
#
# Arguments:
#   $1 - Message
#
# Returns:
#   DAIA_SUCCESS
############################################################

_desktop_log_error()
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
# _desktop_log_success
#
# Write a successful desktop-module message.
#
# Arguments:
#   $1 - Message
#
# Returns:
#   DAIA_SUCCESS
############################################################

_desktop_log_success()
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
# _desktop_function_require
#
# Verify that a required installer function is available.
#
# Arguments:
#   $1 - Function name
#
# Returns:
#   DAIA_SUCCESS when the function exists.
#   DAIA_INVALID_ARGUMENT when no function name is supplied.
#   DAIA_NOT_FOUND when the function is unavailable.
############################################################

_desktop_function_require()
{
    local function_name="${1:-}"

    if [[ -z "$function_name" ]]
    then
        _desktop_log_error \
            "_desktop_function_require requires a function name."

        return "$DAIA_INVALID_ARGUMENT"
    fi

    if ! declare -F "$function_name" >/dev/null 2>&1
    then
        _desktop_log_error \
            "Required desktop-module function is unavailable: ${function_name}"

        return "$DAIA_NOT_FOUND"
    fi

    return "$DAIA_SUCCESS"
}

############################################################
# _desktop_runtime_validate
#
# Confirm that framework functions required by this module
# have already been loaded.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when the runtime is available.
#   A standard DAIA return code otherwise.
############################################################

_desktop_runtime_validate()
{
    local required_function
    local validation_status

    for required_function in \
        require_root \
        require_command \
        framework_install \
        framework_enable_services \
        framework_verify
    do
        _desktop_function_require "$required_function"
        validation_status=$?

        if (( validation_status != DAIA_SUCCESS ))
        then
            return "$validation_status"
        fi
    done

    return "$DAIA_SUCCESS"
}

############################################################
# module_validate
#
# Perform desktop-specific validation before installation.
#
# Generic module metadata, package-manifest, service metadata,
# and lifecycle validation are handled by framework_validate.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when desktop-specific validation succeeds.
#   DAIA_NOT_FOUND when a required command is unavailable.
#   DAIA_ERROR when runtime validation fails.
############################################################

module_validate()
{
    local validation_status

    _desktop_log_info \
        "Validating desktop module requirements."

    _desktop_runtime_validate
    validation_status=$?

    if (( validation_status != DAIA_SUCCESS ))
    then
        return "$validation_status"
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

    if ! require_command systemctl
    then
        return "$DAIA_NOT_FOUND"
    fi

    _desktop_log_success \
        "Desktop module requirements validated successfully."

    return "$DAIA_SUCCESS"
}

############################################################
# module_pre_install
#
# Prepare the system for desktop installation.
#
# The desktop module currently requires no component-specific
# preparation. This lifecycle hook is intentionally retained
# to preserve the standard DAIA module contract.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS
############################################################

module_pre_install()
{
    _desktop_log_info \
        "Preparing desktop module installation."

    return "$DAIA_SUCCESS"
}

############################################################
# module_install
#
# Install the desktop package manifest through the generic
# module framework.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when installation succeeds.
#   A framework or package-library return code on failure.
############################################################

module_install()
{
    local install_status

    _desktop_log_info \
        "Installing the XFCE desktop environment."

    framework_install
    install_status=$?

    if (( install_status != DAIA_SUCCESS ))
    then
        _desktop_log_error \
            "Desktop package installation failed."

        return "$install_status"
    fi

    _desktop_log_success \
        "Desktop packages installed successfully."

    return "$DAIA_SUCCESS"
}

############################################################
# module_post_install
#
# Perform desktop post-installation operations.
#
# The generic framework enables and starts every service
# declared in MODULE_SERVICES. For this module, that service
# is LightDM.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when post-installation succeeds.
#   A framework return code on failure.
############################################################

module_post_install()
{
    local service_status

    _desktop_log_info \
        "Configuring desktop services."

    framework_enable_services
    service_status=$?

    if (( service_status != DAIA_SUCCESS ))
    then
        _desktop_log_error \
            "Desktop service configuration failed."

        return "$service_status"
    fi

    _desktop_log_success \
        "Desktop services configured successfully."

    return "$DAIA_SUCCESS"
}

############################################################
# module_verify
#
# Verify that desktop packages are installed and declared
# services are enabled and active.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when verification succeeds.
#   A framework return code on failure.
############################################################

module_verify()
{
    local verification_status

    _desktop_log_info \
        "Verifying the desktop installation."

    framework_verify
    verification_status=$?

    if (( verification_status != DAIA_SUCCESS ))
    then
        _desktop_log_error \
            "Desktop installation verification failed."

        return "$verification_status"
    fi

    _desktop_log_success \
        "Desktop installation verified successfully."

    return "$DAIA_SUCCESS"
}

############################################################
# module_cleanup
#
# Perform desktop-specific cleanup after installation.
#
# No temporary desktop resources are currently created.
# Generic module metadata and lifecycle functions are removed
# separately by framework_cleanup.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS
############################################################

module_cleanup()
{
    _desktop_log_info \
        "Desktop module cleanup completed."

    return "$DAIA_SUCCESS"
}

############################################################
# End of file
############################################################
