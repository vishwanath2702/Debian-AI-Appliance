#!/usr/bin/env bash
#
# DAIA Capability Registry
#
# Stores validated bidirectional relationships between capabilities and
# provider plugins. The registry contains no provider-selection or
# dependency-resolution policy.
#
# Lifecycle API:
#   daia_capability_registry_init
#   daia_capability_registry_clear
#   daia_capability_registry_register <plugin-id> <capability> [capability...]
#
# Lookup API:
#   daia_capability_exists <capability>
#   daia_plugin_exists <plugin-id>
#   daia_plugin_provides_capability <plugin-id> <capability>
#   daia_capability_get_providers <capability>
#   daia_capability_provider_count <capability>
#   daia_plugin_get_capabilities <plugin-id>
#   daia_plugin_capability_count <plugin-id>
#   daia_capability_registry_get_capabilities
#   daia_capability_registry_get_plugins
#   daia_capability_registry_capability_count
#   daia_capability_registry_plugin_count
#

if [[ -n "${__DAIA_CAPABILITY_REGISTRY_MODULE_INITIALIZED:-}" ]]; then
    if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then
        return 0
    fi

    exit 0
fi

__DAIA_CAPABILITY_REGISTRY_MODULE_INITIALIZED=1

readonly __DAIA_CAPABILITY_REGISTRY_KEY_SEPARATOR=$'\x1f'

declare -gA __DAIA_CAPABILITY_REGISTRY_CAPABILITIES=()
declare -gA __DAIA_CAPABILITY_REGISTRY_PLUGINS=()
declare -gA __DAIA_CAPABILITY_REGISTRY_LINKS=()

__DAIA_CAPABILITY_REGISTRY_INITIALIZED=0

###############################################################################
# Internal helpers
###############################################################################

_daia_capability_registry_error() {
    printf 'DAIA capability-registry error: %s\n' "$*" >&2
}

_daia_capability_registry_require_initialized() {
    if [[ "$__DAIA_CAPABILITY_REGISTRY_INITIALIZED" -ne 1 ]]; then
        _daia_capability_registry_error "registry has not been initialized"
        return 1
    fi
}

_daia_capability_registry_validate_argument_count() {
    local expected="$1"
    local function_name="$2"
    local actual="$3"

    if [[ "$actual" -ne "$expected" ]]; then
        _daia_capability_registry_error \
            "$function_name expects $expected argument(s); received $actual"
        return 1
    fi
}

_daia_capability_registry_validate_plugin_id() {
    local plugin_id="${1-}"

    if [[ -z "$plugin_id" ]]; then
        _daia_capability_registry_error "plugin ID must not be empty"
        return 1
    fi

    if [[ ! "$plugin_id" =~ ^[a-z0-9_-]+/[a-z0-9._-]+$ ]]; then
        _daia_capability_registry_error "invalid plugin ID: $plugin_id"
        return 1
    fi
}

_daia_capability_registry_validate_capability() {
    local capability="${1-}"

    if [[ -z "$capability" ]]; then
        _daia_capability_registry_error "capability must not be empty"
        return 1
    fi

    if [[ ! "$capability" =~ ^[a-z0-9]+([._-][a-z0-9]+)*$ ]]; then
        _daia_capability_registry_error "invalid capability: $capability"
        return 1
    fi
}

_daia_capability_registry_link_key() {
    local capability="${1-}"
    local plugin_id="${2-}"

    printf '%s%s%s\n' \
        "$capability" \
        "$__DAIA_CAPABILITY_REGISTRY_KEY_SEPARATOR" \
        "$plugin_id"
}

_daia_capability_registry_set_capability() {
    local capability="${1-}"

    __DAIA_CAPABILITY_REGISTRY_CAPABILITIES["$capability"]=1
}

_daia_capability_registry_set_plugin() {
    local plugin_id="${1-}"

    __DAIA_CAPABILITY_REGISTRY_PLUGINS["$plugin_id"]=1
}

_daia_capability_registry_set_link() {
    local capability="${1-}"
    local plugin_id="${2-}"
    local link_key

    link_key="$(_daia_capability_registry_link_key "$capability" "$plugin_id")"
    __DAIA_CAPABILITY_REGISTRY_LINKS["$link_key"]=1
}

_daia_capability_registry_link_exists() {
    local capability="${1-}"
    local plugin_id="${2-}"
    local link_key

    link_key="$(_daia_capability_registry_link_key "$capability" "$plugin_id")"
    [[ -n "${__DAIA_CAPABILITY_REGISTRY_LINKS[$link_key]+defined}" ]]
}

_daia_capability_registry_validate_registration() {
    local plugin_id="${1-}"
    local capability
    local -A request_capabilities=()

    _daia_capability_registry_validate_plugin_id "$plugin_id" || return 1

    if [[ "$#" -lt 2 ]]; then
        _daia_capability_registry_error \
            "at least one capability is required for plugin: $plugin_id"
        return 1
    fi

    shift

    for capability in "$@"; do
        _daia_capability_registry_validate_capability "$capability" || return 1

        if [[ -n "${request_capabilities[$capability]+defined}" ]]; then
            _daia_capability_registry_error \
                "duplicate capability for plugin $plugin_id: $capability"
            return 1
        fi

        if _daia_capability_registry_link_exists "$capability" "$plugin_id"; then
            _daia_capability_registry_error \
                "capability already registered for plugin $plugin_id: $capability"
            return 1
        fi

        request_capabilities["$capability"]=1
    done
}

_daia_capability_registry_emit_sorted() {
    if [[ "$#" -eq 0 ]]; then
        return 0
    fi

    printf '%s\n' "$@" | LC_ALL=C sort -u
}

###############################################################################
# Lifecycle API
###############################################################################

_daia_capability_registry_reset() {
    __DAIA_CAPABILITY_REGISTRY_CAPABILITIES=()
    __DAIA_CAPABILITY_REGISTRY_PLUGINS=()
    __DAIA_CAPABILITY_REGISTRY_LINKS=()
    __DAIA_CAPABILITY_REGISTRY_INITIALIZED=1
}

daia_capability_registry_init() {
    _daia_capability_registry_validate_argument_count \
        0 "daia_capability_registry_init" "$#" || return 1

    _daia_capability_registry_reset
}

daia_capability_registry_clear() {
    _daia_capability_registry_validate_argument_count \
        0 "daia_capability_registry_clear" "$#" || return 1
    _daia_capability_registry_require_initialized || return 1

    _daia_capability_registry_reset
}

daia_capability_registry_register() {
    local plugin_id="${1-}"
    local capability

    _daia_capability_registry_require_initialized || return 1
    _daia_capability_registry_validate_registration "$@" || return 1

    shift
    _daia_capability_registry_set_plugin "$plugin_id"

    for capability in "$@"; do
        _daia_capability_registry_set_capability "$capability"
        _daia_capability_registry_set_link "$capability" "$plugin_id"
    done
}

###############################################################################
# Lookup API
###############################################################################

daia_capability_exists() {
    local capability="${1-}"

    _daia_capability_registry_validate_argument_count \
        1 "daia_capability_exists" "$#" || return 1
    _daia_capability_registry_require_initialized || return 1
    _daia_capability_registry_validate_capability "$capability" || return 1

    [[ -n "${__DAIA_CAPABILITY_REGISTRY_CAPABILITIES[$capability]+defined}" ]]
}

daia_plugin_exists() {
    local plugin_id="${1-}"

    _daia_capability_registry_validate_argument_count \
        1 "daia_plugin_exists" "$#" || return 1
    _daia_capability_registry_require_initialized || return 1
    _daia_capability_registry_validate_plugin_id "$plugin_id" || return 1

    [[ -n "${__DAIA_CAPABILITY_REGISTRY_PLUGINS[$plugin_id]+defined}" ]]
}

daia_plugin_provides_capability() {
    local plugin_id="${1-}"
    local capability="${2-}"

    _daia_capability_registry_validate_argument_count \
        2 "daia_plugin_provides_capability" "$#" || return 1
    _daia_capability_registry_require_initialized || return 1
    _daia_capability_registry_validate_plugin_id "$plugin_id" || return 1
    _daia_capability_registry_validate_capability "$capability" || return 1

    _daia_capability_registry_link_exists "$capability" "$plugin_id"
}

daia_capability_get_providers() {
    local capability="${1-}"
    local link_key
    local registered_capability
    local plugin_id
    local -a providers=()

    _daia_capability_registry_validate_argument_count \
        1 "daia_capability_get_providers" "$#" || return 1
    _daia_capability_registry_require_initialized || return 1
    _daia_capability_registry_validate_capability "$capability" || return 1

    if ! daia_capability_exists "$capability"; then
        return 1
    fi

    for link_key in "${!__DAIA_CAPABILITY_REGISTRY_LINKS[@]}"; do
        registered_capability="${link_key%%"$__DAIA_CAPABILITY_REGISTRY_KEY_SEPARATOR"*}"

        if [[ "$registered_capability" != "$capability" ]]; then
            continue
        fi

        plugin_id="${link_key#*"$__DAIA_CAPABILITY_REGISTRY_KEY_SEPARATOR"}"
        providers+=("$plugin_id")
    done

    _daia_capability_registry_emit_sorted "${providers[@]}"
}

daia_capability_provider_count() {
    local capability="${1-}"
    local link_key
    local registered_capability
    local count=0

    _daia_capability_registry_validate_argument_count \
        1 "daia_capability_provider_count" "$#" || return 1
    _daia_capability_registry_require_initialized || return 1
    _daia_capability_registry_validate_capability "$capability" || return 1

    if ! daia_capability_exists "$capability"; then
        return 1
    fi

    for link_key in "${!__DAIA_CAPABILITY_REGISTRY_LINKS[@]}"; do
        registered_capability="${link_key%%"$__DAIA_CAPABILITY_REGISTRY_KEY_SEPARATOR"*}"

        if [[ "$registered_capability" == "$capability" ]]; then
            count=$((count + 1))
        fi
    done

    printf '%d\n' "$count"
}

daia_plugin_get_capabilities() {
    local plugin_id="${1-}"
    local link_key
    local capability
    local registered_plugin_id
    local -a capabilities=()

    _daia_capability_registry_validate_argument_count \
        1 "daia_plugin_get_capabilities" "$#" || return 1
    _daia_capability_registry_require_initialized || return 1
    _daia_capability_registry_validate_plugin_id "$plugin_id" || return 1

    if ! daia_plugin_exists "$plugin_id"; then
        return 1
    fi

    for link_key in "${!__DAIA_CAPABILITY_REGISTRY_LINKS[@]}"; do
        registered_plugin_id="${link_key#*"$__DAIA_CAPABILITY_REGISTRY_KEY_SEPARATOR"}"

        if [[ "$registered_plugin_id" != "$plugin_id" ]]; then
            continue
        fi

        capability="${link_key%%"$__DAIA_CAPABILITY_REGISTRY_KEY_SEPARATOR"*}"
        capabilities+=("$capability")
    done

    _daia_capability_registry_emit_sorted "${capabilities[@]}"
}

daia_plugin_capability_count() {
    local plugin_id="${1-}"
    local link_key
    local registered_plugin_id
    local count=0

    _daia_capability_registry_validate_argument_count \
        1 "daia_plugin_capability_count" "$#" || return 1
    _daia_capability_registry_require_initialized || return 1
    _daia_capability_registry_validate_plugin_id "$plugin_id" || return 1

    if ! daia_plugin_exists "$plugin_id"; then
        return 1
    fi

    for link_key in "${!__DAIA_CAPABILITY_REGISTRY_LINKS[@]}"; do
        registered_plugin_id="${link_key#*"$__DAIA_CAPABILITY_REGISTRY_KEY_SEPARATOR"}"

        if [[ "$registered_plugin_id" == "$plugin_id" ]]; then
            count=$((count + 1))
        fi
    done

    printf '%d\n' "$count"
}

daia_capability_registry_get_capabilities() {
    local -a capabilities=()

    _daia_capability_registry_validate_argument_count \
        0 "daia_capability_registry_get_capabilities" "$#" || return 1
    _daia_capability_registry_require_initialized || return 1

    capabilities=("${!__DAIA_CAPABILITY_REGISTRY_CAPABILITIES[@]}")
    _daia_capability_registry_emit_sorted "${capabilities[@]}"
}

daia_capability_registry_get_plugins() {
    local -a plugins=()

    _daia_capability_registry_validate_argument_count \
        0 "daia_capability_registry_get_plugins" "$#" || return 1
    _daia_capability_registry_require_initialized || return 1

    plugins=("${!__DAIA_CAPABILITY_REGISTRY_PLUGINS[@]}")
    _daia_capability_registry_emit_sorted "${plugins[@]}"
}

daia_capability_registry_capability_count() {
    _daia_capability_registry_validate_argument_count \
        0 "daia_capability_registry_capability_count" "$#" || return 1
    _daia_capability_registry_require_initialized || return 1

    printf '%d\n' "${#__DAIA_CAPABILITY_REGISTRY_CAPABILITIES[@]}"
}

daia_capability_registry_plugin_count() {
    _daia_capability_registry_validate_argument_count \
        0 "daia_capability_registry_plugin_count" "$#" || return 1
    _daia_capability_registry_require_initialized || return 1

    printf '%d\n' "${#__DAIA_CAPABILITY_REGISTRY_PLUGINS[@]}"
}
