#!/usr/bin/env bash
#
# DAIA Desired State Manager
#
# Owns the desired-state resource graph.
#
# Public API:
#   daia_state_init
#   daia_state_add_resource <id> <type> <desired-state> [provider]
#   daia_state_set_property <resource-id> <key> <value>
#   daia_state_add_dependency <resource-id> <dependency-id>
#   daia_state_resource_exists <resource-id>
#   daia_state_resource_ids
#   daia_state_get_field <resource-id> <field>
#   daia_state_get_property <resource-id> <key>
#   daia_state_dependencies <resource-id>
#   daia_state_seal
#   daia_state_is_sealed
#

if [[ -n "${__DAIA_DESIRED_STATE_MODULE_INITIALIZED:-}" ]]; then
# Allow this file to be either sourced or executed.
# shellcheck disable=SC2317
    return 0 2>/dev/null || exit 0
fi

__DAIA_DESIRED_STATE_MODULE_INITIALIZED=1

declare -gA __DAIA_STATE_RESOURCE_TYPES=()
declare -gA __DAIA_STATE_RESOURCE_STATES=()
declare -gA __DAIA_STATE_RESOURCE_PROVIDERS=()
declare -gA __DAIA_STATE_RESOURCE_PROPERTIES=()
declare -gA __DAIA_STATE_RESOURCE_DEPENDENCIES=()
declare -ga __DAIA_STATE_RESOURCE_ORDER=()

__DAIA_STATE_INITIALIZED=0
__DAIA_STATE_SEALED=0


###############################################################################
# Internal helpers
###############################################################################

_daia_state_error() {
    printf 'DAIA desired-state error: %s\n' "$*" >&2
}


_daia_state_valid_identifier() {
    local identifier="${1:-}"

    [[ "$identifier" =~ ^[A-Za-z0-9_][A-Za-z0-9_-]*$ ]]
}


_daia_state_valid_property_name() {
    local property_name="${1:-}"

    [[ "$property_name" =~ ^[A-Za-z_][A-Za-z0-9_-]*$ ]]
}


_daia_state_require_initialized() {
    if [[ "$__DAIA_STATE_INITIALIZED" -ne 1 ]]; then
        _daia_state_error "desired state has not been initialized"
        return 1
    fi
}


_daia_state_require_mutable() {
    _daia_state_require_initialized || return 1

    if [[ "$__DAIA_STATE_SEALED" -eq 1 ]]; then
        _daia_state_error "desired state is sealed and cannot be modified"
        return 1
    fi
}


_daia_state_require_resource() {
    local resource_id="${1:-}"

    if ! daia_state_resource_exists "$resource_id"; then
        _daia_state_error "unknown resource: $resource_id"
        return 1
    fi
}


###############################################################################
# Public API
###############################################################################

daia_state_init() {
    __DAIA_STATE_RESOURCE_TYPES=()
    __DAIA_STATE_RESOURCE_STATES=()
    __DAIA_STATE_RESOURCE_PROVIDERS=()
    __DAIA_STATE_RESOURCE_PROPERTIES=()
    __DAIA_STATE_RESOURCE_DEPENDENCIES=()
    __DAIA_STATE_RESOURCE_ORDER=()

    __DAIA_STATE_INITIALIZED=1
    __DAIA_STATE_SEALED=0
}


daia_state_add_resource() {
    local resource_id="${1:-}"
    local resource_type="${2:-}"
    local desired_state="${3:-}"
    local provider="${4:-}"

    _daia_state_require_mutable || return 1

    if [[ -z "$resource_id" ]]; then
        _daia_state_error "resource ID was not provided"
        return 2
    fi

    if ! _daia_state_valid_identifier "$resource_id"; then
        _daia_state_error "invalid resource ID: $resource_id"
        return 2
    fi

    if [[ -z "$resource_type" ]]; then
        _daia_state_error "resource type was not provided"
        return 2
    fi

    if ! _daia_state_valid_identifier "$resource_type"; then
        _daia_state_error "invalid resource type: $resource_type"
        return 2
    fi

    if [[ -z "$desired_state" ]]; then
        _daia_state_error \
            "desired state was not provided for resource: $resource_id"
        return 2
    fi

    if daia_state_resource_exists "$resource_id"; then
        _daia_state_error "duplicate resource ID: $resource_id"
        return 1
    fi

    __DAIA_STATE_RESOURCE_TYPES["$resource_id"]="$resource_type"
    __DAIA_STATE_RESOURCE_STATES["$resource_id"]="$desired_state"
    __DAIA_STATE_RESOURCE_PROVIDERS["$resource_id"]="$provider"
    __DAIA_STATE_RESOURCE_DEPENDENCIES["$resource_id"]=""
    __DAIA_STATE_RESOURCE_ORDER+=("$resource_id")
}


daia_state_set_property() {
    local resource_id="${1:-}"
    local property_name="${2:-}"
    local property_value="${3-}"
    local property_key

    _daia_state_require_mutable || return 1
    _daia_state_require_resource "$resource_id" || return 1

    if [[ -z "$property_name" ]]; then
        _daia_state_error "property name was not provided"
        return 2
    fi

    if ! _daia_state_valid_property_name "$property_name"; then
        _daia_state_error "invalid property name: $property_name"
        return 2
    fi

    property_key="${resource_id}:${property_name}"
    __DAIA_STATE_RESOURCE_PROPERTIES["$property_key"]="$property_value"
}


daia_state_add_dependency() {
    local resource_id="${1:-}"
    local dependency_id="${2:-}"
    local existing_dependencies
    local existing_dependency

    _daia_state_require_mutable || return 1
    _daia_state_require_resource "$resource_id" || return 1
    _daia_state_require_resource "$dependency_id" || return 1

    if [[ "$resource_id" == "$dependency_id" ]]; then
        _daia_state_error \
            "resource cannot depend on itself: $resource_id"
        return 1
    fi

    existing_dependencies="${__DAIA_STATE_RESOURCE_DEPENDENCIES[$resource_id]}"

    while IFS= read -r existing_dependency; do
        [[ -z "$existing_dependency" ]] && continue

        if [[ "$existing_dependency" == "$dependency_id" ]]; then
            return 0
        fi
    done <<< "$existing_dependencies"

    if [[ -n "$existing_dependencies" ]]; then
        __DAIA_STATE_RESOURCE_DEPENDENCIES["$resource_id"]+=$'\n'
    fi

    __DAIA_STATE_RESOURCE_DEPENDENCIES["$resource_id"]+="$dependency_id"
}


daia_state_resource_exists() {
    local resource_id="${1:-}"

    [[ -n "$resource_id" ]] &&
        [[ -n "${__DAIA_STATE_RESOURCE_TYPES[$resource_id]+defined}" ]]
}


daia_state_resource_ids() {
    local resource_id

    _daia_state_require_initialized || return 1

    for resource_id in "${__DAIA_STATE_RESOURCE_ORDER[@]}"; do
        printf '%s\n' "$resource_id"
    done
}


daia_state_get_field() {
    local resource_id="${1:-}"
    local field_name="${2:-}"

    _daia_state_require_initialized || return 1
    _daia_state_require_resource "$resource_id" || return 1

    case "$field_name" in
        id)
            printf '%s\n' "$resource_id"
            ;;
        type)
            printf '%s\n' \
                "${__DAIA_STATE_RESOURCE_TYPES[$resource_id]}"
            ;;
        desired_state)
            printf '%s\n' \
                "${__DAIA_STATE_RESOURCE_STATES[$resource_id]}"
            ;;
        provider)
            printf '%s\n' \
                "${__DAIA_STATE_RESOURCE_PROVIDERS[$resource_id]}"
            ;;
        *)
            _daia_state_error "unknown resource field: $field_name"
            return 2
            ;;
    esac
}


daia_state_get_property() {
    local resource_id="${1:-}"
    local property_name="${2:-}"
    local property_key

    _daia_state_require_initialized || return 1
    _daia_state_require_resource "$resource_id" || return 1

    if ! _daia_state_valid_property_name "$property_name"; then
        _daia_state_error "invalid property name: $property_name"
        return 2
    fi

    property_key="${resource_id}:${property_name}"

    if [[ -z "${__DAIA_STATE_RESOURCE_PROPERTIES[$property_key]+defined}" ]]; then
        return 1
    fi

    printf '%s\n' \
        "${__DAIA_STATE_RESOURCE_PROPERTIES[$property_key]}"
}


daia_state_dependencies() {
    local resource_id="${1:-}"
    local dependencies

    _daia_state_require_initialized || return 1
    _daia_state_require_resource "$resource_id" || return 1

    dependencies="${__DAIA_STATE_RESOURCE_DEPENDENCIES[$resource_id]}"

    [[ -z "$dependencies" ]] || printf '%s\n' "$dependencies"
}


daia_state_seal() {
    _daia_state_require_initialized || return 1
    __DAIA_STATE_SEALED=1
}


daia_state_is_sealed() {
    [[ "$__DAIA_STATE_INITIALIZED" -eq 1 ]] &&
        [[ "$__DAIA_STATE_SEALED" -eq 1 ]]
}
