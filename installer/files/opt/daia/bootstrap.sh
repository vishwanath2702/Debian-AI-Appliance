#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : bootstrap.sh
# Purpose    : Orchestrate the DAIA module installation
#              lifecycle.
#
# Version    : 1.0.1
# Codename   : Pragna
# License    : GPL-3.0
#
# Responsibilities
# ----------------
# - Load the DAIA configuration.
# - Load shared installer libraries.
# - Read the enabled-module configuration.
# - Load each enabled module.
# - Execute the standard module lifecycle.
# - Record successful module completion.
# - Clean module state between module executions.
#
# Non-Responsibilities
# --------------------
# - Installing packages directly.
# - Managing services directly.
# - Implementing component-specific configuration.
# - Containing knowledge about individual modules.
# - Disabling the first-boot service.
#
# Module Lifecycle
# ----------------
# Each enabled module is processed in this order:
#
#   framework_validate
#   module_validate
#   module_pre_install
#   module_install
#   module_post_install
#   module_verify
#   module_cleanup
#   record completion
#   framework_cleanup
#
# module_cleanup and framework_cleanup are handled separately
# from the primary lifecycle so cleanup is attempted after
# both successful and failed module execution.
#
# Configuration
# -------------
# Enabled modules are read from:
#
#   ${DAIA_CONFIG_DIR}/modules.conf
#
# The module configuration accepts:
#
# - One module name per line.
# - Blank lines.
# - Full-line comments beginning with "#".
# - Inline comments beginning with "#".
#
# State
# -----
# Successful module completion is recorded in:
#
#   ${DAIA_FIRSTBOOT_STATE}
#
# The state file currently provides an installation audit
# record. Completed modules are not automatically skipped.
#
# This file must be executed and must not be sourced.
# ==========================================================

set -euo pipefail

############################################################
# Bootstrap location
############################################################
BOOTSTRAP_CONFIG_DIR="$(
    cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1
    pwd -P
)"
readonly BOOTSTRAP_CONFIG_DIR

readonly BOOTSTRAP_CONFIG_FILE="${BOOTSTRAP_CONFIG_DIR}/config/daia.conf"
############################################################
# Load DAIA configuration
############################################################

if [[ ! -f "$BOOTSTRAP_CONFIG_FILE" ]]
then
    printf 'ERROR: DAIA configuration file does not exist: %s\n' \
        "$BOOTSTRAP_CONFIG_FILE" >&2

    exit 1
fi

if [[ ! -r "$BOOTSTRAP_CONFIG_FILE" ]]
then
    printf 'ERROR: DAIA configuration file is not readable: %s\n' \
        "$BOOTSTRAP_CONFIG_FILE" >&2

    exit 1
fi

# shellcheck source=/dev/null
source "$BOOTSTRAP_CONFIG_FILE"

############################################################
# Load shared libraries
############################################################

############################################################
# Load shared libraries
############################################################

readonly BOOTSTRAP_COMMON_LIBRARY="${DAIA_LIB_DIR}/common.sh"
readonly BOOTSTRAP_LOGGING_LIBRARY="${DAIA_LIB_DIR}/logging.sh"
readonly BOOTSTRAP_PACKAGES_LIBRARY="${DAIA_LIB_DIR}/packages.sh"
readonly BOOTSTRAP_SERVICES_LIBRARY="${DAIA_LIB_DIR}/services.sh"
readonly BOOTSTRAP_MODULE_LIBRARY="${DAIA_LIB_DIR}/module.sh"

for bootstrap_library in \
    "$BOOTSTRAP_COMMON_LIBRARY" \
    "$BOOTSTRAP_LOGGING_LIBRARY" \
    "$BOOTSTRAP_PACKAGES_LIBRARY" \
    "$BOOTSTRAP_SERVICES_LIBRARY" \
    "$BOOTSTRAP_MODULE_LIBRARY"
do
    if [[ ! -f "$bootstrap_library" ]]
    then
        printf 'ERROR: Required DAIA library does not exist: %s\n' \
            "$bootstrap_library" >&2

        exit 1
    fi

    if [[ ! -r "$bootstrap_library" ]]
    then
        printf 'ERROR: Required DAIA library is not readable: %s\n' \
            "$bootstrap_library" >&2

        exit 1
    fi

    # shellcheck source=/dev/null
    source "$bootstrap_library"
done

unset bootstrap_library
############################################################
# Bootstrap constants
############################################################

readonly BOOTSTRAP_MODULES_FILE="${DAIA_CONFIG_DIR}/modules.conf"

readonly -a BOOTSTRAP_PRIMARY_LIFECYCLE=(
    "framework_validate"
    "module_validate"
    "module_pre_install"
    "module_install"
    "module_post_install"
    "module_verify"
)

############################################################
# _bootstrap_runtime_validate
#
# Confirm that configuration values, shared functions,
# return-code constants, files, directories, and commands
# required by bootstrap are available.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when the runtime is valid.
#   DAIA_ERROR when validation fails.
#   DAIA_NOT_FOUND when a required command is unavailable.
############################################################

_bootstrap_runtime_validate()
{
    local required_command
    local required_function
    local required_path_variable
    local required_variable

    for required_function in \
        require_root \
        require_command \
        utc_timestamp \
        log_info \
        log_error \
        log_success \
        framework_cleanup
    do
        if ! declare -F "$required_function" >/dev/null 2>&1
        then
            printf 'ERROR: Required bootstrap function is unavailable: %s\n' \
                "$required_function" >&2

            return "${DAIA_ERROR:-1}"
        fi
    done

    for required_variable in \
        DAIA_SUCCESS \
        DAIA_ERROR \
        DAIA_INVALID_ARGUMENT \
        DAIA_NOT_FOUND
    do
        if [[ -z "${!required_variable+x}" ]]
        then
            log_error \
                "Required bootstrap constant is unavailable: ${required_variable}"

            return "${DAIA_ERROR:-1}"
        fi
    done

    for required_path_variable in \
        DAIA_CONFIG_DIR \
        DAIA_MODULE_DIR \
        DAIA_STATE_DIR \
        DAIA_FIRSTBOOT_STATE
    do
        if [[ -z "${!required_path_variable+x}" ]]
        then
            log_error \
                "Required DAIA path variable is unavailable: ${required_path_variable}"

            return "$DAIA_ERROR"
        fi

        if [[ -z "${!required_path_variable}" ]]
        then
            log_error \
                "Required DAIA path variable is empty: ${required_path_variable}"

            return "$DAIA_ERROR"
        fi
    done

    for required_command in \
        awk \
        chmod \
        mkdir \
        mktemp \
        mv \
        rm
    do
        if ! require_command "$required_command"
        then
            return "$DAIA_NOT_FOUND"
        fi
    done

    return "$DAIA_SUCCESS"
}

############################################################
# _bootstrap_module_name_is_valid
#
# Validate a module name read from modules.conf.
#
# Valid module names:
#
# - Begin with a lowercase letter or digit.
# - Contain lowercase letters, digits, hyphens, or underscores.
#
# Arguments:
#   $1 - Module name
#
# Returns:
#   DAIA_SUCCESS when valid.
#   DAIA_INVALID_ARGUMENT otherwise.
############################################################

_bootstrap_module_name_is_valid()
{
    local module_name="${1:-}"

    if [[ "$module_name" =~ ^[a-z0-9][a-z0-9_-]*$ ]]
    then
        return "$DAIA_SUCCESS"
    fi

    return "$DAIA_INVALID_ARGUMENT"
}

############################################################
# _bootstrap_normalize_config_line
#
# Normalize one line from modules.conf by removing:
#
# - A trailing carriage return.
# - Inline comments.
# - Leading whitespace.
# - Trailing whitespace.
#
# Arguments:
#   $1 - Raw configuration line
#
# Output:
#   Normalized line on standard output.
#
# Returns:
#   DAIA_SUCCESS
############################################################

_bootstrap_normalize_config_line()
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
# _bootstrap_modules_read
#
# Read and validate the enabled-module configuration.
#
# The destination array is cleared before modules are added.
# Duplicate module entries are rejected.
#
# Arguments:
#   $1 - Destination indexed-array variable name
#
# Returns:
#   DAIA_SUCCESS on success.
#   DAIA_INVALID_ARGUMENT for invalid configuration.
#   DAIA_NOT_FOUND when modules.conf does not exist.
#   DAIA_ERROR when modules.conf cannot be read.
############################################################

_bootstrap_modules_read()
{
    local destination_name="${1:-}"
    local line_number=0
    local module_count=0
    local module_name
    local raw_line

    declare -A discovered_modules=()

    if [[ -z "$destination_name" ]]
    then
        log_error \
            "_bootstrap_modules_read requires a destination array name."

        return "$DAIA_INVALID_ARGUMENT"
    fi

    if [[ ! "$destination_name" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]]
    then
        log_error \
            "Invalid destination array name: ${destination_name}"

        return "$DAIA_INVALID_ARGUMENT"
    fi

    if [[ ! -f "$BOOTSTRAP_MODULES_FILE" ]]
    then
        log_error \
            "Enabled-module configuration does not exist: ${BOOTSTRAP_MODULES_FILE}"

        return "$DAIA_NOT_FOUND"
    fi

    if [[ ! -r "$BOOTSTRAP_MODULES_FILE" ]]
    then
        log_error \
            "Enabled-module configuration is not readable: ${BOOTSTRAP_MODULES_FILE}"

        return "$DAIA_ERROR"
    fi

    local -n destination_array="$destination_name"

    destination_array=()

    while IFS= read -r raw_line || [[ -n "$raw_line" ]]
    do
        line_number=$((line_number + 1))

        module_name="$(
            _bootstrap_normalize_config_line "$raw_line"
        )"

        if [[ -z "$module_name" ]]
        then
            continue
        fi

        if ! _bootstrap_module_name_is_valid "$module_name"
        then
            log_error \
                "Invalid module name at ${BOOTSTRAP_MODULES_FILE}:${line_number}: ${module_name}"

            return "$DAIA_INVALID_ARGUMENT"
        fi

        if [[ -n "${discovered_modules[$module_name]+exists}" ]]
        then
            log_error \
                "Duplicate module at ${BOOTSTRAP_MODULES_FILE}:${line_number}: ${module_name}"

            return "$DAIA_INVALID_ARGUMENT"
        fi

        discovered_modules["$module_name"]=1
        destination_array+=("$module_name")
        module_count=$((module_count + 1))
    done < "$BOOTSTRAP_MODULES_FILE"

    if [[ "$module_count" -eq 0 ]]
    then
        log_error \
            "No enabled modules were found in: ${BOOTSTRAP_MODULES_FILE}"

        return "$DAIA_INVALID_ARGUMENT"
    fi

    log_info \
        "Loaded ${module_count} enabled DAIA module(s)."

    return "$DAIA_SUCCESS"
}

############################################################
# _bootstrap_module_path
#
# Construct the source-file path for a module.
#
# Arguments:
#   $1 - Module name
#
# Output:
#   Module path on standard output.
#
# Returns:
#   DAIA_SUCCESS
############################################################

_bootstrap_module_path()
{
    local module_name="${1:-}"

    printf '%s/%s.sh\n' \
        "$DAIA_MODULE_DIR" \
        "$module_name"

    return "$DAIA_SUCCESS"
}

############################################################
# _bootstrap_module_load
#
# Load a module into the current shell.
#
# Arguments:
#   $1 - Module name
#
# Returns:
#   DAIA_SUCCESS on success.
#   DAIA_INVALID_ARGUMENT when the module name is invalid.
#   DAIA_NOT_FOUND when the module file does not exist.
#   DAIA_ERROR when the module file is unreadable or fails to
#   load.
############################################################

_bootstrap_module_load()
{
    local module_name="${1:-}"
    local module_path

    if ! _bootstrap_module_name_is_valid "$module_name"
    then
        log_error \
            "Cannot load invalid module name: ${module_name}"

        return "$DAIA_INVALID_ARGUMENT"
    fi

    module_path="$(
        _bootstrap_module_path "$module_name"
    )"

    if [[ ! -f "$module_path" ]]
    then
        log_error \
            "Enabled module does not exist: ${module_path}"

        return "$DAIA_NOT_FOUND"
    fi

    if [[ ! -r "$module_path" ]]
    then
        log_error \
            "Enabled module is not readable: ${module_path}"

        return "$DAIA_ERROR"
    fi

    log_info \
        "Loading DAIA module: ${module_name}"

    # shellcheck source=/dev/null
    if source "$module_path"
    then
        return "$DAIA_SUCCESS"
    fi

    log_error \
        "Failed to load DAIA module: ${module_name}"

    return "$DAIA_ERROR"
}

############################################################
# _bootstrap_hook_execute
#
# Execute one lifecycle hook for the current module.
#
# Arguments:
#   $1 - Hook function name
#
# Returns:
#   The hook's return code.
#   DAIA_INVALID_ARGUMENT when the hook is unavailable.
############################################################

_bootstrap_hook_execute()
{
    local hook_name="${1:-}"
    local hook_status

    if [[ -z "$hook_name" ]]
    then
        log_error \
            "_bootstrap_hook_execute requires a hook name."

        return "$DAIA_INVALID_ARGUMENT"
    fi

    if ! declare -F "$hook_name" >/dev/null 2>&1
    then
        log_error \
            "Lifecycle hook is unavailable for module ${MODULE_NAME:-unknown}: ${hook_name}"

        return "$DAIA_INVALID_ARGUMENT"
    fi

    log_info \
        "Executing ${hook_name} for module: ${MODULE_NAME:-unknown}"

    if "$hook_name"
    then
        hook_status="$DAIA_SUCCESS"
    else
        hook_status=$?
    fi

    if (( hook_status != DAIA_SUCCESS ))
    then
        log_error \
            "Lifecycle hook failed for module ${MODULE_NAME:-unknown}: ${hook_name} (status ${hook_status})"

        return "$hook_status"
    fi

    return "$DAIA_SUCCESS"
}

############################################################
# _bootstrap_state_prepare
#
# Create the DAIA state directory and first-boot state file.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS on success.
#   DAIA_ERROR on failure.
############################################################

_bootstrap_state_prepare()
{
    if ! mkdir -p -- "$DAIA_STATE_DIR"
    then
        log_error \
            "Failed to create DAIA state directory: ${DAIA_STATE_DIR}"

        return "$DAIA_ERROR"
    fi

    if [[ ! -e "$DAIA_FIRSTBOOT_STATE" ]]
    then
        if ! : > "$DAIA_FIRSTBOOT_STATE"
        then
            log_error \
                "Failed to create DAIA first-boot state file: ${DAIA_FIRSTBOOT_STATE}"

            return "$DAIA_ERROR"
        fi
    fi

    if [[ ! -f "$DAIA_FIRSTBOOT_STATE" ]]
    then
        log_error \
            "DAIA first-boot state path is not a regular file: ${DAIA_FIRSTBOOT_STATE}"

        return "$DAIA_ERROR"
    fi

    if [[ ! -r "$DAIA_FIRSTBOOT_STATE" ]]
    then
        log_error \
            "DAIA first-boot state file is not readable: ${DAIA_FIRSTBOOT_STATE}"

        return "$DAIA_ERROR"
    fi

    if [[ ! -w "$DAIA_FIRSTBOOT_STATE" ]]
    then
        log_error \
            "DAIA first-boot state file is not writable: ${DAIA_FIRSTBOOT_STATE}"

        return "$DAIA_ERROR"
    fi

    return "$DAIA_SUCCESS"
}

############################################################
# _bootstrap_completion_record
#
# Record successful completion of the current module.
#
# Existing records for the same module are replaced
# atomically so the state file contains one current record per
# module.
#
# State format:
#
#   module_name|module_version|UTC_timestamp
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS on success.
#   DAIA_ERROR on failure.
############################################################

_bootstrap_completion_record()
{
    local completion_timestamp
    local temporary_state_file=""

    if [[ -z "${MODULE_NAME:-}" ]]
    then
        log_error \
            "Cannot record completion because MODULE_NAME is unavailable."

        return "$DAIA_ERROR"
    fi

    if [[ -z "${MODULE_VERSION:-}" ]]
    then
        log_error \
            "Cannot record completion because MODULE_VERSION is unavailable."

        return "$DAIA_ERROR"
    fi

    completion_timestamp="$(
        utc_timestamp
    )"

    if ! temporary_state_file="$(
        mktemp "${DAIA_STATE_DIR}/firstboot.state.XXXXXX"
    )"
    then
        log_error \
            "Failed to create temporary DAIA state file."

        return "$DAIA_ERROR"
    fi

    if ! awk -F '|' \
        -v module_name="$MODULE_NAME" \
        '$1 != module_name' \
        "$DAIA_FIRSTBOOT_STATE" > "$temporary_state_file"
    then
        rm -f -- "$temporary_state_file"

        log_error \
            "Failed to prepare updated DAIA state."

        return "$DAIA_ERROR"
    fi

    if ! printf '%s|%s|%s\n' \
        "$MODULE_NAME" \
        "$MODULE_VERSION" \
        "$completion_timestamp" >> "$temporary_state_file"
    then
        rm -f -- "$temporary_state_file"

        log_error \
            "Failed to write DAIA module completion state."

        return "$DAIA_ERROR"
    fi

    if ! chmod --reference="$DAIA_FIRSTBOOT_STATE" \
        "$temporary_state_file"
    then
        rm -f -- "$temporary_state_file"

        log_error \
            "Failed to preserve DAIA state-file permissions."

        return "$DAIA_ERROR"
    fi

    if ! mv -f -- \
        "$temporary_state_file" \
        "$DAIA_FIRSTBOOT_STATE"
    then
        rm -f -- "$temporary_state_file"

        log_error \
            "Failed to commit DAIA module completion state."

        return "$DAIA_ERROR"
    fi

    log_success \
        "Recorded completion of module: ${MODULE_NAME} ${MODULE_VERSION}"

    return "$DAIA_SUCCESS"
}

############################################################
# _bootstrap_module_hook_cleanup
#
# Execute the module-specific cleanup hook when available.
#
# The hook is optional only to support cleanup after a module
# file fails before defining its complete lifecycle. A fully
# loaded and validated module must provide module_cleanup.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when cleanup succeeds or no hook exists.
#   The module cleanup return code otherwise.
############################################################

_bootstrap_module_hook_cleanup()
{
    local cleanup_status

    if ! declare -F module_cleanup >/dev/null 2>&1
    then
        return "$DAIA_SUCCESS"
    fi

    log_info \
        "Executing module_cleanup for module: ${MODULE_NAME:-unknown}"

    if module_cleanup
    then
        cleanup_status="$DAIA_SUCCESS"
    else
        cleanup_status=$?
    fi

    if (( cleanup_status != DAIA_SUCCESS ))
    then
        log_error \
            "Module cleanup failed for ${MODULE_NAME:-unknown} (status ${cleanup_status})."

        return "$cleanup_status"
    fi

    return "$DAIA_SUCCESS"
}

############################################################
# _bootstrap_framework_cleanup
#
# Remove framework-managed module metadata and lifecycle
# functions from the current shell.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when cleanup succeeds.
#   The framework cleanup return code otherwise.
############################################################

_bootstrap_framework_cleanup()
{
    local cleanup_status

    if framework_cleanup
    then
        cleanup_status="$DAIA_SUCCESS"
    else
        cleanup_status=$?
    fi

    if (( cleanup_status != DAIA_SUCCESS ))
    then
        log_error \
            "Framework cleanup failed with status ${cleanup_status}."

        return "$cleanup_status"
    fi

    return "$DAIA_SUCCESS"
}

############################################################
# _bootstrap_module_execute
#
# Load and execute one enabled module.
#
# Execution rules:
#
# - Primary lifecycle hooks stop at the first failure.
# - module_cleanup is attempted after primary execution.
# - Completion is recorded only after primary execution and
#   module cleanup both succeed.
# - framework_cleanup is always attempted after a module has
#   been loaded or a load attempt has failed.
# - The first meaningful failure code is preserved.
#
# Arguments:
#   $1 - Module name
#
# Returns:
#   DAIA_SUCCESS when the module completes successfully.
#   A standard DAIA or module return code on failure.
############################################################

_bootstrap_module_execute()
{
    local cleanup_status="$DAIA_SUCCESS"
    local configured_module_name="${1:-}"
    local hook_name
    local module_status="$DAIA_SUCCESS"
    local record_status="$DAIA_SUCCESS"

    if _bootstrap_module_load "$configured_module_name"
    then
        module_status="$DAIA_SUCCESS"
    else
        module_status=$?

        _bootstrap_framework_cleanup || true

        return "$module_status"
    fi

    if [[ "${MODULE_NAME:-}" != "$configured_module_name" ]]
    then
        log_error \
            "Module metadata name does not match its configuration entry: expected ${configured_module_name}, found ${MODULE_NAME:-undefined}"

        module_status="$DAIA_INVALID_ARGUMENT"
    fi

    if (( module_status == DAIA_SUCCESS ))
    then
        log_info \
            "Beginning DAIA module installation: ${MODULE_NAME} ${MODULE_VERSION:-unknown}"

        for hook_name in "${BOOTSTRAP_PRIMARY_LIFECYCLE[@]}"
        do
            if _bootstrap_hook_execute "$hook_name"
            then
                module_status="$DAIA_SUCCESS"
            else
                module_status=$?
                break
            fi
        done
    fi

    if _bootstrap_module_hook_cleanup
    then
        cleanup_status="$DAIA_SUCCESS"
    else
        cleanup_status=$?
    fi

    if (( module_status == DAIA_SUCCESS && cleanup_status != DAIA_SUCCESS ))
    then
        module_status="$cleanup_status"
    fi

    if (( module_status == DAIA_SUCCESS ))
    then
        if _bootstrap_completion_record
        then
            record_status="$DAIA_SUCCESS"
        else
            record_status=$?
            module_status="$record_status"
        fi
    fi

    if _bootstrap_framework_cleanup
    then
        cleanup_status="$DAIA_SUCCESS"
    else
        cleanup_status=$?

        if (( module_status == DAIA_SUCCESS ))
        then
            module_status="$cleanup_status"
        fi
    fi

    if (( module_status != DAIA_SUCCESS ))
    then
        log_error \
            "DAIA module failed: ${configured_module_name}"

        return "$module_status"
    fi

    log_success \
        "DAIA module completed successfully: ${configured_module_name}"

    return "$DAIA_SUCCESS"
}

############################################################
# main
#
# Execute the complete DAIA bootstrap process.
#
# Arguments:
#   None
#
# Returns:
#   DAIA_SUCCESS when every enabled module succeeds.
#   A standard DAIA or module return code on failure.
############################################################

main()
{
    local module_name
    local module_status
    local read_status
    local runtime_status
    local state_status
    local -a enabled_modules=()

    if _bootstrap_runtime_validate
    then
        runtime_status="$DAIA_SUCCESS"
    else
        runtime_status=$?
        return "$runtime_status"
    fi

    require_root

    if _bootstrap_state_prepare
    then
        state_status="$DAIA_SUCCESS"
    else
        state_status=$?
        return "$state_status"
    fi

    if _bootstrap_modules_read enabled_modules
    then
        read_status="$DAIA_SUCCESS"
    else
        read_status=$?
        return "$read_status"
    fi

    log_info \
        "Starting the DAIA bootstrap process."

    for module_name in "${enabled_modules[@]}"
    do
        if _bootstrap_module_execute "$module_name"
        then
            module_status="$DAIA_SUCCESS"
        else
            module_status=$?

            log_error \
                "DAIA bootstrap stopped after module failure: ${module_name}"

            return "$module_status"
        fi
    done

    log_success \
        "DAIA bootstrap completed successfully."

    return "$DAIA_SUCCESS"
}

main "$@"
