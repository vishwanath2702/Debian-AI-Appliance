#!/usr/bin/env bash

set -u

daia_profile_registry_exists() {
    [[ "$1" == "workstation" ]]
}

daia_profile_registry_capabilities() {
    printf 'desktop.environment\n'
}

daia_capability_exists() {
    [[ "$1" == "desktop.environment" ]]
}

daia_capability_provider_count() {
    printf '0\n'
}

daia_capability_get_providers() {
    return 0
}

daia_plugin_registry_exists() {
    return 1
}

daia_plugin_registry_dependencies() {
    return 1
}

daia_plugin_registry_conflicts() {
    return 1
}
