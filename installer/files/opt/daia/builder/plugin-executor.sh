#!/usr/bin/env bash

# DAIA Plugin Executor
#
# Executes an ordered plugin plan against a target root.
#
# Expected plugin layout:
#
#   <plugin-root>/
#       system/base/install.sh
#       desktop/environment/install.sh
#
# Expected execution plan format:
#
#   system/base
#   desktop/environment
#
# Blank lines and lines beginning with '#' are ignored.

declare -g __DAIA_PLUGIN_EXECUTOR_INITIALIZED=0
declare -g __DAIA_PLUGIN_EXECUTOR_PLUGIN_ROOT=""
declare -g __DAIA_PLUGIN_EXECUTOR_TARGET_ROOT=""

_daia_plugin_executor_error() {
    printf 'DAIA plugin-executor error: %s\n' "$*" >&2
}

_daia_plugin_executor_require_initialization() {
    if [[ "$__DAIA_PLUGIN_EXECUTOR_INITIALIZED" -ne 1 ]]; then
        _daia_plugin_executor_error "executor is not initialized"
        return 1
    fi
}

_daia_plugin_executor_validate_plugin_id() {
    local plugin_id="${1-}"

    if [[ -z "$plugin_id" ]]; then
        _daia_plugin_executor_error "plugin ID must not be empty"
        return 1
    fi

    if [[ ! "$plugin_id" =~ ^[a-z0-9]+([._-][a-z0-9]+)*/[a-z0-9]+([._-][a-z0-9]+)*$ ]]; then
        _daia_plugin_executor_error "invalid plugin ID: $plugin_id"
        return 1
    fi
}

_daia_plugin_executor_normalize_directory() {
    local directory="${1-}"

    (
        cd -- "$directory" &&
            pwd
    )
}

_daia_plugin_executor_plugin_directory() {
    local plugin_id="${1-}"

    printf '%s/%s\n' \
        "$__DAIA_PLUGIN_EXECUTOR_PLUGIN_ROOT" \
        "$plugin_id"
}

daia_plugin_executor_init() {
    local plugin_root="${1-}"
    local target_root="${2-}"
    local normalized_plugin_root
    local normalized_target_root

    if [[ -z "$plugin_root" ]]; then
        _daia_plugin_executor_error "plugin root must not be empty"
        return 1
    fi

    if [[ ! -d "$plugin_root" ]]; then
        _daia_plugin_executor_error \
            "plugin root directory does not exist: $plugin_root"
        return 1
    fi

    if [[ -z "$target_root" ]]; then
        _daia_plugin_executor_error "target root must not be empty"
        return 1
    fi

    if [[ ! -d "$target_root" ]]; then
        _daia_plugin_executor_error \
            "target root directory does not exist: $target_root"
        return 1
    fi

    normalized_plugin_root="$(
        _daia_plugin_executor_normalize_directory "$plugin_root"
    )" || return 1

    normalized_target_root="$(
        _daia_plugin_executor_normalize_directory "$target_root"
    )" || return 1

    __DAIA_PLUGIN_EXECUTOR_PLUGIN_ROOT="$normalized_plugin_root"
    __DAIA_PLUGIN_EXECUTOR_TARGET_ROOT="$normalized_target_root"
    __DAIA_PLUGIN_EXECUTOR_INITIALIZED=1
}

daia_plugin_executor_clear() {
    __DAIA_PLUGIN_EXECUTOR_INITIALIZED=0
    __DAIA_PLUGIN_EXECUTOR_PLUGIN_ROOT=""
    __DAIA_PLUGIN_EXECUTOR_TARGET_ROOT=""
}

daia_plugin_executor_validate_plugin() {
    local plugin_id="${1-}"
    local plugin_directory
    local install_script

    _daia_plugin_executor_require_initialization || return 1
    _daia_plugin_executor_validate_plugin_id "$plugin_id" || return 1

    plugin_directory="$(
        _daia_plugin_executor_plugin_directory "$plugin_id"
    )"

    if [[ ! -d "$plugin_directory" ]]; then
        _daia_plugin_executor_error \
            "plugin directory does not exist: $plugin_directory"
        return 1
    fi

    install_script="$plugin_directory/install.sh"

    if [[ ! -f "$install_script" ]]; then
        _daia_plugin_executor_error \
            "plugin install script does not exist: $install_script"
        return 1
    fi

    if [[ ! -r "$install_script" ]]; then
        _daia_plugin_executor_error \
            "plugin install script is not readable: $install_script"
        return 1
    fi
}

daia_plugin_executor_execute_plugin() {
    local plugin_id="${1-}"
    local plugin_directory
    local install_script
    local status

    _daia_plugin_executor_require_initialization || return 1
    daia_plugin_executor_validate_plugin "$plugin_id" || return 1

    plugin_directory="$(
        _daia_plugin_executor_plugin_directory "$plugin_id"
    )"

    install_script="$plugin_directory/install.sh"

    printf 'DAIA plugin-executor: executing %s\n' "$plugin_id"

    (
        cd -- "$plugin_directory" || exit 1

        export DAIA_PLUGIN_ID="$plugin_id"
        export DAIA_PLUGIN_DIR="$plugin_directory"
        export DAIA_TARGET_ROOT="$__DAIA_PLUGIN_EXECUTOR_TARGET_ROOT"

        bash "$install_script"
    )

    status=$?

    if [[ "$status" -ne 0 ]]; then
        _daia_plugin_executor_error \
            "plugin failed with status $status: $plugin_id"
        return "$status"
    fi

    printf 'DAIA plugin-executor: completed %s\n' "$plugin_id"
}

daia_plugin_executor_execute_plan() {
    local plan_file="${1-}"
    local plugin_id
    local line_number=0
    local -A executed_plugins=()

    _daia_plugin_executor_require_initialization || return 1

    if [[ -z "$plan_file" ]]; then
        _daia_plugin_executor_error "execution plan file must not be empty"
        return 1
    fi

    if [[ ! -f "$plan_file" ]]; then
        _daia_plugin_executor_error \
            "execution plan file does not exist: $plan_file"
        return 1
    fi

    if [[ ! -r "$plan_file" ]]; then
        _daia_plugin_executor_error \
            "execution plan file is not readable: $plan_file"
        return 1
    fi

    while IFS= read -r plugin_id || [[ -n "$plugin_id" ]]; do
        line_number=$((line_number + 1))

        plugin_id="${plugin_id%$'\r'}"

        if [[ -z "$plugin_id" ]]; then
            continue
        fi

        if [[ "$plugin_id" == \#* ]]; then
            continue
        fi

        _daia_plugin_executor_validate_plugin_id "$plugin_id" || {
            _daia_plugin_executor_error \
                "invalid execution plan entry at line $line_number"
            return 1
        }

        if [[ -n "${executed_plugins[$plugin_id]+defined}" ]]; then
            _daia_plugin_executor_error \
                "duplicate plugin in execution plan at line $line_number: $plugin_id"
            return 1
        fi

        daia_plugin_executor_execute_plugin "$plugin_id" || return 1

        executed_plugins["$plugin_id"]=1
    done < "$plan_file"
}
