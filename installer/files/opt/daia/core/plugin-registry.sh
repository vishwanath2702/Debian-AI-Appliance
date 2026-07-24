#!/usr/bin/env bash
#
# DAIA Plugin Registry
#
# Discovers plugins, validates metadata, and stores plugin registrations.
#
# Public API:
#   daia_registry_init
#   daia_registry_discover <plugin-root>
#   daia_registry_plugin_exists <plugin-id>
#   daia_registry_plugin_path <plugin-id>
#   daia_registry_plugin_ids
#   daia_registry_plugin_metadata <plugin-id> [metadata-key]
#

if [[ -n "${__DAIA_PLUGIN_REGISTRY_MODULE_INITIALIZED:-}" ]]; then
    if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then
        return 0
    fi

    exit 0
fi

__DAIA_PLUGIN_REGISTRY_MODULE_INITIALIZED=1

readonly DAIA_SUPPORTED_PLUGIN_API_VERSION="1"
readonly -a __DAIA_PLUGIN_METADATA_KEYS=(
    id
    plugin_version
    api_version
    provider
    description
    provides
)

declare -gA __DAIA_REGISTRY_PLUGIN_PATHS=()
declare -gA __DAIA_REGISTRY_PLUGIN_METADATA=()
declare -gA __DAIA_REGISTRY_PLUGIN_METADATA_RAW=()

__DAIA_REGISTRY_INITIALIZED=0

###############################################################################
# Internal helpers
###############################################################################

_daia_registry_error() {
    printf 'DAIA plugin-registry error: %s\n' "$*" >&2
}

_daia_registry_require_initialized() {
    if [[ "$__DAIA_REGISTRY_INITIALIZED" -ne 1 ]]; then
        _daia_registry_error "registry has not been initialized"
        return 1
    fi
}


_daia_registry_metadata_composite_key() {
    local plugin_id="${1:-}"
    local metadata_key="${2:-}"

    printf '%s:%s\n' "$plugin_id" "$metadata_key"
}

_daia_registry_plugin_registered() {
    local plugin_id="${1:-}"

    [[ -n "$plugin_id" ]] &&
        [[ -n "${__DAIA_REGISTRY_PLUGIN_PATHS[$plugin_id]+defined}" ]]
}

_daia_registry_set_plugin_path() {
    local plugin_id="${1:-}"
    local plugin_path="${2:-}"

    __DAIA_REGISTRY_PLUGIN_PATHS["$plugin_id"]="$plugin_path"
}

_daia_registry_get_plugin_path() {
    local plugin_id="${1:-}"

    if ! _daia_registry_plugin_registered "$plugin_id"; then
        return 1
    fi

    printf '%s\n' "${__DAIA_REGISTRY_PLUGIN_PATHS[$plugin_id]}"
}

_daia_registry_set_metadata() {
    local plugin_id="${1:-}"
    local metadata_key="${2:-}"
    local metadata_value="${3-}"
    local composite_key

    composite_key="$(
        _daia_registry_metadata_composite_key "$plugin_id" "$metadata_key"
    )"

    __DAIA_REGISTRY_PLUGIN_METADATA["$composite_key"]="$metadata_value"
}

_daia_registry_has_metadata() {
    local plugin_id="${1:-}"
    local metadata_key="${2:-}"
    local composite_key

    composite_key="$(
        _daia_registry_metadata_composite_key "$plugin_id" "$metadata_key"
    )"

    [[ -n "${__DAIA_REGISTRY_PLUGIN_METADATA[$composite_key]+defined}" ]]
}

_daia_registry_get_metadata() {
    local plugin_id="${1:-}"
    local metadata_key="${2:-}"
    local composite_key

    if ! _daia_registry_has_metadata "$plugin_id" "$metadata_key"; then
        return 1
    fi

    composite_key="$(
        _daia_registry_metadata_composite_key "$plugin_id" "$metadata_key"
    )"

    printf '%s\n' "${__DAIA_REGISTRY_PLUGIN_METADATA[$composite_key]}"
}

_daia_registry_set_raw_metadata() {
    local plugin_id="${1:-}"
    local metadata="${2-}"

    __DAIA_REGISTRY_PLUGIN_METADATA_RAW["$plugin_id"]="$metadata"
}

_daia_registry_has_raw_metadata() {
    local plugin_id="${1:-}"

    [[ -n "${__DAIA_REGISTRY_PLUGIN_METADATA_RAW[$plugin_id]+defined}" ]]
}

_daia_registry_get_raw_metadata() {
    local plugin_id="${1:-}"

    if ! _daia_registry_has_raw_metadata "$plugin_id"; then
        return 1
    fi

    printf '%s\n' "${__DAIA_REGISTRY_PLUGIN_METADATA_RAW[$plugin_id]}"
}

_daia_registry_validate_metadata_key() {
    local key="${1:-}"
    local allowed_key

    if [[ -z "$key" ]]; then
        _daia_registry_error "metadata key must not be empty"
        return 1
    fi

    for allowed_key in "${__DAIA_PLUGIN_METADATA_KEYS[@]}"; do
        if [[ "$key" == "$allowed_key" ]]; then
            return 0
        fi
    done

    _daia_registry_error "unknown metadata key: $key"
    return 1
}

_daia_registry_validate_plugin_id() {
    local plugin_id="${1:-}"

    if [[ -z "$plugin_id" ]]; then
        _daia_registry_error "plugin ID must not be empty"
        return 1
    fi

    if [[ ! "$plugin_id" =~ ^[a-z0-9_-]+/[a-z0-9._-]+$ ]]; then
        _daia_registry_error "invalid plugin ID: $plugin_id"
        return 1
    fi
}

_daia_registry_validate_plugin_version() {
    local plugin_version="${1:-}"

    if [[ -z "$plugin_version" ]]; then
        _daia_registry_error "plugin version must not be empty"
        return 1
    fi

    if [[ ! "$plugin_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        _daia_registry_error "invalid plugin version: $plugin_version"
        return 1
    fi
}

_daia_registry_validate_api_version() {
    local api_version="${1:-}"

    if [[ -z "$api_version" ]]; then
        _daia_registry_error "Plugin API version must not be empty"
        return 1
    fi

    if [[ ! "$api_version" =~ ^[1-9][0-9]*$ ]]; then
        _daia_registry_error "invalid Plugin API version: $api_version"
        return 1
    fi
}

_daia_registry_validate_provider() {
    local provider="${1:-}"

    if [[ -z "$provider" ]]; then
        _daia_registry_error "plugin provider must not be empty"
        return 1
    fi

    if [[ ! "$provider" =~ ^[a-z0-9_-]+$ ]]; then
        _daia_registry_error "invalid plugin provider: $provider"
        return 1
    fi
}

_daia_registry_validate_description() {
    local description="${1:-}"

    if [[ -z "${description//[[:space:]]/}" ]]; then
        _daia_registry_error "plugin description must not be empty"
        return 1
    fi
}

_daia_registry_validate_provides() {
    local capability="${1:-}"

    if [[ -z "$capability" ]]; then
        _daia_registry_error "provided capability must not be empty"
        return 1
    fi

    if [[ ! "$capability" =~ ^[a-z0-9._-]+$ ]]; then
        _daia_registry_error "invalid provided capability: $capability"
        return 1
    fi
}

_daia_registry_validate_metadata() {
    local key="${1:-}"
    local value="${2-}"

    case "$key" in
        id)
            _daia_registry_validate_plugin_id "$value"
            ;;
        plugin_version)
            _daia_registry_validate_plugin_version "$value"
            ;;
        api_version)
            _daia_registry_validate_api_version "$value"
            ;;
        provider)
            _daia_registry_validate_provider "$value"
            ;;
        description)
            _daia_registry_validate_description "$value"
            ;;
        provides)
            _daia_registry_validate_provides "$value"
            ;;
        *)
            _daia_registry_error "cannot validate unknown metadata key: $key"
            return 1
            ;;
    esac
}

_daia_registry_check_plugin_file() {
    local plugin_file="${1:-}"
    local mode

    if [[ ! -e "$plugin_file" ]]; then
        _daia_registry_error "plugin file does not exist: $plugin_file"
        return 1
    fi

    if [[ -L "$plugin_file" ]]; then
        _daia_registry_error \
            "symbolic-link plugin files are not allowed: $plugin_file"
        return 1
    fi

    if [[ ! -f "$plugin_file" ]]; then
        _daia_registry_error "plugin path is not a regular file: $plugin_file"
        return 1
    fi

    if [[ ! -r "$plugin_file" ]]; then
        _daia_registry_error "plugin file is not readable: $plugin_file"
        return 1
    fi

    if ! bash -n "$plugin_file"; then
        _daia_registry_error "plugin contains invalid Bash syntax: $plugin_file"
        return 1
    fi

    if ! mode="$(stat -c '%a' -- "$plugin_file" 2>/dev/null)"; then
        _daia_registry_error "could not inspect plugin permissions: $plugin_file"
        return 1
    fi

    # Group-writable and world-writable plugins are unsafe to load.
    if (( (8#$mode & 0022) != 0 )); then
        _daia_registry_error \
            "plugin file is writable by group or others: $plugin_file"
        return 1
    fi
}

_daia_registry_read_metadata() {
    local plugin_file="${1:-}"

    bash --noprofile --norc -c '
        plugin_file="$1"

        # shellcheck disable=SC1090
        source "$plugin_file" || exit 1

        if ! declare -F daia_plugin_metadata >/dev/null; then
            printf \
                "required function daia_plugin_metadata is missing\n" \
                >&2
            exit 2
        fi

        daia_plugin_metadata
    ' _ "$plugin_file"
}

_daia_registry_register_plugin() {
    local plugin_file="${1:-}"
    local metadata_output
    local metadata_status
    local line
    local key
    local value
    local plugin_id
    local api_version
    local required_key

    declare -A parsed_metadata=()

    _daia_registry_check_plugin_file "$plugin_file" || return 1

    metadata_output="$(_daia_registry_read_metadata "$plugin_file")"
    metadata_status=$?

    if [[ "$metadata_status" -ne 0 ]]; then
        _daia_registry_error "failed to read plugin metadata: $plugin_file"
        return 1
    fi

    while IFS= read -r line || [[ -n "$line" ]]; do
        [[ -z "$line" ]] && continue

        if [[ "$line" != *=* ]]; then
            _daia_registry_error \
                "malformed metadata record in $plugin_file: $line"
            return 1
        fi

        key="${line%%=*}"
        value="${line#*=}"

        _daia_registry_validate_metadata_key "$key" || return 1
        _daia_registry_validate_metadata "$key" "$value" || return 1

        if [[ -n "${parsed_metadata[$key]+defined}" ]]; then
            _daia_registry_error \
                "duplicate metadata key in $plugin_file: $key"
            return 1
        fi

        parsed_metadata["$key"]="$value"
    done <<< "$metadata_output"

    for required_key in "${__DAIA_PLUGIN_METADATA_KEYS[@]}"; do
        if [[ -z "${parsed_metadata[$required_key]+defined}" ]]; then
            _daia_registry_error \
                "required metadata field is missing in $plugin_file: $required_key"
            return 1
        fi
    done

    plugin_id="${parsed_metadata[id]}"
    api_version="${parsed_metadata[api_version]}"

    if [[ "$api_version" != "$DAIA_SUPPORTED_PLUGIN_API_VERSION" ]]; then
        _daia_registry_error \
            "unsupported Plugin API version '$api_version' for $plugin_id"
        return 1
    fi

    if _daia_registry_plugin_registered "$plugin_id"; then
        _daia_registry_error "duplicate plugin ID: $plugin_id"
        return 1
    fi

    _daia_registry_set_plugin_path "$plugin_id" "$plugin_file"
    _daia_registry_set_raw_metadata "$plugin_id" "$metadata_output"

    for key in "${!parsed_metadata[@]}"; do
        _daia_registry_set_metadata             "$plugin_id"             "$key"             "${parsed_metadata[$key]}"
    done
}

###############################################################################
# Public API
###############################################################################

daia_registry_init() {
    __DAIA_REGISTRY_PLUGIN_PATHS=()
    __DAIA_REGISTRY_PLUGIN_METADATA=()
    __DAIA_REGISTRY_PLUGIN_METADATA_RAW=()
    __DAIA_REGISTRY_INITIALIZED=1
}

daia_registry_discover() {
    local plugin_root="${1:-}"
    local canonical_root
    local plugin_file
    local canonical_plugin

    _daia_registry_require_initialized || return 1

    if [[ -z "$plugin_root" ]]; then
        _daia_registry_error "plugin root was not provided"
        return 2
    fi

    if [[ ! -d "$plugin_root" ]]; then
        _daia_registry_error "plugin root is not a directory: $plugin_root"
        return 1
    fi

    if [[ ! -r "$plugin_root" ]]; then
        _daia_registry_error "plugin root is not readable: $plugin_root"
        return 1
    fi

    canonical_root="$(realpath -e -- "$plugin_root")" || {
        _daia_registry_error "could not resolve plugin root: $plugin_root"
        return 1
    }

    while IFS= read -r -d '' plugin_file; do
        canonical_plugin="$(realpath -e -- "$plugin_file")" || {
            _daia_registry_error "could not resolve plugin path: $plugin_file"
            return 1
        }

        case "$canonical_plugin" in
            "$canonical_root"/*)
                ;;
            *)
                _daia_registry_error "plugin escapes approved root: $plugin_file"
                return 1
                ;;
        esac

        _daia_registry_register_plugin "$canonical_plugin" || return 1
    done < <(
        find "$canonical_root" \
            -type f \
            -name 'plugin.sh' \
            -print0 |
            sort -z
    )
}

daia_registry_plugin_exists() {
    local plugin_id="${1:-}"

    [[ "$__DAIA_REGISTRY_INITIALIZED" -eq 1 ]] &&
        _daia_registry_plugin_registered "$plugin_id"
}

daia_registry_plugin_path() {
    local plugin_id="${1:-}"

    _daia_registry_require_initialized || return 1

    if ! _daia_registry_plugin_registered "$plugin_id"; then
        _daia_registry_error "plugin is not registered: $plugin_id"
        return 1
    fi

    _daia_registry_get_plugin_path "$plugin_id"
}

daia_registry_plugin_ids() {
    _daia_registry_require_initialized || return 1

    if [[ "${#__DAIA_REGISTRY_PLUGIN_PATHS[@]}" -eq 0 ]]; then
        return 0
    fi

    printf '%s\n' "${!__DAIA_REGISTRY_PLUGIN_PATHS[@]}" |
        LC_ALL=C sort
}

daia_registry_plugin_metadata() {
    local plugin_id="${1:-}"
    local metadata_key="${2:-}"

    _daia_registry_require_initialized || return 1

    if ! _daia_registry_plugin_registered "$plugin_id"; then
        _daia_registry_error "plugin is not registered: $plugin_id"
        return 1
    fi

    if [[ -z "$metadata_key" ]]; then
        _daia_registry_get_raw_metadata "$plugin_id"
        return
    fi

    _daia_registry_validate_metadata_key "$metadata_key" || return 2
    _daia_registry_get_metadata "$plugin_id" "$metadata_key"
}
