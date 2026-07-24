#!/usr/bin/env bash

set -u

#
# Profile Registry
#

daia_profile_registry_exists() {
    [[ "$1" == "workstation" ]]
}

daia_profile_registry_capabilities() {
    cat <<EOF
unknown.capability
EOF
}

#
# Capability Registry
#

daia_capability_exists() {
    return 1
}

daia_capability_provider_count() {
    return 1
}

daia_capability_get_providers() {
    return 1
}

#
# Plugin Registry
#

daia_registry_plugin_exists() {
    return 1
}

daia_plugin_registry_plugin_dependencies() {
    return 1
}

daia_plugin_registry_plugin_conflicts() {
    return 1
}
