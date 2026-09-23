#!/usr/bin/env bash

# DAIA Open WebUI runtime module.
#
# This module is sourced by the DAIA runtime. It is not intended to be
# executed directly.

openwebui_validate() {
    local payload_root="${DAIA_PAYLOAD_ROOT:-/opt/daia/payload}"
    local image_file="${payload_root}/images/open-webui.tar"

    if [[ ! -s "$image_file" ]]; then
        echo "Open WebUI image is missing or empty: $image_file" >&2
        return 1
    fi

    if ! command -v docker >/dev/null 2>&1; then
        echo "Required command is unavailable: docker" >&2
        return 1
    fi

    if ! docker info >/dev/null 2>&1; then
        echo "Docker daemon is not available" >&2
        return 1
    fi

    return 0
}

openwebui_install() {
    local payload_root="${DAIA_PAYLOAD_ROOT:-/opt/daia/payload}"
    local image_file="${payload_root}/images/open-webui.tar"
    local container_name="${DAIA_OPEN_WEBUI_CONTAINER:-open-webui}"
    local volume_name="${DAIA_OPEN_WEBUI_VOLUME:-open-webui}"
    local ollama_url="${DAIA_OPEN_WEBUI_OLLAMA_URL:-http://host.docker.internal:11434}"

    openwebui_validate || return 1

    local image_load_output
    local image_reference

    if ! image_load_output="$(docker load --input "$image_file")"
    then
        echo "Failed to load Open WebUI image: $image_file" >&2
        return 1
    fi

    image_reference="$(printf '%s\n' "$image_load_output" | sed -n 's/^Loaded image: //p' | tail -n 1)"

    if [[ -z "$image_reference" ]]
    then
        echo "Unable to determine loaded Open WebUI image reference" >&2
        return 1
    fi

    if docker container inspect "$container_name" >/dev/null 2>&1
    then
        if ! docker rm -f "$container_name" >/dev/null; then
            echo "Failed to remove existing Open WebUI container: $container_name" >&2
            return 1
        fi
    fi

    if ! docker volume inspect "$volume_name" >/dev/null 2>&1
    then
        if ! docker volume create "$volume_name" >/dev/null; then
            echo "Failed to create Open WebUI data volume: $volume_name" >&2
            return 1
        fi
    fi

    if ! docker run \
        --detach \
        --publish 3000:8080 \
        --add-host=host.docker.internal:host-gateway \
        --volume "${volume_name}:/app/backend/data" \
        --env "OLLAMA_BASE_URL=${ollama_url}" \
        --name "$container_name" \
        --restart always \
        "$image_reference"
    then
        echo "Failed to start Open WebUI container: $container_name" >&2
        return 1
    fi

    return 0
}

openwebui_verify() {
    local container_name="${DAIA_OPEN_WEBUI_CONTAINER:-open-webui}"

    if ! command -v docker >/dev/null 2>&1; then
        echo "Required command is unavailable: docker" >&2
        return 1
    fi

    if ! docker container inspect "$container_name" >/dev/null 2>&1; then
        echo "Open WebUI container does not exist: $container_name" >&2
        return 1
    fi

    if [[ "$(docker inspect -f '{{.State.Status}}' "$container_name")" != "running" ]]
    then
        echo "Open WebUI container is not running: $container_name" >&2
        return 1
    fi

    if ! docker inspect \
        -f '{{range .Config.Env}}{{println .}}{{end}}' \
        "$container_name" |
        grep -Fxq "OLLAMA_BASE_URL=${DAIA_OPEN_WEBUI_OLLAMA_URL:-http://host.docker.internal:11434}"
    then
        echo "Open WebUI Ollama connection is not configured correctly" >&2
        return 1
    fi

    if ! docker inspect \
        -f '{{range .Mounts}}{{println .Name ":" .Destination}}' \
        "$container_name" |
        grep -Fxq "${DAIA_OPEN_WEBUI_VOLUME:-open-webui}:/app/backend/data"
    then
        echo "Open WebUI data volume is not mounted correctly" >&2
        return 1
    fi

    return 0
}
