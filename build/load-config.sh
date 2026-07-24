#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : build/load-config.sh
# Purpose    : Load and validate the DAIA build configuration.
#
# Version    : 1.0.0
# Codename   : Pragna
# License    : GPL-3.0
#
# Responsibilities
# ----------------
# - Locate the selected DAIA build configuration.
# - Load the configuration into the current shell.
# - Validate required variables and supported values.
# - Convert relative payload paths into absolute paths.
# - Export validated settings to calling build scripts.
#
# Dependencies
# ------------
# - Bash 4 or later.
# - build/variables.sh
# - payload/config/pragna.conf, unless another configuration
#   path is supplied through DAIA_CONFIG_FILE.
#
# Outputs
# -------
# Validated DAIA configuration variables in the current shell.
#
# Failure Modes
# -------------
# Returns a non-zero status when:
# - the configuration file is missing;
# - a required variable is empty;
# - a Boolean value is invalid;
# - an unsupported architecture or desktop is selected;
# - a required payload path is unsafe.
#
# Future Extension
# ----------------
# Add new validation functions for additional editions,
# architectures, desktops, components, and feature flags.
#
# Usage
# -----
# This file is intended to be sourced:
#
#   source build/load-config.sh
#
# An alternative configuration can be selected with:
#
#   DAIA_CONFIG_FILE=/path/to/custom.conf
#   source build/load-config.sh
# ==========================================================

set -euo pipefail

############################################################
#
# Sections
#
#   1. Loader Environment
#   2. Logging Helpers
#   3. Validation Helpers
#   4. Path Helpers
#   5. Configuration Loading
#   6. Configuration Validation
#   7. Exported Values
#
############################################################

############################################################
# 1. Loader Environment
############################################################

DAIA_CONFIG_LOADER_DIR="$(
    cd "$(dirname "${BASH_SOURCE[0]}")" &&
    pwd
)"

# shellcheck disable=SC1091
source "$DAIA_CONFIG_LOADER_DIR/variables.sh"

DAIA_DEFAULT_CONFIG_FILE="$PROJECT_ROOT/payload/config/pragna.conf"
DAIA_CONFIG_FILE="${DAIA_CONFIG_FILE:-$DAIA_DEFAULT_CONFIG_FILE}"

############################################################
# 2. Logging Helpers
############################################################

############################################################
# config_log_info
#
# Print an informational message.
#
# Arguments:
#   $1 - Message text
#
# Returns:
#   0
############################################################
config_log_info()
{
    printf '[INFO] %s\n' "$1"
}

############################################################
# config_log_success
#
# Print a success message.
#
# Arguments:
#   $1 - Message text
#
# Returns:
#   0
############################################################
config_log_success()
{
    printf '[ OK ] %s\n' "$1"
}

############################################################
# config_log_error
#
# Print an error message to standard error.
#
# Arguments:
#   $1 - Message text
#
# Returns:
#   0
############################################################
config_log_error()
{
    printf '[FAIL] %s\n' "$1" >&2
}

############################################################
# config_die
#
# Print an error message and terminate configuration loading.
#
# Arguments:
#   $1 - Error message
#   $2 - Optional exit code, default 1
#
# Returns:
#   Does not return.
############################################################
config_die()
{
    local message="$1"
    local exit_code="${2:-1}"

    config_log_error "$message"
    return "$exit_code"
}

############################################################
# 3. Validation Helpers
############################################################

############################################################
# require_config_variable
#
# Verify that a required configuration variable is defined
# and contains a non-empty value.
#
# Arguments:
#   $1 - Variable name
#
# Returns:
#   0 when valid.
#   1 otherwise.
############################################################
require_config_variable()
{
    local variable_name="$1"
    local variable_value="${!variable_name-}"

    if [[ -z "$variable_value" ]]
    then
        config_log_error "Required configuration variable is missing: $variable_name"
        return 1
    fi

    return 0
}

############################################################
# validate_boolean_value
#
# Verify that a value uses a supported Boolean representation.
#
# Accepted values:
#   true
#   false
#
# Arguments:
#   $1 - Variable name
#
# Returns:
#   0 when valid.
#   1 otherwise.
############################################################
validate_boolean_value()
{
    local variable_name="$1"
    local variable_value="${!variable_name-}"

    case "$variable_value" in
        true|false)
            return 0
            ;;
        *)
            config_log_error \
                "Invalid Boolean value for $variable_name: $variable_value"
            return 1
            ;;
    esac
}

############################################################
# validate_supported_value
#
# Verify that a variable contains one of the supported values.
#
# Arguments:
#   $1 - Variable name
#   $2... - Supported values
#
# Returns:
#   0 when supported.
#   1 otherwise.
############################################################
validate_supported_value()
{
    local variable_name="$1"
    shift

    local variable_value="${!variable_name-}"
    local supported_value

    for supported_value in "$@"
    do
        if [[ "$variable_value" == "$supported_value" ]]
        then
            return 0
        fi
    done

    config_log_error \
        "Unsupported value for $variable_name: $variable_value"

    return 1
}

############################################################
# validate_relative_project_path
#
# Verify that a path is relative to the project root and does
# not contain parent-directory traversal.
#
# Arguments:
#   $1 - Variable name
#
# Returns:
#   0 when safe.
#   1 otherwise.
############################################################
validate_relative_project_path()
{
    local variable_name="$1"
    local variable_value="${!variable_name-}"

    if [[ "$variable_value" == /* ]]
    then
        config_log_error \
            "$variable_name must be project-relative: $variable_value"
        return 1
    fi

    if [[ "$variable_value" == ".." ||
          "$variable_value" == ../* ||
          "$variable_value" == */../* ||
          "$variable_value" == */.. ]]
    then
        config_log_error \
            "$variable_name contains unsafe parent traversal: $variable_value"
        return 1
    fi

    return 0
}

############################################################
# 4. Path Helpers
############################################################

############################################################
# make_project_absolute
#
# Convert a project-relative path into an absolute path.
#
# Arguments:
#   $1 - Relative project path
#
# Returns:
#   Absolute path on standard output.
############################################################
make_project_absolute()
{
    local relative_path="$1"

    printf '%s/%s\n' "$PROJECT_ROOT" "$relative_path"
}

############################################################
# 5. Configuration Loading
############################################################

if [[ ! -f "$DAIA_CONFIG_FILE" ]]
then
    config_die "DAIA configuration file not found: $DAIA_CONFIG_FILE"
    return $?
fi

config_log_info "Loading DAIA configuration:"
printf '       %s\n' "$DAIA_CONFIG_FILE"

# shellcheck disable=SC1090
source "$DAIA_CONFIG_FILE"

############################################################
# 6. Configuration Validation
############################################################

configuration_errors=0

required_variables=(
    DAIA_NAME
    DAIA_FULL_NAME
    DAIA_VERSION
    DAIA_CODENAME
    DAIA_EDITION
    DAIA_ARCHITECTURE
    DAIA_BASE_DISTRIBUTION
    DAIA_BASE_VERSION
    DAIA_DESKTOP_ENABLED
    DAIA_DESKTOP_ENVIRONMENT
    DAIA_ENABLE_DOCKER
    DAIA_ENABLE_OLLAMA
    DAIA_ENABLE_OPEN_WEBUI
    DAIA_ENABLE_DEFAULT_MODEL
    DAIA_DOCKER_SOURCE
    DAIA_OLLAMA_SOURCE
    DAIA_DEPENDENCIES_SOURCE
    DAIA_OPEN_WEBUI_IMAGE
    DAIA_DEFAULT_MODEL_SOURCE
    DAIA_OFFLINE_INSTALL
    DAIA_FIRST_BOOT_SETUP
    DAIA_TELEMETRY_ENABLED
    DAIA_REMOTE_ACCESS_ENABLED
    DAIA_BRANDING_ENABLED
    DAIA_DEFAULT_WALLPAPER
    DAIA_ASSISTANT_ICON
)

boolean_variables=(
    DAIA_DESKTOP_ENABLED
    DAIA_ENABLE_DOCKER
    DAIA_ENABLE_OLLAMA
    DAIA_ENABLE_OPEN_WEBUI
    DAIA_ENABLE_DEFAULT_MODEL
    DAIA_OFFLINE_INSTALL
    DAIA_FIRST_BOOT_SETUP
    DAIA_TELEMETRY_ENABLED
    DAIA_REMOTE_ACCESS_ENABLED
    DAIA_BRANDING_ENABLED
)

path_variables=(
    DAIA_DOCKER_SOURCE
    DAIA_OLLAMA_SOURCE
    DAIA_DEPENDENCIES_SOURCE
    DAIA_OPEN_WEBUI_IMAGE
    DAIA_DEFAULT_MODEL_SOURCE
    DAIA_DEFAULT_WALLPAPER
    DAIA_ASSISTANT_ICON
)

for variable_name in "${required_variables[@]}"
do
    if ! require_config_variable "$variable_name"
    then
        configuration_errors=$((configuration_errors + 1))
    fi
done

for variable_name in "${boolean_variables[@]}"
do
    if ! validate_boolean_value "$variable_name"
    then
        configuration_errors=$((configuration_errors + 1))
    fi
done

for variable_name in "${path_variables[@]}"
do
    if ! validate_relative_project_path "$variable_name"
    then
        configuration_errors=$((configuration_errors + 1))
    fi
done

if ! validate_supported_value \
    DAIA_ARCHITECTURE \
    amd64
then
    configuration_errors=$((configuration_errors + 1))
fi

if ! validate_supported_value \
    DAIA_BASE_DISTRIBUTION \
    debian
then
    configuration_errors=$((configuration_errors + 1))
fi

if ! validate_supported_value \
    DAIA_DESKTOP_ENVIRONMENT \
    xfce
then
    configuration_errors=$((configuration_errors + 1))
fi

if (( configuration_errors > 0 ))
then
    config_die \
        "DAIA configuration validation failed with $configuration_errors error(s)."
    return $?
fi

############################################################
# 7. Exported Values
############################################################

DAIA_DOCKER_SOURCE_ABS="$(
    make_project_absolute "$DAIA_DOCKER_SOURCE"
)"

DAIA_OLLAMA_SOURCE_ABS="$(
    make_project_absolute "$DAIA_OLLAMA_SOURCE"
)"

DAIA_DEPENDENCIES_SOURCE_ABS="$(
    make_project_absolute "$DAIA_DEPENDENCIES_SOURCE"
)"

DAIA_OPEN_WEBUI_IMAGE_ABS="$(
    make_project_absolute "$DAIA_OPEN_WEBUI_IMAGE"
)"

DAIA_DEFAULT_MODEL_SOURCE_ABS="$(
    make_project_absolute "$DAIA_DEFAULT_MODEL_SOURCE"
)"

DAIA_DEFAULT_WALLPAPER_ABS="$(
    make_project_absolute "$DAIA_DEFAULT_WALLPAPER"
)"

DAIA_ASSISTANT_ICON_ABS="$(
    make_project_absolute "$DAIA_ASSISTANT_ICON"
)"

export DAIA_CONFIG_FILE

export DAIA_NAME
export DAIA_FULL_NAME
export DAIA_VERSION
export DAIA_CODENAME
export DAIA_EDITION

export DAIA_ARCHITECTURE
export DAIA_BASE_DISTRIBUTION
export DAIA_BASE_VERSION

export DAIA_DESKTOP_ENABLED
export DAIA_DESKTOP_ENVIRONMENT

export DAIA_ENABLE_DOCKER
export DAIA_ENABLE_OLLAMA
export DAIA_ENABLE_OPEN_WEBUI
export DAIA_ENABLE_DEFAULT_MODEL

export DAIA_OFFLINE_INSTALL
export DAIA_FIRST_BOOT_SETUP
export DAIA_TELEMETRY_ENABLED
export DAIA_REMOTE_ACCESS_ENABLED
export DAIA_BRANDING_ENABLED

export DAIA_DOCKER_SOURCE_ABS
export DAIA_OLLAMA_SOURCE_ABS
export DAIA_DEPENDENCIES_SOURCE_ABS
export DAIA_OPEN_WEBUI_IMAGE_ABS
export DAIA_DEFAULT_MODEL_SOURCE_ABS
export DAIA_DEFAULT_WALLPAPER_ABS
export DAIA_ASSISTANT_ICON_ABS

config_log_success \
    "DAIA configuration loaded: $DAIA_NAME $DAIA_VERSION $DAIA_CODENAME"

############################################################
# End of File
############################################################
