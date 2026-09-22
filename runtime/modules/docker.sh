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
    local docker_group_entry
    local socket_mode
    local socket_owner
    local socket_group

    if ! command -v dpkg-query >/dev/null 2>&1; then
        echo "Required command is unavailable: dpkg-query" >&2
        return 1
    fi

    if ! command -v systemctl >/dev/null 2>&1; then
        echo "Required command is unavailable: systemctl" >&2
        return 1
    fi

    if ! command -v docker >/dev/null 2>&1; then
        echo "Required command is unavailable: docker" >&2
        return 1
    fi

    if ! dpkg-query -W -f='${Status}' docker.io 2>/dev/null | grep -q '^install ok installed$'; then
        echo "Docker engine package is not installed: docker.io" >&2
        return 1
    fi

    if ! dpkg-query -W -f='${Status}' docker-cli 2>/dev/null | grep -q '^install ok installed$'; then
        echo "Docker CLI package is not installed: docker-cli" >&2
        return 1
    fi

    if ! systemctl is-active --quiet containerd.service; then
        echo "Docker dependency service is not active: containerd.service" >&2
        return 1
    fi

    if ! systemctl is-active --quiet docker.socket; then
        echo "Docker socket is not active: docker.socket" >&2
        return 1
    fi

    if ! systemctl is-active --quiet docker.service; then
        echo "Docker service is not active: docker.service" >&2
        return 1
    fi

    if [[ ! -S /run/docker.sock ]]; then
        echo "Docker socket is missing: /run/docker.sock" >&2
        return 1
    fi

    socket_mode="$(stat -c '%a' /run/docker.sock)" || {
        echo "Failed to inspect Docker socket permissions" >&2
        return 1
    }

    socket_owner="$(stat -c '%U' /run/docker.sock)" || {
        echo "Failed to inspect Docker socket owner" >&2
        return 1
    }

    socket_group="$(stat -c '%G' /run/docker.sock)" || {
        echo "Failed to inspect Docker socket group" >&2
        return 1
    }

    if [[ "$socket_mode" != "660" ]]; then
        echo "Docker socket has unexpected permissions: $socket_mode" >&2
        return 1
    fi

    if [[ "$socket_owner" != "root" || "$socket_group" != "docker" ]]; then
        echo "Docker socket has unexpected ownership: $socket_owner:$socket_group" >&2
        return 1
    fi

    if ! docker info >/dev/null 2>&1; then
        echo "Docker daemon did not respond successfully" >&2
        return 1
    fi

    docker_group_entry="$(getent group docker 2>/dev/null || true)"

    if [[ -z "$docker_group_entry" ]]; then
        echo "Docker service group does not exist: docker" >&2
        return 1
    fi

    return 0
}
