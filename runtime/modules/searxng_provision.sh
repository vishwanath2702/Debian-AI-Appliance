#!/usr/bin/env bash
# Guarded create-only SearXNG provisioning; runtime activation is separate.

_searxng_resource_absent() {
    local kind="${1:-}" name="${2:-}" inventory

    case "$kind" in
        container)
            inventory="$(docker container ls -a --format '{{.Names}}')" ||
                return 2
            ;;
        network)
            inventory="$(docker network ls --format '{{.Name}}')" ||
                return 2
            ;;
        *) return 2 ;;
    esac

    if printf '%s\n' "$inventory" | grep -Fxq -- "$name"; then
        return 1
    fi
    return 0
}

searxng_provision_preflight() {
    local expected_image="sha256:be131d45cb5daf22a1f47c530b6d11063c76e53bf0efa99d047b07e4b3d4274d"
    local image_id internal status=0

    [[ "$EUID" -eq 0 ]] || return 1
    [[ "${DAIA_ENABLE_SEARXNG:-false}" == "true" ]] || return 1
    [[ "${DAIA_SEARXNG_DOCKER_IMAGE_ID:-}" == "$expected_image" ]] ||
        return 1

    command -v docker >/dev/null 2>&1 || return 1
    docker info >/dev/null 2>&1 || return 1

    image_id="$(docker image inspect --format '{{.Id}}' \
        "$expected_image")" || return 1
    [[ "$image_id" == "$expected_image" ]] || return 1

    internal="$(docker network inspect --format \
        '{{.Name}}|{{.Driver}}|{{.Internal}}|{{index .Labels "org.daia.component"}}|{{index .Labels "org.daia.managed"}}' \
        daia-rag-internal)" || return 1

    [[ "$internal" == "daia-rag-internal|bridge|true|rag|true" ]] ||
        return 1

    _searxng_resource_absent container daia-searxng || status=$?

    case "$status" in
        0) return 0 ;;
        1)
            echo "Existing SearXNG container requires separate authorization" >&2
            return 1
            ;;
        *)
            echo "Cannot safely inspect Docker container inventory" >&2
            return 1
            ;;
    esac
}

searxng_provision_container() {
    local expected_image="sha256:be131d45cb5daf22a1f47c530b6d11063c76e53bf0efa99d047b07e4b3d4274d"
    local container_name="daia-searxng"
    local egress="daia-searxng-egress"
    local container_id inspected_id metadata members verified_id

    searxng_provision_preflight || return 1

    # Never adopt an unverified egress network.
    metadata="$(docker network inspect --format \
        '{{.Name}}|{{.Driver}}|{{.Internal}}|{{index .Labels "org.daia.component"}}|{{index .Labels "org.daia.managed"}}' \
        "$egress")" || return 1

    if [[ "$metadata" != "daia-searxng-egress|bridge|false|searxng|true" ]]; then
        echo "Untrusted SearXNG egress network" >&2
        return 1
    fi

    members="$(docker network inspect --format \
        '{{range $id, $container := .Containers}}{{$id}}{{"\n"}}{{end}}' \
        "$egress")" || return 1

    if [[ -n "$members" ]]; then
        echo "SearXNG egress network is not empty" >&2
        return 1
    fi

    # Both trusted helpers must exist before container creation.
    [[ -x /usr/libexec/daia-searxng-verify-container ]] || return 1
    [[ -x /usr/libexec/daia-searxng-publish-registration ]] || return 1

    # Create only. No published ports, mounts, privileges,
    # automatic restart, or startup.
    container_id="$(docker container create \
        --name "$container_name" \
        --label org.daia.component=searxng \
        --label org.daia.managed=true \
        --restart no \
        --security-opt no-new-privileges:true \
        --network daia-rag-internal \
        "$expected_image")" || return 1

    if [[ ! "$container_id" =~ ^[0-9a-f]{64}$ ]]; then
        echo "Invalid created SearXNG container identity" >&2
        return 1
    fi

    inspected_id="$(docker container inspect \
        --format '{{.Id}}' "$container_name")" || return 1

    if [[ "$inspected_id" != "$container_id" ]]; then
        echo "SearXNG container identity changed" >&2
        return 1
    fi

    docker network connect "$egress" "$container_id" || return 1

    # Independent verification precedes registration.
    verified_id="$(/usr/libexec/daia-searxng-verify-container \
        "$container_id")" || return 1

    if [[ "$verified_id" != "$container_id" ]]; then
        echo "SearXNG independent verification failed" >&2
        return 1
    fi

    # Publisher performs its own independent verification.
    /usr/libexec/daia-searxng-publish-registration \
        "$container_id" || return 1

    # Startup is separately authorized.
    return 0
}

# The internal RAG network must already have passed provisioning preflight.
# No existing egress network is adopted automatically.
searxng_provision_egress_network() {
    local network="daia-searxng-egress"
    local status=0
    local metadata members

    searxng_provision_preflight || return 1

    _searxng_resource_absent network "$network" || status=$?

    case "$status" in
        0) ;;
        1)
            echo "Existing SearXNG egress network requires separate authorization" >&2
            return 1
            ;;
        *)
            echo "Cannot safely inventory SearXNG egress networks" >&2
            return 1
            ;;
    esac

    # This is the only Docker mutation in this provisioning stage.
    # Docker's default bridge network is external, not --internal.
    if ! docker network create \
        --driver bridge \
        --label org.daia.component=searxng \
        --label org.daia.managed=true \
        "$network" >/dev/null; then
        echo "Unable to create SearXNG egress network" >&2
        return 1
    fi

    metadata="$(docker network inspect --format \
        '{{.Name}}|{{.Driver}}|{{.Internal}}|{{index .Labels "org.daia.component"}}|{{index .Labels "org.daia.managed"}}' \
        "$network")" || return 1

    if [[ "$metadata" != "daia-searxng-egress|bridge|false|searxng|true" ]]; then
        echo "Created SearXNG egress network failed ownership verification" >&2
        return 1
    fi

    members="$(docker network inspect --format \
        '{{range $id, $container := .Containers}}{{$id}}{{"\\n"}}{{end}}' \
        "$network")" || return 1

    if [[ -n "$members" ]]; then
        echo "Created SearXNG egress network has unexpected members" >&2
        return 1
    fi

    return 0
}
