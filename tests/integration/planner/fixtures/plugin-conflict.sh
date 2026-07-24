#!/usr/bin/env bash

set -u

daia_profile_registry_exists() {
    [[ "$1" == "workstation" ]]
}

daia_profile_registry_capabilities() {
    [[ "$1" == "workstation" ]] || return 1

    printf '%s\n' \
        'desktop.environment' \
        'legacy.display'
}

daia_capability_exists() {
    case "$1" in
        desktop.environment | window.manager | graphics.stack | legacy.display)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

daia_capability_get_providers() {
    case "$1" in
        desktop.environment)
            printf '%s\n' 'desktop/environment'
            ;;
        window.manager)
            printf '%s\n' 'desktop/window-manager'
            ;;
        graphics.stack)
            printf '%s\n' 'system/graphics-stack'
            ;;
        legacy.display)
            printf '%s\n' 'legacy/display'
            ;;
        *)
            return 1
            ;;
    esac
}

daia_capability_provider_count() {
    case "$1" in
        desktop.environment | window.manager | graphics.stack | legacy.display)
            printf '%s\n' '1'
            ;;
        *)
            printf '%s\n' '0'
            ;;
    esac
}

daia_registry_plugin_ids() {
    printf '%s\n' \
        'desktop/environment' \
        'desktop/window-manager' \
        'system/graphics-stack' \
        'legacy/display'
}

daia_registry_plugin_exists() {
    case "$1" in
        desktop/environment | desktop/window-manager | system/graphics-stack | legacy/display)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

daia_registry_plugin_metadata() {
    local plugin_id="$1"
    local field="$2"

    case "$plugin_id:$field" in
        desktop/environment:provides)
            printf '%s\n' 'desktop.environment'
            ;;
        desktop/window-manager:provides)
            printf '%s\n' 'window.manager'
            ;;
        system/graphics-stack:provides)
            printf '%s\n' 'graphics.stack'
            ;;
        legacy/display:provides)
            printf '%s\n' 'legacy.display'
            ;;
        *)
            return 1
            ;;
    esac
}

daia_plugin_registry_plugin_dependencies() {
    case "$1" in
        desktop/environment)
            printf '%s\n' 'desktop/window-manager'
            ;;
        desktop/window-manager)
            printf '%s\n' 'system/graphics-stack'
            ;;
        system/graphics-stack | legacy/display)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

daia_plugin_registry_plugin_conflicts() {
    case "$1" in
        desktop/environment)
            printf '%s\n' 'legacy/display'
            ;;
        legacy/display)
            printf '%s\n' 'desktop/environment'
            ;;
        desktop/window-manager | system/graphics-stack)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}
