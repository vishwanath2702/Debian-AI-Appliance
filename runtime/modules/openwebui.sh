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

# Verify persistent storage ownership without modifying Docker state.
openwebui_volume_owned() {
    local volume_name="${1:-}"
    local labels

    if [[ "$volume_name" != "open-webui" ]]; then
        echo "Unexpected Open WebUI volume name" >&2
        return 1
    fi

    if ! labels="$(docker volume inspect --format \
        '{{index .Labels "org.daia.component"}}{{printf "\t"}}{{index .Labels "org.daia.managed"}}' \
        "$volume_name")"; then
        echo "Cannot inspect Open WebUI volume ownership" >&2
        return 1
    fi

    if [[ "$labels" != $'openwebui\ttrue' ]]; then
        echo "Open WebUI volume ownership mismatch" >&2
        return 1
    fi

    return 0
}


openwebui_ensure_volume() {
    local volume_name="${DAIA_OPEN_WEBUI_VOLUME:-open-webui}"
    if [[ "$volume_name" != "open-webui" ]]; then
        echo "Unexpected Open WebUI volume configuration" >&2
        return 1
    fi

    # Existing volumes must already carry DAIA ownership labels.
    # Never silently adopt or overwrite an unrelated volume.
    local volume_inventory
    if ! volume_inventory="$(docker volume ls --format '{{.Name}}')"; then
        echo "Cannot enumerate Docker volumes" >&2
        return 1
    fi

    if printf '%s\n' "$volume_inventory" | grep -Fxq "$volume_name"; then
        openwebui_volume_owned "$volume_name" || return 1
    else
        if ! docker volume create \
            --label org.daia.component=openwebui \
            --label org.daia.managed=true \
            "$volume_name" >/dev/null; then
            echo "Failed to create Open WebUI data volume" >&2
            return 1
        fi

        openwebui_volume_owned "$volume_name" || return 1
    fi

}



# Prepare database access before authorizing any Open WebUI container.
# Repeated firstboot attempts may reuse only independently verified state.
openwebui_prepare_rag() {
    local tika_id="${1:-}"
    local -a provision_args=()

    if (( $# > 1 )); then
        echo "Invalid RAG preparation arguments" >&2
        return 1
    fi

    if (( $# == 1 )); then
        openwebui_require_tika_id "$tika_id" || return 1
        provision_args=(--tika-id "$tika_id")
    fi

    if [[ "$EUID" -ne 0 ]]; then
        echo "Open WebUI RAG preparation requires root" >&2
        return 1
    fi

    local credential_helper="/usr/libexec/daia-openwebui-pgvector-credentials.py"
    local provision_helper="/usr/libexec/daia-openwebui-rag-provision.py"
    local helper

    for helper in "$credential_helper" "$provision_helper"; do
        if [[ ! -f "$helper" || -L "$helper" || ! -x "$helper" ]]; then
            echo "Required Open WebUI RAG helper is unavailable" >&2
            return 1
        fi
    done

    if ! "$credential_helper"; then
        echo "Open WebUI credential preparation failed" >&2
        return 1
    fi

    if ! "$provision_helper" "${provision_args[@]}"; then
        echo "Open WebUI database role verification failed" >&2
        return 1
    fi

    return 0
}


openwebui_resolve_registered_tika_id() {
    local helper="/usr/libexec/daia-tika-register-container"
    local registered_id

    if [[ "$EUID" -ne 0 ]]; then
        echo "Tika resolution requires root" >&2
        return 1
    fi

    if [[ ! -f "$helper" || -L "$helper" || ! -x "$helper" ]]; then
        echo "Trusted Tika resolver is unavailable" >&2
        return 1
    fi

    if ! registered_id="$("$helper" --resolve-registered)" ||
       [[ ! "$registered_id" =~ ^[0-9a-f]{64}$ ]]; then
        echo "Trusted Tika registration cannot be resolved" >&2
        return 1
    fi

    printf '%s\n' "$registered_id"
}

openwebui_require_tika_id() {
    local requested_id="${1:-}"
    local helper="/usr/libexec/daia-tika-register-container"
    local resolved_id

    if [[ "$EUID" -ne 0 ]]; then
        echo "Trusted Tika resolution requires root" >&2
        return 1
    fi

    if [[ ! "$requested_id" =~ ^[0-9a-f]{64}$ ]]; then
        echo "Invalid requested Tika identity" >&2
        return 1
    fi

    if [[ ! -f "$helper" || -L "$helper" || ! -x "$helper" ]]; then
        echo "Trusted Tika resolver is unavailable" >&2
        return 1
    fi

    if ! resolved_id="$("$helper" --resolve-registered)" ||
       [[ "$resolved_id" != "$requested_id" ]]; then
        echo "Tika registered identity verification failed" >&2
        return 1
    fi

    return 0
}

openwebui_start_registered() {
    local container_id="${1:-}"
    local helper="/usr/libexec/daia-openwebui-register-container"
    local authorized_id
    local verified_id
    local identity_id
    local tika_id="${2:-}"
    local -a authorization_args
    local -a verification_args

    if (( $# < 1 || $# > 2 )); then
        echo "Invalid Open WebUI startup arguments" >&2
        return 1
    fi

    if [[ "$EUID" -ne 0 ]]; then
        echo "Open WebUI startup requires root" >&2
        return 1
    fi

    if [[ ! "$container_id" =~ ^[0-9a-f]{64}$ ]]; then
        echo "Invalid Open WebUI startup identity" >&2
        return 1
    fi

    if [[ ! -f "$helper" || -L "$helper" || ! -x "$helper" ]]; then
        echo "Trusted Open WebUI startup helper unavailable" >&2
        return 1
    fi

    if ! command -v docker >/dev/null 2>&1; then
        echo "Docker is unavailable" >&2
        return 1
    fi

    authorization_args=(--authorize-start "$container_id")
    verification_args=(--verify-running "$container_id")

    if (( $# == 2 )); then
        openwebui_require_tika_id "$tika_id" || return 1
        authorization_args+=(--tika-id "$tika_id")
        verification_args+=(--tika-id "$tika_id")
    fi

    # Authorization independently checks trusted registration, immutable
    # identity, security configuration, mounts, network and RAG role.
    if ! authorized_id="$("$helper" "${authorization_args[@]}")"; then
        echo "Open WebUI startup authorization failed" >&2
        return 1
    fi

    if [[ "$authorized_id" != "$container_id" ]]; then
        echo "Open WebUI startup authorization identity mismatch" >&2
        return 1
    fi

    if ! docker start "$container_id" >/dev/null; then
        echo "Open WebUI container startup failed" >&2
        return 1
    fi

    # Never declare success merely because Docker accepted start.
    if ! verified_id="$("$helper" "${verification_args[@]}")" ||
       [[ "$verified_id" != "$container_id" ]]; then
        echo "Open WebUI post-start verification failed" >&2

        # Stop only after independently proving the same registered
        # identity. Never remove the container or persistent volume.
        if identity_id="$("$helper" --verify-identity "$container_id")" &&
           [[ "$identity_id" == "$container_id" ]]; then
            if ! docker stop --time 60 "$container_id" >/dev/null; then
                echo "Open WebUI guarded cleanup failed" >&2
            fi
        else
            echo "Open WebUI identity changed; manual recovery required" >&2
        fi
        return 1
    fi

    return 0
}


openwebui_install() {
    local payload_root="${DAIA_PAYLOAD_ROOT:-/opt/daia/payload}"
    local image_file="${payload_root}/images/open-webui.tar"
    local container_name="${DAIA_OPEN_WEBUI_CONTAINER:-open-webui}"
    local volume_name="${DAIA_OPEN_WEBUI_VOLUME:-open-webui}"
    if [[ "$volume_name" != "open-webui" ]]; then
        echo "Unexpected Open WebUI volume configuration" >&2
        return 1
    fi
    local ollama_url="${DAIA_OPEN_WEBUI_OLLAMA_URL:-http://127.0.0.1:11434}"

    openwebui_validate || return 1

    local tika_id=""
    local -a tika_positional=()
    local -a tika_flags=()

    if [[ "${DAIA_ENABLE_TIKA:-false}" == "true" ]]; then
        tika_id="$(openwebui_resolve_registered_tika_id)" || return 1
        tika_positional=("$tika_id")
        tika_flags=(--tika-id "$tika_id")
    fi

    # An interrupted firstboot may leave a registered container behind.
    # Resolve its immutable ID once; never adopt by name or labels alone.
    if docker container inspect "$container_name" >/dev/null 2>&1; then
        local existing_id
        local trusted_id
        local running_id
        local existing_state
        local helper="/usr/libexec/daia-openwebui-register-container"

        if [[ "$EUID" -ne 0 ]] ||
           [[ ! -f "$helper" || -L "$helper" || ! -x "$helper" ]]; then
            echo "Trusted Open WebUI recovery requires root and helper" >&2
            return 1
        fi

        if ! existing_id="$(docker container inspect --format '{{.Id}}' "$container_name")" ||
           [[ ! "$existing_id" =~ ^[0-9a-f]{64}$ ]]; then
            echo "Cannot resolve existing Open WebUI immutable identity" >&2
            return 1
        fi

        if ! trusted_id="$("$helper" --verify-identity "$existing_id")" ||
           [[ "$trusted_id" != "$existing_id" ]]; then
            echo "Existing Open WebUI container is not trusted" >&2
            return 1
        fi

        if ! existing_state="$(docker container inspect --format             '{{.State.Running}}{{printf "|"}}{{.State.Paused}}{{printf "|"}}{{.State.Restarting}}{{printf "|"}}{{.State.Status}}'             "$existing_id")"; then
            echo "Cannot inspect existing Open WebUI state" >&2
            return 1
        fi

        case "$existing_state" in
            "true|false|false|running")
                if ! running_id="$("$helper" --verify-running "$existing_id" "${tika_flags[@]}")" ||
                   [[ "$running_id" != "$existing_id" ]]; then
                    echo "Existing running Open WebUI failed verification" >&2
                    return 1
                fi
                return 0
                ;;
            "false|false|false|created"|"false|false|false|exited")
                openwebui_start_registered "$existing_id" "${tika_positional[@]}" || return 1
                return 0
                ;;
            *)
                echo "Existing Open WebUI is in an unsafe state" >&2
                return 1
                ;;
        esac
    fi

    # Credentials and database role must be verified before authorization.
    openwebui_prepare_rag "${tika_positional[@]}" || return 1

    # RAG integration must be complete before container creation.
    # In particular, never pass database credentials through Docker
    # environment arguments or silently fall back to local pgvector.
    if [[ ! -x /usr/libexec/daia-openwebui-rag-authorize ]]; then
        echo "Open WebUI RAG authorization is not installed" >&2
        return 1
    fi

    if ! /usr/libexec/daia-openwebui-rag-authorize "${tika_flags[@]}"; then
        echo "Open WebUI RAG authorization failed" >&2
        return 1
    fi

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

    # Never remove or replace an existing container implicitly.
    # Existing instances require separately authorized migration.
    if docker container inspect "$container_name" >/dev/null 2>&1
    then
        echo "Existing Open WebUI container requires verified adoption or migration" >&2
        return 1
    fi

    openwebui_ensure_volume || return 1

    # The image archive is checked during payload construction.
    # Docker must execute only the pinned immutable image ID.
    local expected_image="sha256:e29d4d2457b54bb14434bee2666420e822cb0e9389466f02405d69fa7fd8d06c"
    local loaded_image_id
    local container_id

    if ! loaded_image_id="$(docker image inspect         --format '{{.Id}}' "$image_reference")"; then
        echo "Cannot inspect loaded Open WebUI image" >&2
        return 1
    fi

    if [[ "$loaded_image_id" != "$expected_image" ]]; then
        echo "Open WebUI immutable image identity mismatch" >&2
        return 1
    fi

    # Create only. Never start an unregistered container.
    if ! container_id="$(docker container create \
        --network daia-rag-internal \
        --publish 127.0.0.1:3000:3000 \
        --label org.daia.component=openwebui \
        --label org.daia.managed=true \
        --security-opt no-new-privileges:true \
        --env "PORT=3000" \
        --env "VECTOR_DB=pgvector" \
        --volume "${volume_name}:/app/backend/data" \
        --mount "type=bind,src=/var/lib/daia/pgvector/openwebui-password,dst=/run/secrets/daia-openwebui-pgvector-password,readonly" \
        --mount "type=bind,src=/usr/libexec/daia-openwebui-rag-entrypoint.py,dst=/usr/local/bin/daia-openwebui-rag-entrypoint.py,readonly" \
        --env "OLLAMA_BASE_URL=${ollama_url}" \
        --name "$container_name" \
        --restart no \
        "$expected_image" \
        python3 /usr/local/bin/daia-openwebui-rag-entrypoint.py)"; then
        echo "Failed to create Open WebUI container" >&2
        return 1
    fi

    if [[ ! "$container_id" =~ ^[0-9a-f]{64}$ ]]; then
        echo "Docker returned an invalid Open WebUI container ID" >&2
        return 1
    fi

    # The privileged helper must independently inspect Docker metadata
    # before publishing trusted registration.
    if [[ ! -x /usr/libexec/daia-openwebui-register-container ]]; then
        echo "Open WebUI registration helper is unavailable" >&2
        return 1
    fi

    if ! /usr/libexec/daia-openwebui-register-container "${tika_flags[@]}" "$container_id"; then
        echo "Open WebUI registration failed" >&2
        return 1
    fi

    # Start only the independently registered immutable container.
    if ! openwebui_start_registered "$container_id" "${tika_positional[@]}"; then
        echo "Open WebUI guarded startup failed" >&2
        return 1
    fi

    return 0
}

openwebui_verify() {
    local container_name="${DAIA_OPEN_WEBUI_CONTAINER:-open-webui}"
    local helper="/usr/libexec/daia-openwebui-register-container"
    local container_id
    local verified_id
    local configured_env
    local tika_id="${1:-}"
    local -a verification_args

    if [[ "${DAIA_ENABLE_TIKA:-false}" == "true" ]]; then
        local registered_tika_id
        registered_tika_id="$(openwebui_resolve_registered_tika_id)" ||
            return 1

        if [[ -n "$tika_id" && "$tika_id" != "$registered_tika_id" ]]; then
            echo "Requested Tika identity differs from registration" >&2
            return 1
        fi

        tika_id="$registered_tika_id"
    fi

    if (( $# > 1 )); then
        echo "Invalid Open WebUI verification arguments" >&2
        return 1
    fi

    if [[ "$EUID" -ne 0 ]]; then
        echo "Open WebUI verification requires root" >&2
        return 1
    fi

    if ! command -v docker >/dev/null 2>&1; then
        echo "Docker is unavailable" >&2
        return 1
    fi

    if [[ ! -f "$helper" || -L "$helper" || ! -x "$helper" ]]; then
        echo "Trusted Open WebUI verifier is unavailable" >&2
        return 1
    fi

    # Resolve the mutable name once. All subsequent checks use its ID.
    if ! container_id="$(docker container inspect \
        --format '{{.Id}}' "$container_name")"; then
        echo "Cannot resolve Open WebUI container identity" >&2
        return 1
    fi

    if [[ ! "$container_id" =~ ^[0-9a-f]{64}$ ]]; then
        echo "Invalid Open WebUI container identity" >&2
        return 1
    fi

    verification_args=(--verify-running "$container_id")

    if [[ -n "$tika_id" ]]; then
        openwebui_require_tika_id "$tika_id" || return 1
        verification_args+=(--tika-id "$tika_id")
    fi

    # Independent registration, image, security, mount, network,
    # running-state and PostgreSQL RAG checks. No Docker mutation.
    if ! verified_id="$("$helper" "${verification_args[@]}")"; then
        echo "Open WebUI trusted runtime verification failed" >&2
        return 1
    fi

    if [[ "$verified_id" != "$container_id" ]]; then
        echo "Open WebUI verified identity mismatch" >&2
        return 1
    fi

    # Preserve the configured Ollama endpoint contract, pinned to ID.
    if ! configured_env="$(docker container inspect \
        --format '{{range .Config.Env}}{{println .}}{{end}}' \
        "$container_id")"; then
        echo "Cannot inspect Open WebUI configuration" >&2
        return 1
    fi

    if ! grep -Fxq \
        "OLLAMA_BASE_URL=${DAIA_OPEN_WEBUI_OLLAMA_URL:-http://127.0.0.1:11434}" \
        <<< "$configured_env"; then
        echo "Open WebUI Ollama endpoint mismatch" >&2
        return 1
    fi

    return 0
}
