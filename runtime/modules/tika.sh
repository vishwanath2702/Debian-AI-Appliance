#!/usr/bin/env bash
# DAIA Apache Tika offline runtime module.
# Container creation remains disabled pending authorization integration.

tika_validate() {
    local payload_root="${DAIA_PAYLOAD_ROOT:-/opt/daia/payload}"
    local archive="${payload_root}/images/tika.tar"
    local expected_sha="ed3fbd4c58bd8f86e7c932f31d250490d31fbb175b318e22fcb6f4f96baa5ae4"
    local actual_sha

    if [[ "${DAIA_ENABLE_TIKA:-false}" != "true" ]]; then
        echo "Tika runtime is not enabled" >&2
        return 1
    fi

    if [[ "${DAIA_TIKA_DOCKER_IMAGE_ID:-}" != "sha256:3f1999aaaa706a5427e8c0d3139b30ba5973e5e0489a43f4f7fd2491a4172d6a" ]]; then
        echo "Unexpected Tika image identity" >&2
        return 1
    fi

    if [[ ! -f "$archive" || -L "$archive" ]]; then
        echo "Tika offline archive is unavailable or unsafe" >&2
        return 1
    fi

    if ! actual_sha="$(sha256sum -- "$archive")"; then
        echo "Unable to verify Tika archive" >&2
        return 1
    fi

    actual_sha="${actual_sha%% *}"

    if [[ "$actual_sha" != "$expected_sha" ]]; then
        echo "Tika offline archive checksum mismatch" >&2
        return 1
    fi

    return 0
}

tika_load_verified_offline_image() {
    local payload_root="${DAIA_PAYLOAD_ROOT:-/opt/daia/payload}"
    local archive="${payload_root}/images/tika.tar"
    local expected_id="sha256:3f1999aaaa706a5427e8c0d3139b30ba5973e5e0489a43f4f7fd2491a4172d6a"
    local image_ref="docker.io/apache/tika:3.3.3.0"
    local loaded_id

    if [[ "$EUID" -ne 0 ]]; then
        echo "Tika image loading requires root" >&2
        return 1
    fi

    tika_validate || return 1

    if ! command -v docker >/dev/null 2>&1 ||
       ! docker info >/dev/null 2>&1; then
        echo "Docker is unavailable for Tika image loading" >&2
        return 1
    fi

    if ! docker load --input "$archive" >/dev/null; then
        echo "Unable to load verified Tika image archive" >&2
        return 1
    fi

    if ! loaded_id="$(docker image inspect --format '{{.Id}}' "$image_ref")"; then
        echo "Unable to inspect loaded Tika image" >&2
        return 1
    fi

    if [[ "$loaded_id" != "$expected_id" ]]; then
        echo "Loaded Tika immutable image identity mismatch" >&2
        return 1
    fi

    return 0
}

tika_provision_container() {
    local expected_id="sha256:3f1999aaaa706a5427e8c0d3139b30ba5973e5e0489a43f4f7fd2491a4172d6a"
    local container_name="daia-tika"
    local container_id
    local inspected_id

    if [[ "$EUID" -ne 0 ]]; then
        echo "Tika container provisioning requires root" >&2
        return 1
    fi

    tika_validate || return 1

    if ! command -v docker >/dev/null 2>&1 ||
       ! docker info >/dev/null 2>&1; then
        echo "Docker is unavailable for Tika provisioning" >&2
        return 1
    fi

    if [[ ! -x /usr/libexec/daia-tika-register-container ]]; then
        echo "Tika privileged registration helper is unavailable" >&2
        return 1
    fi

    # Read the trusted PostgreSQL identity through the existing
    # root-owned registration contract, never from Docker labels.
    local postgresql_id
    if ! postgresql_id="$(/usr/libexec/daia-pgvector-authorize-start runtime)"; then
        echo "Unable to read trusted PostgreSQL registration" >&2
        return 1
    fi

    if [[ ! "$postgresql_id" =~ ^[0-9a-f]{64}$ ]]; then
        echo "Invalid trusted PostgreSQL identity" >&2
        return 1
    fi

    if ! /usr/libexec/daia-rag-verify-network registration "$postgresql_id"; then
        echo "Tika network registration prerequisites failed" >&2
        return 1
    fi

    # Refuse implicit adoption or replacement of an existing container.
    if docker container inspect "$container_name" >/dev/null 2>&1; then
        echo "Existing Tika container requires separately authorized adoption" >&2
        return 1
    fi

    tika_load_verified_offline_image || return 1

    # Create only; no published ports, host mounts, or automatic restart.
    if ! container_id="$(docker container create \
        --name "$container_name" \
        --label org.daia.component=tika \
        --label org.daia.managed=true \
        --restart no \
        --security-opt no-new-privileges:true \
        --network daia-rag-internal \
        "$expected_id")"; then
        echo "Unable to create Tika container" >&2
        return 1
    fi

    if [[ ! "$container_id" =~ ^[0-9a-f]{64}$ ]]; then
        echo "Docker returned an invalid Tika container identity" >&2
        return 1
    fi

    if ! inspected_id="$(docker container inspect \
        --format '{{.Id}}' "$container_name")"; then
        return 1
    fi

    if [[ "$inspected_id" != "$container_id" ]]; then
        echo "Tika container identity changed during provisioning" >&2
        return 1
    fi

    /usr/libexec/daia-tika-register-container "$container_id" || return 1

    # Startup remains intentionally disabled.
    return 0
}


tika_start_registered() {
    local container_id="${1:-}"
    local helper="/usr/libexec/daia-tika-register-container"
    local authorized_id verified_id trusted_id

    if [[ "$EUID" -ne 0 ]]; then
        echo "Tika startup requires root" >&2
        return 1
    fi

    if [[ ! "$container_id" =~ ^[0-9a-f]{64}$ ]]; then
        echo "Invalid Tika startup identity" >&2
        return 1
    fi

    if [[ ! -f "$helper" || -L "$helper" || ! -x "$helper" ]]; then
        echo "Trusted Tika helper is unavailable" >&2
        return 1
    fi

    if ! command -v docker >/dev/null 2>&1 ||
       ! docker info >/dev/null 2>&1; then
        echo "Docker is unavailable for Tika startup" >&2
        return 1
    fi

    if ! authorized_id="$("$helper" --authorize-start "$container_id")" ||
       [[ "$authorized_id" != "$container_id" ]]; then
        echo "Tika startup authorization failed" >&2
        return 1
    fi

    if ! docker start "$container_id" >/dev/null; then
        echo "Unable to start registered Tika container" >&2
        return 1
    fi

    verified_id=""
    if ! verified_id="$("$helper" --verify-running "$container_id")" ||
       [[ "$verified_id" != "$container_id" ]]; then
        # Do not stop a container whose trusted identity is unverified.
        if trusted_id="$("$helper" --verify-identity "$container_id")" &&
           [[ "$trusted_id" == "$container_id" ]]; then
            docker stop --time 60 "$container_id" >/dev/null || true
        fi
        echo "Tika post-start verification failed" >&2
        return 1
    fi

    return 0
}

tika_verify_registered() {
    local helper="/usr/libexec/daia-tika-register-container"
    local container_id verified_id

    if [[ "$EUID" -ne 0 ]]; then
        echo "Tika verification requires root" >&2
        return 1
    fi

    if [[ ! -f "$helper" || -L "$helper" || ! -x "$helper" ]]; then
        echo "Trusted Tika helper is unavailable" >&2
        return 1
    fi

    if ! command -v docker >/dev/null 2>&1 ||
       ! docker info >/dev/null 2>&1; then
        echo "Docker is unavailable for Tika verification" >&2
        return 1
    fi

    if ! container_id="$(docker container inspect \
        --format '{{.Id}}' daia-tika)"; then
        echo "Cannot inspect registered Tika container" >&2
        return 1
    fi

    if [[ ! "$container_id" =~ ^[0-9a-f]{64}$ ]]; then
        echo "Invalid registered Tika identity" >&2
        return 1
    fi

    if ! verified_id="$("$helper" --verify-running "$container_id")" ||
       [[ "$verified_id" != "$container_id" ]]; then
        echo "Registered Tika runtime verification failed" >&2
        return 1
    fi

    return 0
}



tika_install_registered() {
    local container_name="daia-tika"
    local helper="/usr/libexec/daia-tika-register-container"
    local container_id verified_id state

    if [[ "$EUID" -ne 0 ]]; then
        echo "Tika installation requires root" >&2
        return 1
    fi

    tika_validate || return 1

    if [[ ! -f "$helper" || -L "$helper" || ! -x "$helper" ]]; then
        echo "Trusted Tika registration helper is unavailable" >&2
        return 1
    fi

    if ! command -v docker >/dev/null 2>&1 ||
       ! docker info >/dev/null 2>&1; then
        echo "Docker is unavailable for Tika installation" >&2
        return 1
    fi

    if docker container inspect "$container_name" >/dev/null 2>&1; then
        # An existing container must already have trusted registration.
        if ! container_id="$(docker container inspect \
            --format '{{.Id}}' "$container_name")"; then
            return 1
        fi

        if [[ ! "$container_id" =~ ^[0-9a-f]{64}$ ]]; then
            echo "Invalid existing Tika identity" >&2
            return 1
        fi

        if ! verified_id="$("$helper" --verify-identity "$container_id")" ||
           [[ "$verified_id" != "$container_id" ]]; then
            echo "Existing Tika container is not trusted" >&2
            return 1
        fi

        if ! state="$(docker container inspect \
            --format '{{.State.Running}}{{printf "|"}}{{.State.Paused}}{{printf "|"}}{{.State.Restarting}}{{printf "|"}}{{.State.Status}}' \
            "$container_id")"; then
            return 1
        fi

        case "$state" in
            "true|false|false|running")
                # Verify only; do not restart a running container.
                tika_verify_registered || return 1
                ;;
            "false|false|false|created"|"false|false|false|exited")
                tika_start_registered "$container_id" || return 1
                tika_verify_registered || return 1
                ;;
            *)
                echo "Unsafe existing Tika container state" >&2
                return 1
                ;;
        esac
        return 0
    fi

    # A new container is created and registered by the existing
    # provisioning function. Do not adopt an untrusted replacement.
    tika_provision_container || return 1

    if ! container_id="$(docker container inspect \
        --format '{{.Id}}' "$container_name")"; then
        return 1
    fi

    if [[ ! "$container_id" =~ ^[0-9a-f]{64}$ ]]; then
        echo "Invalid newly provisioned Tika identity" >&2
        return 1
    fi

    if ! verified_id="$("$helper" --verify-identity "$container_id")" ||
       [[ "$verified_id" != "$container_id" ]]; then
        echo "New Tika registration verification failed" >&2
        return 1
    fi

    tika_start_registered "$container_id" || return 1
    tika_verify_registered || return 1
    return 0
}

tika_install() {
    tika_install_registered || return 1
}

tika_verify() {
    tika_verify_registered || return 1
}
