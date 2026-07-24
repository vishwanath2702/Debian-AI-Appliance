#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : install.sh
# Purpose    : Execute the DAIA bootstrap installer and
#              disable the first-boot service after successful

#              completion.
#
# Version    : 1.0.1
# Codename   : Pragna
# License    : GPL-3.0
#
# Responsibilities
# ----------------
# - Load the DAIA configuration.
# - Load shared installer libraries.
# - Require root privileges.
# - Execute bootstrap.sh.
# - Disable the first-boot systemd service.
#
# Non-Responsibilities
# --------------------
# - Reading enabled modules.
# - Loading modules.
# - Installing packages directly.
# - Managing module lifecycle.
# - Configuring AI components.
#
# This file must be executed and must not be sourced.
# ==========================================================

set -euo pipefail

############################################################
# Installer location
############################################################

INSTALL_CONFIG_DIR="$(
    cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1
    pwd -P
)"
readonly INSTALL_CONFIG_DIR

readonly INSTALL_CONFIG_FILE="${INSTALL_CONFIG_DIR}/config/daia.conf"
############################################################
# Load configuration
############################################################

if [[ ! -f "$INSTALL_CONFIG_FILE" ]]
then
    printf 'ERROR: Configuration file does not exist: %s\n' \
        "$INSTALL_CONFIG_FILE" >&2
    exit 1
fi

# shellcheck source=/dev/null
source "$INSTALL_CONFIG_FILE"

############################################################
# Load libraries
############################################################

# shellcheck source=/dev/null
source "${DAIA_LIB_DIR}/common.sh"

# shellcheck source=/dev/null
source "${DAIA_LIB_DIR}/logging.sh"

############################################################
# Constants
############################################################

readonly INSTALL_BOOTSTRAP_SCRIPT="${DAIA_HOME}/bootstrap.sh"
readonly INSTALL_FIRSTBOOT_SERVICE="daia-firstboot.service"

############################################################
# _install_runtime_validate
############################################################

_install_runtime_validate()
{
    local function_name
    local command_name

    for function_name in \
        require_root \
        require_command \
        log_info \
        log_error \
        log_success
    do
        if ! declare -F "$function_name" >/dev/null 2>&1
        then
            printf 'ERROR: Missing required function: %s\n' \
                "$function_name" >&2
            return "$DAIA_ERROR"
        fi
    done

    if [[ ! -x "$INSTALL_BOOTSTRAP_SCRIPT" ]]
    then
        log_error \
            "Bootstrap script is missing or not executable: ${INSTALL_BOOTSTRAP_SCRIPT}"
        return "$DAIA_NOT_FOUND"
    fi

    for command_name in \
        bash \
        systemctl
    do
        if ! require_command "$command_name"
        then
            return "$DAIA_NOT_FOUND"
        fi
    done

    return "$DAIA_SUCCESS"
}

############################################################
# _install_bootstrap
############################################################

_install_bootstrap()
{
    local status

    log_info "Executing DAIA bootstrap."

    if bash "$INSTALL_BOOTSTRAP_SCRIPT"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
    fi

    if (( status != DAIA_SUCCESS ))
    then
        log_error \
            "Bootstrap failed with status ${status}."

        return "$status"
    fi

    log_success \
        "Bootstrap completed successfully."

    return "$DAIA_SUCCESS"
}

############################################################
# _disable_firstboot_service
############################################################

_disable_firstboot_service()
{
    local status

    log_info \
        "Disabling ${INSTALL_FIRSTBOOT_SERVICE}"

    if systemctl disable "$INSTALL_FIRSTBOOT_SERVICE"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
    fi

    if (( status != DAIA_SUCCESS ))
    then
        log_error \
            "Unable to disable ${INSTALL_FIRSTBOOT_SERVICE}"

        return "$status"
    fi

    log_success \
        "${INSTALL_FIRSTBOOT_SERVICE} disabled."

    return "$DAIA_SUCCESS"
}

############################################################
# main
############################################################

main()
{
    local status

    if _install_runtime_validate
    then
        :
    else
        status=$?
        return "$status"
    fi

    require_root

    if _install_bootstrap
    then
        :
    else
        status=$?
        return "$status"
    fi

    if _disable_firstboot_service
    then
        :
    else
        status=$?
        return "$status"
    fi

    log_success \
        "DAIA installation completed successfully."

    return "$DAIA_SUCCESS"
}

main "$@"
