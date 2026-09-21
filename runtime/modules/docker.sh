#!/usr/bin/env bash

docker_validate() {
    local payload_root="${DAIA_PAYLOAD_ROOT:-/opt/daia/payload}"
    local docker_package_directory="${payload_root}/packages/docker"

    if [[ ! -d "$docker_package_directory" ]]; then
        echo "Docker package directory is missing: $docker_package_directory" >&2
        return 1
    fi

    if ! compgen -G "${docker_package_directory}/*.deb" >/dev/null; then
        echo "Docker package payload is empty: $docker_package_directory" >&2
        return 1
    fi

    return 0
}

docker_install() {
    return 0
}

docker_verify() {
    return 0
}
