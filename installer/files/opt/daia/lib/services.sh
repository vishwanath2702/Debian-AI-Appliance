#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : services.sh
# Purpose    : Provide centralized systemd service-management
#              operations for the DAIA installer framework.
#
# Version    : 1.0
# Codename   : Pragna
# License    : GPL-3.0
#
# Responsibilities
# ----------------
# - Validate systemd service-unit names.
# - Detect whether systemd service units exist.
# - Query service active and enabled states.
# - Enable and disable services.
# - Start, stop, restart, and reload services.
# - Provide idempotent service-management operations.
# - Return standard DAIA status codes.
#
# Non-Responsibilities
# --------------------
# - Deciding which services a module requires.
# - Installing packages that provide services.
# - Defining module-specific service policy.
# - Managing non-service systemd unit types.
# - Logging module lifecycle events.
#
# Dependencies
# ------------
# This library expects the following functions and constants
# to have been loaded before it is sourced:
#
# Functions:
#
#   command_exists
#   log_info
#   log_error
#   log_success
#
# Constants:
#
#   DAIA_SUCCESS
#   DAIA_ERROR
#   DAIA_INVALID_ARGUMENT
#   DAIA_NOT_FOUND
#   DAIA_VERIFICATION_FAILED
#
# Public API
# ----------
#   service_exists
#   service_is_active
#   service_is_enabled
#   service_enable
#   service_disable
#   service_start
#   service_stop
#   service_restart
#   service_reload
#
# This file must be sourced and must not be executed.
# ==========================================================

if [[ "${BASH_SOURCE[0]}" == "$0" ]]
then
    printf 'ERROR: %s must be sourced, not executed.\n' \
        "${BASH_SOURCE[0]}" >&2
    exit 1
fi

############################################################
# Service-library constants
############################################################

readonly DAIA_SYSTEMCTL_COMMAND="systemctl"
readonly DAIA_SYSTEMD_SERVICE_SUFFIX=".service"

############################################################
# _service_dependency_validate
#
# Confirm that functions, constants, and commands required by
# this library are available.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when all dependencies are available.
#   DAIA_ERROR when a required function or constant is absent.
#   DAIA_NOT_FOUND when systemctl is unavailable.
############################################################

_service_dependency_validate()
{
    local required_constant
    local required_function

    for required_function in \
        command_exists \
        log_info \
        log_error \
        log_success
    do
        if ! declare -F "$required_function" >/dev/null 2>&1
        then
            printf 'ERROR: Required services-library function is unavailable: %s\n' \
                "$required_function" >&2

            return "${DAIA_ERROR:-1}"
        fi
    done

    for required_constant in \
        DAIA_SUCCESS \
        DAIA_ERROR \
        DAIA_INVALID_ARGUMENT \
        DAIA_NOT_FOUND \
        DAIA_VERIFICATION_FAILED
    do
        if [[ -z "${!required_constant+x}" ]]
        then
            log_error \
                "Required services-library constant is unavailable: ${required_constant}"

            return "${DAIA_ERROR:-1}"
        fi
    done

    if ! command_exists "$DAIA_SYSTEMCTL_COMMAND"
    then
        log_error \
            "Required service-management command is unavailable: ${DAIA_SYSTEMCTL_COMMAND}"

        return "$DAIA_NOT_FOUND"
    fi

    return "$DAIA_SUCCESS"
}

############################################################
# _service_name_normalize
#
# Normalize a service name by adding the ".service" suffix
# when the caller provides only the base unit name.
#
# Arguments:
#   $1 - Service name
#
# Output:
#   Normalized service-unit name on standard output.
#
# Returns:
#   DAIA_SUCCESS
############################################################

_service_name_normalize()
{
    local service_name="${1:-}"

    if [[ "$service_name" == *"$DAIA_SYSTEMD_SERVICE_SUFFIX" ]]
    then
        printf '%s\n' "$service_name"
    else
        printf '%s%s\n' \
            "$service_name" \
            "$DAIA_SYSTEMD_SERVICE_SUFFIX"
    fi

    return "$DAIA_SUCCESS"
}

############################################################
# _service_name_validate
#
# Validate a systemd service-unit name.
#
# Accepted names:
#
# - May include letters, digits, underscores, periods,
#   colons, at signs, backslashes, and hyphens.
# - May be supplied with or without the ".service" suffix.
# - Must not contain path separators.
# - Must not begin with a hyphen.
# - Must not contain whitespace.
#
# Arguments:
#   $1 - Service name
#
# Returns:
#   DAIA_SUCCESS when valid.
#   DAIA_INVALID_ARGUMENT otherwise.
############################################################

_service_name_validate()
{
    local service_name="${1:-}"

    if [[ -z "$service_name" ]]
    then
        log_error \
            "A service name is required."

        return "$DAIA_INVALID_ARGUMENT"
    fi

    if [[ "$service_name" == -* ]]
    then
        log_error \
            "A service name must not begin with a hyphen: ${service_name}"

        return "$DAIA_INVALID_ARGUMENT"
    fi

    if [[ "$service_name" == */* ]]
    then
        log_error \
            "A service name must not contain a path separator: ${service_name}"

        return "$DAIA_INVALID_ARGUMENT"
    fi

    if [[ "$service_name" =~ [[:space:]] ]]
    then
        log_error \
            "A service name must not contain whitespace: ${service_name}"

        return "$DAIA_INVALID_ARGUMENT"
    fi

    if [[ ! "$service_name" =~ ^[A-Za-z0-9_.:@\\-]+$ ]]
    then
        log_error \
            "Invalid systemd service name: ${service_name}"

        return "$DAIA_INVALID_ARGUMENT"
    fi

    return "$DAIA_SUCCESS"
}

############################################################
# _service_unit_resolve
#
# Validate and normalize a service name.
#
# Arguments:
#   $1 - Service name
#
# Output:
#   Normalized service-unit name on standard output.
#
# Returns:
#   DAIA_SUCCESS when valid.
#   DAIA_INVALID_ARGUMENT otherwise.
############################################################

_service_unit_resolve()
{
    local service_name="${1:-}"

    if ! _service_name_validate "$service_name"
    then
        return "$DAIA_INVALID_ARGUMENT"
    fi

    _service_name_normalize "$service_name"

    return "$DAIA_SUCCESS"
}

############################################################
# _service_unit_load_state
#
# Query the systemd load state for a service unit.
#
# Arguments:
#   $1 - Normalized service-unit name
#
# Output:
#   Unit load state on standard output.
#
# Returns:
#   DAIA_SUCCESS when the query succeeds.
#   DAIA_ERROR when systemctl cannot query the unit.
############################################################

_service_unit_load_state()
{
    local service_unit="${1:-}"
    local load_state

    if load_state="$(
        "$DAIA_SYSTEMCTL_COMMAND" show \
            --property=LoadState \
            --value \
            -- \
            "$service_unit" 2>/dev/null
    )"
    then
        printf '%s\n' "$load_state"
        return "$DAIA_SUCCESS"
    fi

    return "$DAIA_ERROR"
}

############################################################
# _service_require_exists
#
# Verify that a service unit exists before performing an
# operation on it.
#
# Arguments:
#   $1 - Normalized service-unit name
#
# Returns:
#   DAIA_SUCCESS when the unit exists.
#   DAIA_NOT_FOUND when the unit does not exist.
#   DAIA_ERROR when the unit state cannot be queried.
############################################################

_service_require_exists()
{
    local load_state
    local service_unit="${1:-}"
    local status

    if load_state="$(
        _service_unit_load_state "$service_unit"
    )"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
    fi

    if (( status != DAIA_SUCCESS ))
    then
        log_error \
            "Unable to query systemd service unit: ${service_unit}"

        return "$status"
    fi

    case "$load_state" in
        loaded)
            return "$DAIA_SUCCESS"
            ;;

        not-found)
            log_error \
                "Systemd service unit does not exist: ${service_unit}"

            return "$DAIA_NOT_FOUND"
            ;;

        *)
            log_error \
                "Systemd service unit is not loadable: ${service_unit} (${load_state:-unknown})"

            return "$DAIA_ERROR"
            ;;
    esac
}

############################################################
# _service_systemctl_execute
#
# Execute a systemctl action for a normalized service unit.
#
# Arguments:
#   $1 - systemctl action
#   $2 - Normalized service-unit name
#
# Returns:
#   DAIA_SUCCESS when the action succeeds.
#   The systemctl return code when the action fails.
############################################################

_service_systemctl_execute()
{
    local action="${1:-}"
    local service_unit="${2:-}"
    local status

    if "$DAIA_SYSTEMCTL_COMMAND" \
        "$action" \
        -- \
        "$service_unit"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
    fi

    if (( status != DAIA_SUCCESS ))
    then
        log_error \
            "systemctl ${action} failed for ${service_unit} with status ${status}."

        return "$status"
    fi

    return "$DAIA_SUCCESS"
}

############################################################
# service_exists
#
# Determine whether a systemd service unit exists and is
# loadable.
#
# Arguments:
#   $1 - Service name, with or without ".service"
#
# Returns:
#   DAIA_SUCCESS when the service exists.
#   DAIA_INVALID_ARGUMENT when the name is invalid.
#   DAIA_NOT_FOUND when the service does not exist.
#   DAIA_ERROR when systemd cannot query the service.
############################################################

service_exists()
{
    local service_unit
    local status

    if service_unit="$(
        _service_unit_resolve "${1:-}"
    )"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    _service_require_exists "$service_unit"

    return $?
}

############################################################
# service_is_active
#
# Determine whether a systemd service is currently active.
#
# Arguments:
#   $1 - Service name, with or without ".service"
#
# Returns:
#   DAIA_SUCCESS when the service is active.
#   DAIA_VERIFICATION_FAILED when the service exists but is
#   not active.
#   DAIA_INVALID_ARGUMENT when the name is invalid.
#   DAIA_NOT_FOUND when the service does not exist.
#   DAIA_ERROR when systemd cannot query the service.
############################################################

service_is_active()
{
    local service_unit
    local status

    if service_unit="$(
        _service_unit_resolve "${1:-}"
    )"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if _service_require_exists "$service_unit"
    then
        :
    else
        status=$?
        return "$status"
    fi

    if "$DAIA_SYSTEMCTL_COMMAND" \
        is-active \
        --quiet \
        -- \
        "$service_unit"
    then
        return "$DAIA_SUCCESS"
    fi

    return "$DAIA_VERIFICATION_FAILED"
}

############################################################
# service_is_enabled
#
# Determine whether a systemd service is enabled.
#
# Static, generated, transient, indirect, and alias units are
# treated as not enabled because they are not enabled through
# the normal systemd enablement mechanism.
#
# Arguments:
#   $1 - Service name, with or without ".service"
#
# Returns:
#   DAIA_SUCCESS when the service is enabled.
#   DAIA_VERIFICATION_FAILED when the service exists but is
#   not enabled.
#   DAIA_INVALID_ARGUMENT when the name is invalid.
#   DAIA_NOT_FOUND when the service does not exist.
#   DAIA_ERROR when systemd cannot query the service.
############################################################

service_is_enabled()
{
    local service_unit
    local status

    if service_unit="$(
        _service_unit_resolve "${1:-}"
    )"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if _service_require_exists "$service_unit"
    then
        :
    else
        status=$?
        return "$status"
    fi

    if "$DAIA_SYSTEMCTL_COMMAND" \
        is-enabled \
        --quiet \
        -- \
        "$service_unit"
    then
        return "$DAIA_SUCCESS"
    fi

    return "$DAIA_VERIFICATION_FAILED"
}

############################################################
# service_enable
#
# Enable a systemd service.
#
# The operation is idempotent. An already-enabled service is
# treated as success without invoking systemctl enable.
#
# Arguments:
#   $1 - Service name, with or without ".service"
#
# Returns:
#   DAIA_SUCCESS when the service is enabled.
#   DAIA_INVALID_ARGUMENT when the name is invalid.
#   DAIA_NOT_FOUND when the service does not exist.
#   DAIA_ERROR or a systemctl return code on failure.
############################################################

service_enable()
{
    local service_unit
    local status

    if service_unit="$(
        _service_unit_resolve "${1:-}"
    )"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if _service_require_exists "$service_unit"
    then
        :
    else
        status=$?
        return "$status"
    fi

    if service_is_enabled "$service_unit"
    then
        log_info \
            "Systemd service is already enabled: ${service_unit}"

        return "$DAIA_SUCCESS"
    else
        status=$?
    fi

    if (( status != DAIA_VERIFICATION_FAILED ))
    then
        return "$status"
    fi

    log_info \
        "Enabling systemd service: ${service_unit}"

    if _service_systemctl_execute enable "$service_unit"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if ! service_is_enabled "$service_unit"
    then
        status=$?

        if (( status == DAIA_VERIFICATION_FAILED ))
        then
            log_error \
                "Systemd service was not enabled after the enable operation: ${service_unit}"
        fi

        return "$status"
    fi

    log_success \
        "Systemd service enabled: ${service_unit}"

    return "$DAIA_SUCCESS"
}

############################################################
# service_disable
#
# Disable a systemd service.
#
# The operation is idempotent. A service that is already not
# enabled is treated as success.
#
# Arguments:
#   $1 - Service name, with or without ".service"
#
# Returns:
#   DAIA_SUCCESS when the service is disabled.
#   DAIA_INVALID_ARGUMENT when the name is invalid.
#   DAIA_NOT_FOUND when the service does not exist.
#   DAIA_ERROR or a systemctl return code on failure.
############################################################

service_disable()
{
    local service_unit
    local status

    if service_unit="$(
        _service_unit_resolve "${1:-}"
    )"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if _service_require_exists "$service_unit"
    then
        :
    else
        status=$?
        return "$status"
    fi

    if service_is_enabled "$service_unit"
    then
        :
    else
        status=$?

        if (( status == DAIA_VERIFICATION_FAILED ))
        then
            log_info \
                "Systemd service is already disabled: ${service_unit}"

            return "$DAIA_SUCCESS"
        fi

        return "$status"
    fi

    log_info \
        "Disabling systemd service: ${service_unit}"

    if _service_systemctl_execute disable "$service_unit"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if service_is_enabled "$service_unit"
    then
        log_error \
            "Systemd service remained enabled after the disable operation: ${service_unit}"

        return "$DAIA_VERIFICATION_FAILED"
    else
        status=$?
    fi

    if (( status != DAIA_VERIFICATION_FAILED ))
    then
        return "$status"
    fi

    log_success \
        "Systemd service disabled: ${service_unit}"

    return "$DAIA_SUCCESS"
}

############################################################
# service_start
#
# Start a systemd service.
#
# The operation is idempotent. An already-active service is
# treated as success.
#
# Arguments:
#   $1 - Service name, with or without ".service"
#
# Returns:
#   DAIA_SUCCESS when the service is active.
#   DAIA_INVALID_ARGUMENT when the name is invalid.
#   DAIA_NOT_FOUND when the service does not exist.
#   DAIA_ERROR, DAIA_VERIFICATION_FAILED, or a systemctl
#   return code on failure.
############################################################

service_start()
{
    local service_unit
    local status

    if service_unit="$(
        _service_unit_resolve "${1:-}"
    )"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if _service_require_exists "$service_unit"
    then
        :
    else
        status=$?
        return "$status"
    fi

    if service_is_active "$service_unit"
    then
        log_info \
            "Systemd service is already active: ${service_unit}"

        return "$DAIA_SUCCESS"
    else
        status=$?
    fi

    if (( status != DAIA_VERIFICATION_FAILED ))
    then
        return "$status"
    fi

    log_info \
        "Starting systemd service: ${service_unit}"

    if _service_systemctl_execute start "$service_unit"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if ! service_is_active "$service_unit"
    then
        status=$?

        if (( status == DAIA_VERIFICATION_FAILED ))
        then
            log_error \
                "Systemd service is not active after the start operation: ${service_unit}"
        fi

        return "$status"
    fi

    log_success \
        "Systemd service started: ${service_unit}"

    return "$DAIA_SUCCESS"
}

############################################################
# service_stop
#
# Stop a systemd service.
#
# The operation is idempotent. An already-inactive service is
# treated as success.
#
# Arguments:
#   $1 - Service name, with or without ".service"
#
# Returns:
#   DAIA_SUCCESS when the service is inactive.
#   DAIA_INVALID_ARGUMENT when the name is invalid.
#   DAIA_NOT_FOUND when the service does not exist.
#   DAIA_ERROR, DAIA_VERIFICATION_FAILED, or a systemctl
#   return code on failure.
############################################################

service_stop()
{
    local service_unit
    local status

    if service_unit="$(
        _service_unit_resolve "${1:-}"
    )"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if _service_require_exists "$service_unit"
    then
        :
    else
        status=$?
        return "$status"
    fi

    if service_is_active "$service_unit"
    then
        :
    else
        status=$?

        if (( status == DAIA_VERIFICATION_FAILED ))
        then
            log_info \
                "Systemd service is already inactive: ${service_unit}"

            return "$DAIA_SUCCESS"
        fi

        return "$status"
    fi

    log_info \
        "Stopping systemd service: ${service_unit}"

    if _service_systemctl_execute stop "$service_unit"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if service_is_active "$service_unit"
    then
        log_error \
            "Systemd service remained active after the stop operation: ${service_unit}"

        return "$DAIA_VERIFICATION_FAILED"
    else
        status=$?
    fi

    if (( status != DAIA_VERIFICATION_FAILED ))
    then
        return "$status"
    fi

    log_success \
        "Systemd service stopped: ${service_unit}"

    return "$DAIA_SUCCESS"
}

############################################################
# service_restart
#
# Restart a systemd service and verify that it becomes active.
#
# Unlike service_start, restart is always executed because the
# caller explicitly requests a service restart.
#
# Arguments:
#   $1 - Service name, with or without ".service"
#
# Returns:
#   DAIA_SUCCESS when the service restarts and is active.
#   DAIA_INVALID_ARGUMENT when the name is invalid.
#   DAIA_NOT_FOUND when the service does not exist.
#   DAIA_ERROR, DAIA_VERIFICATION_FAILED, or a systemctl
#   return code on failure.
############################################################

service_restart()
{
    local service_unit
    local status

    if service_unit="$(
        _service_unit_resolve "${1:-}"
    )"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if _service_require_exists "$service_unit"
    then
        :
    else
        status=$?
        return "$status"
    fi

    log_info \
        "Restarting systemd service: ${service_unit}"

    if _service_systemctl_execute restart "$service_unit"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if ! service_is_active "$service_unit"
    then
        status=$?

        if (( status == DAIA_VERIFICATION_FAILED ))
        then
            log_error \
                "Systemd service is not active after the restart operation: ${service_unit}"
        fi

        return "$status"
    fi

    log_success \
        "Systemd service restarted: ${service_unit}"

    return "$DAIA_SUCCESS"
}

############################################################
# service_reload
#
# Reload the configuration of an active systemd service.
#
# Reload is always executed because the caller explicitly
# requests it. The service must remain active after reload.
#
# Arguments:
#   $1 - Service name, with or without ".service"
#
# Returns:
#   DAIA_SUCCESS when the service reloads and remains active.
#   DAIA_INVALID_ARGUMENT when the name is invalid.
#   DAIA_NOT_FOUND when the service does not exist.
#   DAIA_VERIFICATION_FAILED when the service is not active.
#   DAIA_ERROR or a systemctl return code on failure.
############################################################

service_reload()
{
    local service_unit
    local status

    if service_unit="$(
        _service_unit_resolve "${1:-}"
    )"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if _service_require_exists "$service_unit"
    then
        :
    else
        status=$?
        return "$status"
    fi

    if service_is_active "$service_unit"
    then
        :
    else
        status=$?

        if (( status == DAIA_VERIFICATION_FAILED ))
        then
            log_error \
                "Cannot reload an inactive systemd service: ${service_unit}"
        fi

        return "$status"
    fi

    log_info \
        "Reloading systemd service: ${service_unit}"

    if _service_systemctl_execute reload "$service_unit"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if ! service_is_active "$service_unit"
    then
        status=$?

        if (( status == DAIA_VERIFICATION_FAILED ))
        then
            log_error \
                "Systemd service is not active after the reload operation: ${service_unit}"
        fi

        return "$status"
    fi

    log_success \
        "Systemd service reloaded: ${service_unit}"

    return "$DAIA_SUCCESS"
}

############################################################
# Validate library dependencies
############################################################

_service_dependency_validate
