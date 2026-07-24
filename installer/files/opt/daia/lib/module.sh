#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : module.sh
# Purpose    : Provide the DAIA module framework mechanics.
#
# Version    : 1.1.0
# Codename   : Pragna
# License    : GPL-3.0
#
# Responsibilities
# ----------------
# - Validate module metadata and lifecycle hooks.
# - Install and verify module package manifests.
# - Enable, disable, and restart module services.
# - Remove module metadata and lifecycle functions after use.
#
# Non-Responsibilities
# --------------------
# - Defining module-specific installation policy.
# - Installing packages directly through apt or dpkg.
# - Calling systemctl directly.
# - Loading module files.
# - Orchestrating the complete module lifecycle.
#
# Dependencies
# ------------
# The following libraries must be loaded before this file:
#
#   common.sh
#   logging.sh
#   packages.sh
#   services.sh
#
# Module Metadata
# ---------------
# Every module must define:
#
#   MODULE_NAME
#   MODULE_DESCRIPTION
#   MODULE_VERSION
#   MODULE_MANIFEST
#   MODULE_SERVICES
#
# MODULE_SERVICES must be an indexed array. It may be empty
# when the module does not manage any systemd services.
#
# Module Lifecycle Hooks
# ----------------------
# Every module must define:
#
#   module_validate
#   module_pre_install
#   module_install
#   module_post_install
#   module_verify
#   module_cleanup
#
# Public API
# ----------
#   framework_validate
#   framework_install
#   framework_verify
#   framework_enable_services
#   framework_disable_services
#   framework_restart_services
#   framework_cleanup
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
# Framework constants
############################################################


readonly -a DAIA_MODULE_REQUIRED_HOOKS=(
    "module_validate"
    "module_pre_install"
    "module_install"
    "module_post_install"
    "module_verify"
    "module_cleanup"
)

readonly -a DAIA_MODULE_MANAGED_METADATA=(
    "MODULE_NAME"
    "MODULE_DESCRIPTION"
    "MODULE_VERSION"
    "MODULE_MANIFEST"
    "MODULE_SERVICES"
)

readonly -a DAIA_MODULE_MANAGED_HOOKS=(
    "module_validate"
    "module_pre_install"
    "module_install"
    "module_post_install"
    "module_verify"
    "module_cleanup"
)

############################################################
# _framework_dependency_validate
#
# Confirm that functions and constants required by the module
# framework are available.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when all dependencies are available.
#   DAIA_ERROR when a dependency is unavailable.
############################################################

_framework_dependency_validate()
{
    local required_constant
    local required_function

    for required_function in \
        log_info \
        log_error \
        log_success \
        package_manifest_read \
        package_manifest_install \
        package_manifest_verify \
        service_exists \
        service_is_active \
        service_is_enabled \
        service_enable \
        service_disable \
        service_restart
    do
        if ! declare -F "$required_function" >/dev/null 2>&1
        then
            printf 'ERROR: Required module-framework function is unavailable: %s\n' \
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
                "Required module-framework constant is unavailable: ${required_constant}"

            return "${DAIA_ERROR:-1}"
        fi
    done

    return "$DAIA_SUCCESS"
}

############################################################
# _framework_variable_is_declared
#
# Determine whether a shell variable is declared.
#
# Arguments:
#   $1 - Variable name
#
# Returns:
#   DAIA_SUCCESS when declared.
#   DAIA_NOT_FOUND otherwise.
############################################################

_framework_variable_is_declared()
{
    local variable_name="${1:-}"

    if [[ -z "$variable_name" ]]
    then
        return "$DAIA_INVALID_ARGUMENT"
    fi

    if declare -p "$variable_name" >/dev/null 2>&1
    then
        return "$DAIA_SUCCESS"
    fi

    return "$DAIA_NOT_FOUND"
}

############################################################
# _framework_scalar_metadata_validate
#
# Validate a required scalar module metadata variable.
#
# Arguments:
#   $1 - Variable name
#
# Returns:
#   DAIA_SUCCESS when valid.
#   DAIA_INVALID_ARGUMENT when absent, empty, or not scalar.
############################################################

_framework_scalar_metadata_validate()
{
    local declaration
    local variable_name="${1:-}"

    if ! _framework_variable_is_declared "$variable_name"
    then
        log_error \
            "Required module metadata is unavailable: ${variable_name}"

        return "$DAIA_INVALID_ARGUMENT"
    fi

    declaration="$(
        declare -p "$variable_name" 2>/dev/null
    )"

    case "$declaration" in
        "declare -a "*|"declare -A "*)
            log_error \
                "Module metadata must be scalar: ${variable_name}"

            return "$DAIA_INVALID_ARGUMENT"
            ;;
    esac

    if [[ -z "${!variable_name}" ]]
    then
        log_error \
            "Required module metadata is empty: ${variable_name}"

        return "$DAIA_INVALID_ARGUMENT"
    fi

    return "$DAIA_SUCCESS"
}

############################################################
# _framework_services_metadata_validate
#
# Validate MODULE_SERVICES.
#
# MODULE_SERVICES must be an indexed array. Empty arrays are
# valid for modules that do not manage services.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when valid.
#   DAIA_INVALID_ARGUMENT otherwise.
############################################################

_framework_services_metadata_validate()
{
    local declaration
    local service_name

    if ! _framework_variable_is_declared MODULE_SERVICES
    then
        log_error \
            "Required module metadata is unavailable: MODULE_SERVICES"

        return "$DAIA_INVALID_ARGUMENT"
    fi

    declaration="$(
        declare -p MODULE_SERVICES 2>/dev/null
    )"

    if [[ "$declaration" != "declare -a "* ]]
    then
        log_error \
            "MODULE_SERVICES must be an indexed array."

        return "$DAIA_INVALID_ARGUMENT"
    fi

    for service_name in "${MODULE_SERVICES[@]}"
    do
        if [[ -z "$service_name" ]]
        then
            log_error \
                "MODULE_SERVICES contains an empty service name."

            return "$DAIA_INVALID_ARGUMENT"
        fi
    done

    return "$DAIA_SUCCESS"
}

############################################################
# _framework_module_name_validate
#
# Validate MODULE_NAME.
#
# Valid module names:
#
# - Begin with a lowercase letter or digit.
# - Contain lowercase letters, digits, underscores, or
#   hyphens.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when valid.
#   DAIA_INVALID_ARGUMENT otherwise.
############################################################

_framework_module_name_validate()
{
    if [[ -z "${MODULE_NAME-}" ]]
    then
        log_error "Required module metadata is missing: MODULE_NAME"
        return "$DAIA_INVALID_ARGUMENT"
    fi

    if [[ "$MODULE_NAME" =~ ^[a-z0-9][a-z0-9_-]*$ ]]
    then
        return "$DAIA_SUCCESS"
    fi

    log_error \
        "Invalid MODULE_NAME: ${MODULE_NAME}"

    return "$DAIA_INVALID_ARGUMENT"
}
############################################################
# _framework_module_version_validate
#
# Validate MODULE_VERSION.
#
# Versions must begin with a digit and may contain letters,
# digits, periods, underscores, plus signs, and hyphens.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when valid.
#   DAIA_INVALID_ARGUMENT otherwise.
############################################################

_framework_module_version_validate()
{
    if [[ "$MODULE_VERSION" =~ ^[0-9][A-Za-z0-9._+-]*$ ]]
    then
        return "$DAIA_SUCCESS"
    fi

    log_error \
        "Invalid MODULE_VERSION for ${MODULE_NAME}: ${MODULE_VERSION}"

    return "$DAIA_INVALID_ARGUMENT"
}

############################################################
# _framework_manifest_validate
#
# Validate the module package manifest by reading it through
# the package-management library.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when valid.
#   A package-library return code otherwise.
############################################################

_framework_manifest_validate()
{
# Populated indirectly by package_manifest_read through a nameref.
# shellcheck disable=SC2034

    local -a manifest_packages=()
    local status

    if package_manifest_read \
        "$MODULE_MANIFEST" \
        manifest_packages
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
    fi

    if (( status != DAIA_SUCCESS ))
    then
        log_error \
            "Invalid package manifest for module ${MODULE_NAME}: ${MODULE_MANIFEST}"

        return "$status"
    fi

    return "$DAIA_SUCCESS"
}

############################################################
# _framework_hooks_validate
#
# Confirm that every required module lifecycle hook exists.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when all hooks exist.
#   DAIA_INVALID_ARGUMENT otherwise.
############################################################

_framework_hooks_validate()
{
    local hook_name

    for hook_name in "${DAIA_MODULE_REQUIRED_HOOKS[@]}"
    do
        if ! declare -F "$hook_name" >/dev/null 2>&1
        then
            log_error \
                "Required lifecycle hook is unavailable for module ${MODULE_NAME}: ${hook_name}"

            return "$DAIA_INVALID_ARGUMENT"
        fi
    done

    return "$DAIA_SUCCESS"
}

############################################################
# _framework_services_validate
#
# Verify that every service declared by the current module
# exists.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when all declared services exist.
#   A service-library return code otherwise.
############################################################

_framework_services_validate()
{
    local service_name
    local status

    for service_name in "${MODULE_SERVICES[@]}"
    do
        if service_exists "$service_name"
        then
            status="$DAIA_SUCCESS"
        else
            status=$?

            log_error \
                "Required service is unavailable for module ${MODULE_NAME}: ${service_name}"

            return "$status"
        fi
    done

    return "$DAIA_SUCCESS"
}

############################################################
# _framework_services_enabled_verify
#
# Verify that every service declared by the current module is
# enabled.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when all services are enabled.
#   A service-library return code otherwise.
############################################################

_framework_services_enabled_verify()
{
    local service_name
    local status

    for service_name in "${MODULE_SERVICES[@]}"
    do
        if service_is_enabled "$service_name"
        then
            status="$DAIA_SUCCESS"
        else
            status=$?

            if (( status == DAIA_VERIFICATION_FAILED ))
            then
                log_error \
                    "Service is not enabled for module ${MODULE_NAME}: ${service_name}"
            fi

            return "$status"
        fi
    done

    return "$DAIA_SUCCESS"
}

############################################################
# _framework_services_active_verify
#
# Verify that every service declared by the current module is
# active.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when all services are active.
#   A service-library return code otherwise.
############################################################

_framework_services_active_verify()
{
    local service_name
    local status

    for service_name in "${MODULE_SERVICES[@]}"
    do
        if service_is_active "$service_name"
        then
            status="$DAIA_SUCCESS"
        else
            status=$?

            if (( status == DAIA_VERIFICATION_FAILED ))
            then
                log_error \
                    "Service is not active for module ${MODULE_NAME}: ${service_name}"
            fi

            return "$status"
        fi
    done

    return "$DAIA_SUCCESS"
}

############################################################
# framework_validate
#
# Validate the current module's metadata, lifecycle hooks,
# package manifest, and declared services.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when the module is valid.
#   A standard DAIA return code otherwise.
############################################################

framework_validate()
{
    local metadata_variable
    local status

    for metadata_variable in \
        MODULE_NAME \
        MODULE_DESCRIPTION \
        MODULE_VERSION \
        MODULE_MANIFEST
    do
        if _framework_scalar_metadata_validate "$metadata_variable"
        then
            status="$DAIA_SUCCESS"
        else
            status=$?
            return "$status"
        fi
    done

    if _framework_services_metadata_validate
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if _framework_module_name_validate
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if _framework_module_version_validate
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if _framework_hooks_validate
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if _framework_manifest_validate
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if _framework_services_validate
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    log_success \
        "Module framework validation completed: ${MODULE_NAME} ${MODULE_VERSION}"

    return "$DAIA_SUCCESS"
}

############################################################
# framework_install
#
# Install all packages declared by MODULE_MANIFEST.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when installation succeeds.
#   A package-library return code otherwise.
############################################################

framework_install()
{
    local status

    log_info \
        "Installing package manifest for module ${MODULE_NAME}: ${MODULE_MANIFEST}"

    if package_manifest_install "$MODULE_MANIFEST"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
    fi

    if (( status != DAIA_SUCCESS ))
    then
        log_error \
            "Package installation failed for module ${MODULE_NAME}."

        return "$status"
    fi

    log_success \
        "Package installation completed for module: ${MODULE_NAME}"

    return "$DAIA_SUCCESS"
}

############################################################
# framework_verify
#
# Verify all packages declared by MODULE_MANIFEST and verify
# that declared module services are enabled and active.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when verification succeeds.
#   A package-library or service-library return code
#   otherwise.
############################################################

framework_verify()
{
    local status

    log_info \
        "Verifying framework-managed resources for module: ${MODULE_NAME}"

    if package_manifest_verify "$MODULE_MANIFEST"
    then
        status="$DAIA_SUCCESS"
    else
        status=$?

        log_error \
            "Package verification failed for module ${MODULE_NAME}."

        return "$status"
    fi

    if _framework_services_enabled_verify
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    if _framework_services_active_verify
    then
        status="$DAIA_SUCCESS"
    else
        status=$?
        return "$status"
    fi

    log_success \
        "Framework verification completed for module: ${MODULE_NAME}"

    return "$DAIA_SUCCESS"
}

############################################################
# framework_enable_services
#
# Enable all services declared in MODULE_SERVICES.
#
# The service library provides idempotent behavior.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when all services are enabled.
#   A service-library return code otherwise.
############################################################

framework_enable_services()
{
    local service_name
    local status

    if (( ${#MODULE_SERVICES[@]} == 0 ))
    then
        log_info \
            "Module does not declare services to enable: ${MODULE_NAME}"

        return "$DAIA_SUCCESS"
    fi

    for service_name in "${MODULE_SERVICES[@]}"
    do
        if service_enable "$service_name"
        then
            status="$DAIA_SUCCESS"
        else
            status=$?

            log_error \
                "Failed to enable service for module ${MODULE_NAME}: ${service_name}"

            return "$status"
        fi
    done

    log_success \
        "All declared services enabled for module: ${MODULE_NAME}"

    return "$DAIA_SUCCESS"
}

############################################################
# framework_disable_services
#
# Disable all services declared in MODULE_SERVICES.
#
# The service library provides idempotent behavior.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when all services are disabled.
#   A service-library return code otherwise.
############################################################

framework_disable_services()
{
    local service_name
    local status

    if (( ${#MODULE_SERVICES[@]} == 0 ))
    then
        log_info \
            "Module does not declare services to disable: ${MODULE_NAME}"

        return "$DAIA_SUCCESS"
    fi

    for service_name in "${MODULE_SERVICES[@]}"
    do
        if service_disable "$service_name"
        then
            status="$DAIA_SUCCESS"
        else
            status=$?

            log_error \
                "Failed to disable service for module ${MODULE_NAME}: ${service_name}"

            return "$status"
        fi
    done

    log_success \
        "All declared services disabled for module: ${MODULE_NAME}"

    return "$DAIA_SUCCESS"
}

############################################################
# framework_restart_services
#
# Restart all services declared in MODULE_SERVICES.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when all services restart successfully.
#   A service-library return code otherwise.
############################################################

framework_restart_services()
{
    local service_name
    local status

    if (( ${#MODULE_SERVICES[@]} == 0 ))
    then
        log_info \
            "Module does not declare services to restart: ${MODULE_NAME}"

        return "$DAIA_SUCCESS"
    fi

    for service_name in "${MODULE_SERVICES[@]}"
    do
        if service_restart "$service_name"
        then
            status="$DAIA_SUCCESS"
        else
            status=$?

            log_error \
                "Failed to restart service for module ${MODULE_NAME}: ${service_name}"

            return "$status"
        fi
    done

    log_success \
        "All declared services restarted for module: ${MODULE_NAME}"

    return "$DAIA_SUCCESS"
}

############################################################
# framework_cleanup
#
# Remove module-owned metadata and lifecycle functions from
# the current shell after module processing.
#
# Framework functions and shared-library state are preserved.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS
############################################################

framework_cleanup()
{
    local hook_name
    local metadata_variable
    local module_name="${MODULE_NAME:-unknown}"

    for hook_name in "${DAIA_MODULE_MANAGED_HOOKS[@]}"
    do
        unset -f "$hook_name" 2>/dev/null || true
    done

    for metadata_variable in "${DAIA_MODULE_MANAGED_METADATA[@]}"
    do
        unset "$metadata_variable" 2>/dev/null || true
    done

    log_info \
        "Framework state cleaned for module: ${module_name}"

    return "$DAIA_SUCCESS"
}

############################################################
# Validate library dependencies
############################################################

_framework_dependency_validate
