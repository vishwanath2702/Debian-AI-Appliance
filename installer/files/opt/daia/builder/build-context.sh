#!/usr/bin/env bash
#
# DAIA Build Context
#
# Stores the immutable configuration for a single build.
#

declare -g __DAIA_BUILD_CONTEXT_INITIALIZED=0
declare -g __DAIA_BUILD_CONTEXT_VALID=0
declare -g __DAIA_BUILD_CONTEXT_LOCKED=0

declare -gA __DAIA_BUILD_CONTEXT=()

readonly __DAIA_BUILD_CONTEXT_REQUIRED_FIELDS=(
    workspace
    plugin_root
    profile
    execution_plan
    architecture
    output_directory
    build_name
    version
)

_daia_build_context_error() {
    printf 'DAIA build-context error: %s\n' "$*" >&2
}

_daia_build_context_require_initialization() {
    if [[ "$__DAIA_BUILD_CONTEXT_INITIALIZED" -ne 1 ]]; then
        _daia_build_context_error "build context is not initialized"
        return 1
    fi
}

_daia_build_context_require_unlocked() {
    if [[ "$__DAIA_BUILD_CONTEXT_LOCKED" -eq 1 ]]; then
        _daia_build_context_error "build context is locked"
        return 1
    fi
}

_daia_build_context_normalize_directory() {
    local directory="${1-}"

    if [[ -z "$directory" ]]; then
        _daia_build_context_error "directory must not be empty"
        return 1
    fi

    (
        cd -- "$directory" &&
            pwd -P
    )
}

_daia_build_context_set() {
    local key="${1-}"
    local value="${2-}"

    _daia_build_context_require_initialization || return 1
    _daia_build_context_require_unlocked || return 1

    if [[ -v "__DAIA_BUILD_CONTEXT[$key]" ]]; then
        _daia_build_context_error "$key has already been set"
        return 1
    fi

    __DAIA_BUILD_CONTEXT["$key"]="$value"

    return 0
}

_daia_build_context_get() {
    local key="${1-}"

    _daia_build_context_require_initialization || return 1

    printf '%s\n' "${__DAIA_BUILD_CONTEXT[$key]-}"

    return 0
}
daia_build_context_init() {
    if [[ "$__DAIA_BUILD_CONTEXT_INITIALIZED" -eq 1 ]]; then
        _daia_build_context_error "build context is already initialized"
        return 1
    fi

    __DAIA_BUILD_CONTEXT_INITIALIZED=1
    __DAIA_BUILD_CONTEXT_VALID=0
    __DAIA_BUILD_CONTEXT_LOCKED=0

    __DAIA_BUILD_CONTEXT=()

    return 0
}

daia_build_context_clear() {
    _daia_build_context_require_initialization || return 1

    __DAIA_BUILD_CONTEXT_INITIALIZED=0
    __DAIA_BUILD_CONTEXT_VALID=0
    __DAIA_BUILD_CONTEXT_LOCKED=0

    __DAIA_BUILD_CONTEXT=()

    return 0
}

daia_build_context_is_initialized() {
    [[ "$__DAIA_BUILD_CONTEXT_INITIALIZED" -eq 1 ]]
}

daia_build_context_is_valid() {
    [[ "$__DAIA_BUILD_CONTEXT_VALID" -eq 1 ]]
}

daia_build_context_is_locked() {
    [[ "$__DAIA_BUILD_CONTEXT_LOCKED" -eq 1 ]]
}


daia_build_context_set_workspace() {
    local workspace="${1-}"

    workspace="$(_daia_build_context_normalize_directory "$workspace")" ||
        return 1

    _daia_build_context_set workspace "$workspace"
}

daia_build_context_set_plugin_root() {
    local plugin_root="${1-}"

    plugin_root="$(_daia_build_context_normalize_directory "$plugin_root")" ||
        return 1

    _daia_build_context_set plugin_root "$plugin_root"
}

daia_build_context_set_profile() {
    local profile="${1-}"

    if [[ -z "$profile" ]]; then
        _daia_build_context_error "profile must not be empty"
        return 1
    fi

    _daia_build_context_set profile "$profile"
}

daia_build_context_set_execution_plan() {
    local execution_plan="${1-}"

    if [[ -z "$execution_plan" ]]; then
        _daia_build_context_error "execution plan must not be empty"
        return 1
    fi

    _daia_build_context_set execution_plan "$execution_plan"
}

daia_build_context_set_architecture() {
    local architecture="${1-}"

    case "$architecture" in
        x86_64|aarch64)
            ;;
        *)
            _daia_build_context_error \
                "unsupported architecture: $architecture"
            return 1
            ;;
    esac

    _daia_build_context_set architecture "$architecture"
}

daia_build_context_set_output_directory() {
    local output_directory="${1-}"

    output_directory="$(
        _daia_build_context_normalize_directory "$output_directory"
    )" || return 1

    _daia_build_context_set output_directory "$output_directory"
}

daia_build_context_set_build_name() {
    local build_name="${1-}"

    if [[ -z "$build_name" ]]; then
        _daia_build_context_error "build name must not be empty"
        return 1
    fi

    _daia_build_context_set build_name "$build_name"
}

daia_build_context_set_version() {
    local version="${1-}"

    if [[ -z "$version" ]]; then
        _daia_build_context_error "version must not be empty"
        return 1
    fi

    _daia_build_context_set version "$version"
}

daia_build_context_workspace() {
    _daia_build_context_get workspace
}

daia_build_context_plugin_root() {
    _daia_build_context_get plugin_root
}

daia_build_context_profile() {
    _daia_build_context_get profile
}

daia_build_context_execution_plan() {
    _daia_build_context_get execution_plan
}

daia_build_context_architecture() {
    _daia_build_context_get architecture
}

daia_build_context_output_directory() {
    _daia_build_context_get output_directory
}

daia_build_context_build_name() {
    _daia_build_context_get build_name
}

daia_build_context_version() {
    _daia_build_context_get version
}

daia_build_context_is_complete() {
    local field

    _daia_build_context_require_initialization || return 1

    for field in "${__DAIA_BUILD_CONTEXT_REQUIRED_FIELDS[@]}"; do
        if [[ ! -v "__DAIA_BUILD_CONTEXT[$field]" ]]; then
            return 1
        fi

        if [[ -z "${__DAIA_BUILD_CONTEXT[$field]}" ]]; then
            return 1
        fi
    done

    return 0
}

daia_build_context_validate() {
    local build_name
    local version

    _daia_build_context_require_initialization || return 1
    _daia_build_context_require_unlocked || return 1

    if ! daia_build_context_is_complete; then
        _daia_build_context_error \
            "build context is incomplete"
        return 1
    fi

    build_name="${__DAIA_BUILD_CONTEXT[build_name]}"
    version="${__DAIA_BUILD_CONTEXT[version]}"

    if [[ ! "$build_name" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ ]]; then
        _daia_build_context_error \
            "invalid build name: $build_name"
        return 1
    fi

    if [[ ! "$version" =~ ^[a-zA-Z0-9][a-zA-Z0-9._+-]*$ ]]; then
        _daia_build_context_error \
            "invalid version: $version"
        return 1
    fi

    __DAIA_BUILD_CONTEXT_VALID=1
    __DAIA_BUILD_CONTEXT_LOCKED=1

    return 0
}
