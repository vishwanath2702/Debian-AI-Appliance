#!/usr/bin/env bash

set -u

###############################################################################
# Profile Registry
###############################################################################

daia_profile_registry_exists() {
    [[ "$1" == "workstation" ]]
}

daia_profile_registry_capabilities() {
    case "$1" in
        workstation)
            printf '%s\n' \
                desktop.environment \
                package.manager
            ;;
        *)
            return 1
            ;;
    esac
}

###############################################################################
# Capability Registry
###############################################################################

daia_capability_exists() {
    case "$1" in
        desktop.environment|package.manager)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

daia_capability_provider_count() {
    case "$1" in
        desktop.environment|package.manager)
            printf '1\n'
            ;;
        *)
            return 1
            ;;
    esac
}

daia_capability_get_providers() {
    case "$1" in
        desktop.environment)
            printf '%s\n' desktop/xfce
            ;;
        package.manager)
            printf '%s\n' package-manager/apt
            ;;
        *)
            return 1
            ;;
    esac
}

###############################################################################
# Plugin Registry
###############################################################################

daia_registry_plugin_exists() {
    case "$1" in
        filesystem/base|package-manager/apt|desktop/xfce)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

daia_plugin_registry_plugin_dependencies() {
    case "$1" in
        filesystem/base)
            ;;
        package-manager/apt)
            printf '%s\n' filesystem/base
            ;;
        desktop/xfce)
            printf '%s\n' \
                filesystem/base \
                package-manager/apt
            ;;
        *)
            return 1
            ;;
    esac
}

daia_plugin_registry_plugin_conflicts() {
    case "$1" in
        filesystem/base|package-manager/apt|desktop/xfce)
            ;;
        *)
            return 1
            ;;
    esac
}
