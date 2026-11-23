#!/usr/bin/env bash
# SearXNG offline runtime foundation.
# Container provisioning and startup require separate authorization.

searxng_validate() {
    local payload_root="${DAIA_PAYLOAD_ROOT:-/opt/daia/payload}"
    local archive="${payload_root}/images/searxng.tar"
    local actual_sha

    [[ "${DAIA_ENABLE_SEARXNG:-false}" == "true" ]] || return 1

    [[ "${DAIA_SEARXNG_DOCKER_IMAGE_ID:-}" == "sha256:be131d45cb5daf22a1f47c530b6d11063c76e53bf0efa99d047b07e4b3d4274d" ]] ||
        return 1

    [[ "${DAIA_SEARXNG_ARCHIVE_SHA256:-}" == "fbd19b12819ae924f5b4bc52cc51e1e05fed244be29f17dfc6cb81fae2139c1b" ]] ||
        return 1

    [[ -f "$archive" && ! -L "$archive" ]] || return 1

    actual_sha="$(sha256sum -- "$archive")" || return 1
    actual_sha="${actual_sha%% *}"

    [[ "$actual_sha" == "fbd19b12819ae924f5b4bc52cc51e1e05fed244be29f17dfc6cb81fae2139c1b" ]]
}

searxng_load_verified_offline_image() {
    local expected_id="sha256:be131d45cb5daf22a1f47c530b6d11063c76e53bf0efa99d047b07e4b3d4274d"
    local image_ref="searxng/searxng:pinned"
    local payload_root="${DAIA_PAYLOAD_ROOT:-/opt/daia/payload}"
    local archive="${payload_root}/images/searxng.tar"
    local loaded_id

    searxng_validate || return 1

    command -v docker >/dev/null 2>&1 || return 1
    docker info >/dev/null 2>&1 || return 1

    docker load --input "$archive" >/dev/null || return 1

    loaded_id="$(docker image inspect --format '{{.Id}}' "$image_ref")" ||
        return 1

    [[ "$loaded_id" == "$expected_id" ]]
}

searxng_verify_network_contract() {
    local internal="daia-rag-internal"
    local egress="daia-searxng-egress"
    local internal_meta egress_meta

    command -v docker >/dev/null 2>&1 || return 1
    docker info >/dev/null 2>&1 || return 1

    internal_meta="$(docker network inspect \
        --format '{{.Name}}|{{.Driver}}|{{.Internal}}|{{index .Labels "org.daia.component"}}|{{index .Labels "org.daia.managed"}}' \
        "$internal")" || return 1

    [[ "$internal_meta" == "daia-rag-internal|bridge|true|rag|true" ]] ||
        return 1

    egress_meta="$(docker network inspect \
        --format '{{.Name}}|{{.Driver}}|{{.Internal}}|{{index .Labels "org.daia.component"}}|{{index .Labels "org.daia.managed"}}' \
        "$egress")" || return 1

    [[ "$egress_meta" == "daia-searxng-egress|bridge|false|searxng|true" ]] ||
        return 1
}

# Start only a trusted, registered SearXNG container.
# This is not wired into recovery until readiness gating is implemented.
searxng_start_registered() {
    local container_id="${1:-}"
    local helper="/usr/libexec/daia-searxng-verify-registration"
    local verified_id="" state="" running_id=""

    [[ "$EUID" -eq 0 ]] || return 1
    [[ "$container_id" =~ ^[0-9a-f]{64}$ ]] || return 1
    [[ -f "$helper" && ! -L "$helper" && -x "$helper" ]] || return 1

    command -v docker >/dev/null 2>&1 || return 1
    docker info >/dev/null 2>&1 || return 1

    verified_id="$("$helper" --resolve-registered)" || return 1
    [[ "$verified_id" == "$container_id" ]] || return 1

    searxng_verify_network_contract || return 1

    state="$(docker container inspect --format \
        '{{.State.Running}}|{{.State.Paused}}|{{.State.Restarting}}|{{.State.Status}}' \
        "$container_id")" || return 1

    case "$state" in
        false\|false\|false\|created|false\|false\|false\|exited)
            ;;
        *)
            echo "SearXNG startup refused: invalid container state" >&2
            return 1
            ;;
    esac

    # Reauthorize immediately before mutation.
    verified_id="$("$helper" --resolve-registered)" || return 1
    [[ "$verified_id" == "$container_id" ]] || return 1

    docker start "$container_id" >/dev/null || return 1

    # Verify the running identity and network contract.
    # Only the container successfully started above may be stopped.
    if ! running_id="$("$helper" --resolve-registered)"; then
        if ! docker stop --time 10 "$container_id" >/dev/null; then
            echo "SearXNG SECURITY WARNING: failed to stop container after identity verification failure: $container_id" >&2
        fi
        return 1
    fi

    if [[ "$running_id" != "$container_id" ]]; then
        if ! docker stop --time 10 "$container_id" >/dev/null; then
            echo "SearXNG SECURITY WARNING: failed to stop container after identity verification failure: $container_id" >&2
        fi
        return 1
    fi
}

# Bounded HTTP readiness probe, not yet wired into recovery.
# The pinned image's HTTP endpoint remains to be verified.
searxng_wait_ready() {
    local container_id="${1:-}"
    local helper="/usr/libexec/daia-searxng-verify-registration"
    local timeout_seconds="${2:-30}"
    local started_at now remaining verified_id
    local probe_seconds helper_seconds uptime_seconds

    [[ "$EUID" -eq 0 ]] || return 1
    [[ "$container_id" =~ ^[0-9a-f]{64}$ ]] || return 1
    [[ "$timeout_seconds" =~ ^[1-9][0-9]?$ ]] || return 1
    (( timeout_seconds <= 60 )) || return 1
    [[ -f "$helper" && ! -L "$helper" && -x "$helper" ]] || return 1

    command -v docker >/dev/null 2>&1 || return 1
    command -v timeout >/dev/null 2>&1 || return 1
    [[ -r /proc/uptime ]] || return 1

    read -r uptime_seconds _ < /proc/uptime || return 1
    [[ "$uptime_seconds" =~ ^[0-9]+\.[0-9]+$ ]] || return 1
    started_at="${uptime_seconds%%.*}"

    while true; do
        read -r uptime_seconds _ < /proc/uptime || return 1
        [[ "$uptime_seconds" =~ ^[0-9]+\.[0-9]+$ ]] || return 1
        now="${uptime_seconds%%.*}"
        remaining=$(( timeout_seconds - (now - started_at) ))

        if (( remaining <= 0 )); then
            echo "SearXNG readiness deadline exceeded" >&2
            return 1
        fi

        helper_seconds=2
        (( remaining < helper_seconds )) && helper_seconds="$remaining"

        verified_id="$(timeout --signal=TERM --kill-after=1 \
            "${helper_seconds}s" "$helper" --resolve-registered)" || return 1

        [[ "$verified_id" == "$container_id" ]] || return 1

        read -r uptime_seconds _ < /proc/uptime || return 1
        [[ "$uptime_seconds" =~ ^[0-9]+\.[0-9]+$ ]] || return 1
        now="${uptime_seconds%%.*}"
        remaining=$(( timeout_seconds - (now - started_at) ))
        (( remaining > 0 )) || return 1

        probe_seconds=3
        (( remaining < probe_seconds )) && probe_seconds="$remaining"

        if timeout --signal=TERM --kill-after=1 \
            "${probe_seconds}s" \
            docker exec "$container_id" python3 -c \
            'import urllib.request; r=urllib.request.urlopen("http://127.0.0.1:8080/healthz", timeout=2); assert r.status == 200 and r.read(3) == b"OK"' \
            >/dev/null 2>&1; then

            read -r uptime_seconds _ < /proc/uptime || return 1
            [[ "$uptime_seconds" =~ ^[0-9]+\.[0-9]+$ ]] || return 1
            now="${uptime_seconds%%.*}"
            remaining=$(( timeout_seconds - (now - started_at) ))
            (( remaining > 0 )) || return 1

            helper_seconds=2
            (( remaining < helper_seconds )) && helper_seconds="$remaining"

            verified_id="$(timeout --signal=TERM --kill-after=1 \
                "${helper_seconds}s" "$helper" --resolve-registered)" || return 1

            [[ "$verified_id" == "$container_id" ]] || return 1

            read -r uptime_seconds _ < /proc/uptime || return 1
            [[ "$uptime_seconds" =~ ^[0-9]+\.[0-9]+$ ]] || return 1
            now="${uptime_seconds%%.*}"
            (( now - started_at < timeout_seconds )) || return 1

            return 0
        fi

        read -r uptime_seconds _ < /proc/uptime || return 1
        [[ "$uptime_seconds" =~ ^[0-9]+\.[0-9]+$ ]] || return 1
        now="${uptime_seconds%%.*}"
        remaining=$(( timeout_seconds - (now - started_at) ))

        if (( remaining <= 0 )); then
            echo "SearXNG readiness deadline exceeded" >&2
            return 1
        fi

        sleep 1
    done
}

# Still deferred:
# - network/container provisioning and registration
# - recovery orchestration and readiness integration

# Create-only first-boot SearXNG orchestration.
# Deliberately not wired into runtime.sh.
# The internal RAG network must already be trusted.
searxng_firstboot_provision_registered() {
    [[ "$EUID" -eq 0 ]] || return 1
    [[ "${DAIA_ENABLE_SEARXNG:-false}" == "true" ]] || return 1

    declare -F searxng_validate >/dev/null || return 1
    declare -F searxng_load_verified_offline_image >/dev/null || return 1
    declare -F searxng_provision_preflight >/dev/null || return 1
    declare -F searxng_provision_egress_network >/dev/null || return 1
    declare -F searxng_provision_container >/dev/null || return 1

    searxng_validate || return 1
    searxng_load_verified_offline_image || return 1
    searxng_provision_preflight || return 1
    searxng_provision_egress_network || return 1
    searxng_provision_container || return 1

    return 0
}
