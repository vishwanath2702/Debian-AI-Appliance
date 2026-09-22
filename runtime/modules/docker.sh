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
    local payload_root="${DAIA_PAYLOAD_ROOT:-/opt/daia/payload}"
    local docker_package_directory="${payload_root}/packages/docker"
    local -a docker_packages

    docker_validate || return 1

    if ! command -v apt-get >/dev/null 2>&1; then
        echo "Required command is unavailable: apt-get" >&2
        return 1
    fi

    docker_packages=("${docker_package_directory}"/*.deb)

    if ! apt-get \
        --yes \
        --no-download \
        --no-install-recommends \
        install \
        "${docker_packages[@]}"
    then
        echo "Failed to install offline Docker packages" >&2
        return 1
    fi

    if ! command -v systemctl >/dev/null 2>&1; then
        echo "Required command is unavailable: systemctl" >&2
        return 1
    fi

    if ! systemctl enable containerd.service docker.socket docker.service; then
        echo "Failed to enable Docker runtime services" >&2
        return 1
    fi

    if ! systemctl start containerd.service docker.socket docker.service; then
        echo "Failed to start Docker runtime services" >&2
        return 1
    fi

    return 0
}

docker_verify() {
    return 0
}
