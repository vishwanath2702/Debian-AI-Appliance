#!/usr/bin/env bash
#
# DAIA Workspace Builder
#
# Creates, validates, exposes, and safely destroys a DAIA build workspace.
#
# Workspace layout:
#
#   workspace/
#   ├── cache/
#   ├── logs/
#   ├── plan/
#   ├── root/
#   ├── state/
#   ├── tmp/
#   └── work/
#

declare -g __DAIA_WORKSPACE_INITIALIZED=0
declare -g __DAIA_WORKSPACE=""

_daia_workspace_builder_error() {
    printf 'DAIA workspace-builder error: %s\n' "$*" >&2
}

_daia_workspace_builder_require_initialization() {
    if [[ "$__DAIA_WORKSPACE_INITIALIZED" -ne 1 ]]; then
        _daia_workspace_builder_error "workspace builder is not initialized"
        return 1
    fi
}

_daia_workspace_builder_validate_path() {
    local workspace="${1-}"

    if [[ -z "$workspace" ]]; then
        _daia_workspace_builder_error "workspace path must not be empty"
        return 1
    fi

    if [[ "$workspace" == "/" ]]; then
        _daia_workspace_builder_error "workspace path must not be the filesystem root"
        return 1
    fi

    if [[ "$workspace" == "." || "$workspace" == ".." ]]; then
        _daia_workspace_builder_error \
            "workspace path must not be '.' or '..'"
        return 1
    fi
}

_daia_workspace_builder_normalize_path() {
    local path="${1-}"
    local parent
    local name
    local normalized_parent

    _daia_workspace_builder_validate_path "$path" || return 1

    if [[ -d "$path" ]]; then
        (
            cd -- "$path" || exit 1
            pwd -P
        )
        return
    fi

    parent="$(dirname -- "$path")"
    name="$(basename -- "$path")"

    if [[ "$name" == "." || "$name" == ".." || -z "$name" ]]; then
        _daia_workspace_builder_error \
            "invalid workspace directory name: $name"
        return 1
    fi

    if [[ ! -d "$parent" ]]; then
        _daia_workspace_builder_error \
            "workspace parent directory does not exist: $parent"
        return 1
    fi

    normalized_parent="$(
        cd -- "$parent" &&
            pwd -P
    )" || {
        _daia_workspace_builder_error \
            "unable to normalize workspace parent directory: $parent"
        return 1
    }

    printf '%s/%s\n' "$normalized_parent" "$name"
}

_daia_workspace_builder_marker() {
    printf '%s/.daia-workspace\n' "$__DAIA_WORKSPACE"
}

_daia_workspace_builder_path() {
    local directory_name="${1-}"

    _daia_workspace_builder_require_initialization || return 1

    printf '%s/%s\n' "$__DAIA_WORKSPACE" "$directory_name"
}

_daia_workspace_builder_has_valid_marker() {
    local marker
    local marker_value

    marker="$(_daia_workspace_builder_marker)"

    if [[ ! -f "$marker" || ! -r "$marker" ]]; then
        return 1
    fi

    IFS= read -r marker_value < "$marker" || return 1

    [[ "$marker_value" == "$__DAIA_WORKSPACE" ]]
}

_daia_workspace_builder_has_required_directories() {
    local directory_name

    for directory_name in \
        cache \
        logs \
        plan \
        root \
        state \
        tmp \
        work
    do
        if [[ ! -d "$__DAIA_WORKSPACE/$directory_name" ]]; then
            return 1
        fi
    done
}

daia_workspace_builder_init() {
    local workspace="${1-}"
    local normalized_workspace

    if [[ "$__DAIA_WORKSPACE_INITIALIZED" -eq 1 ]]; then
        _daia_workspace_builder_error \
            "workspace builder is already initialized"
        return 1
    fi

    normalized_workspace="$(
        _daia_workspace_builder_normalize_path "$workspace"
    )" || return 1

    _daia_workspace_builder_validate_path "$normalized_workspace" ||
        return 1

    __DAIA_WORKSPACE="$normalized_workspace"
    __DAIA_WORKSPACE_INITIALIZED=1
}

daia_workspace_builder_clear() {
    __DAIA_WORKSPACE_INITIALIZED=0
    __DAIA_WORKSPACE=""
}

daia_workspace_builder_create() {
    local marker
    local directory_name

    _daia_workspace_builder_require_initialization || return 1

    if [[ -e "$__DAIA_WORKSPACE" ]]; then
        _daia_workspace_builder_error \
            "workspace path already exists: $__DAIA_WORKSPACE"
        return 1
    fi

    if ! mkdir -- "$__DAIA_WORKSPACE"; then
        _daia_workspace_builder_error \
            "unable to create workspace: $__DAIA_WORKSPACE"
        return 1
    fi

    for directory_name in \
        cache \
        logs \
        plan \
        root \
        state \
        tmp \
        work
    do
        if ! mkdir -- "$__DAIA_WORKSPACE/$directory_name"; then
            _daia_workspace_builder_error \
                "unable to create workspace directory: $directory_name"

            rm -rf -- "$__DAIA_WORKSPACE"
            return 1
        fi
    done

    marker="$(_daia_workspace_builder_marker)"

    if ! printf '%s\n' "$__DAIA_WORKSPACE" > "$marker"; then
        _daia_workspace_builder_error \
            "unable to create workspace ownership marker"

        rm -rf -- "$__DAIA_WORKSPACE"
        return 1
    fi

    printf '%s\n' "$__DAIA_WORKSPACE"
}

daia_workspace_builder_destroy() {
    local normalized_workspace

    _daia_workspace_builder_require_initialization || return 1

    _daia_workspace_builder_validate_path "$__DAIA_WORKSPACE" ||
        return 1

    if [[ ! -d "$__DAIA_WORKSPACE" ]]; then
        _daia_workspace_builder_error \
            "workspace does not exist: $__DAIA_WORKSPACE"
        return 1
    fi

    normalized_workspace="$(
        cd -- "$__DAIA_WORKSPACE" &&
            pwd -P
    )" || {
        _daia_workspace_builder_error \
            "unable to normalize existing workspace path"
        return 1
    }

    if [[ "$normalized_workspace" != "$__DAIA_WORKSPACE" ]]; then
        _daia_workspace_builder_error \
            "workspace path no longer matches initialized path"
        return 1
    fi

    if ! _daia_workspace_builder_has_valid_marker; then
        _daia_workspace_builder_error \
            "workspace ownership marker is missing or invalid"
        return 1
    fi

    if ! rm -rf -- "$__DAIA_WORKSPACE"; then
        _daia_workspace_builder_error \
            "unable to destroy workspace: $__DAIA_WORKSPACE"
        return 1
    fi

    daia_workspace_builder_clear
}

daia_workspace_builder_exists() {
    _daia_workspace_builder_require_initialization || return 1

    [[ -d "$__DAIA_WORKSPACE" ]] &&
        _daia_workspace_builder_has_valid_marker &&
        _daia_workspace_builder_has_required_directories
}

daia_workspace_builder_root() {
    _daia_workspace_builder_path "root"
}

daia_workspace_builder_work() {
    _daia_workspace_builder_path "work"
}

daia_workspace_builder_cache() {
    _daia_workspace_builder_path "cache"
}

daia_workspace_builder_logs() {
    _daia_workspace_builder_path "logs"
}

daia_workspace_builder_plan() {
    _daia_workspace_builder_path "plan"
}

daia_workspace_builder_state() {
    _daia_workspace_builder_path "state"
}

daia_workspace_builder_tmp() {
    _daia_workspace_builder_path "tmp"
}
