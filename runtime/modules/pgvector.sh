#!/usr/bin/env bash

# DAIA PostgreSQL 17 + pgvector runtime module.
# Sourced by the DAIA runtime; not executed directly.

pgvector_validate() {
    local payload_root="${DAIA_PAYLOAD_ROOT:-/opt/daia/payload}"
    local image_file="${payload_root}/images/pgvector.tar"

    if [[ ! -s "$image_file" ]]; then
        echo "PostgreSQL pgvector image is missing or empty: $image_file" >&2
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


pgvector_container_exists() {
    local container_name="${DAIA_PGVECTOR_CONTAINER:-daia-pgvector}"

    if [[ ! "$container_name" =~ ^[a-zA-Z0-9][a-zA-Z0-9_.-]*$ ]]; then
        echo "Invalid pgvector container name" >&2
        return 2
    fi

    docker container inspect "$container_name" >/dev/null 2>&1
}



# Return 0 when present, 1 when absent, 2 on ambiguous failure.
pgvector_container_status() {
    local container_name="${DAIA_PGVECTOR_CONTAINER:-daia-pgvector}"
    local inventory
    local entry

    if [[ ! "$container_name" =~ ^[a-zA-Z0-9][a-zA-Z0-9_.-]*$ ]]; then
        echo "Invalid pgvector container name" >&2
        return 2
    fi

    if ! inventory="$(docker container ls -a --format '{{.Names}}')"; then
        echo "Unable to inspect Docker container inventory" >&2
        return 2
    fi

    while IFS= read -r entry; do
        if [[ "$entry" == "$container_name" ]]; then
            return 0
        fi
    done <<< "$inventory"

    return 1
}



# Read-only ownership check. Return 0 only for explicitly DAIA-managed
# pgvector containers. Return 1 for mismatch, 2 for inspection failure.
pgvector_container_owned() {
    local container_name="${DAIA_PGVECTOR_CONTAINER:-daia-pgvector}"
    local component
    local managed

    if [[ ! "$container_name" =~ ^[a-zA-Z0-9][a-zA-Z0-9_.-]*$ ]]; then
        echo "Invalid pgvector container name" >&2
        return 2
    fi

    if ! component="$(docker container inspect \
        --format '{{index .Config.Labels "org.daia.component"}}' \
        "$container_name")"; then
        echo "Unable to inspect pgvector component ownership" >&2
        return 2
    fi

    if ! managed="$(docker container inspect \
        --format '{{index .Config.Labels "org.daia.managed"}}' \
        "$container_name")"; then
        echo "Unable to inspect pgvector management ownership" >&2
        return 2
    fi

    if [[ "$component" != "pgvector" || "$managed" != "true" ]]; then
        echo "Container is not DAIA-managed pgvector" >&2
        return 1
    fi

    return 0
}



# Read-only configured image-reference check.
# This does not establish image digest integrity.
pgvector_container_image_matches() {
    local container_name="${DAIA_PGVECTOR_CONTAINER:-daia-pgvector}"
    local expected_image="${DAIA_PGVECTOR_IMAGE_REF:-pgvector/pgvector:pg17}"
    local actual_image

    if [[ ! "$container_name" =~ ^[a-zA-Z0-9][a-zA-Z0-9_.-]*$ ]]; then
        echo "Invalid pgvector container name" >&2
        return 2
    fi

    if [[ "$expected_image" != "pgvector/pgvector:pg17" ]]; then
        echo "Unexpected pgvector image reference" >&2
        return 2
    fi

    if ! actual_image="$(docker container inspect \
        --format '{{.Config.Image}}' \
        "$container_name")"; then
        echo "Unable to inspect pgvector container image" >&2
        return 2
    fi

    if [[ "$actual_image" != "$expected_image" ]]; then
        echo "Pgvector container image reference mismatch" >&2
        return 1
    fi

    return 0
}




# Verify the immutable image ID actually used by an existing container.
# A matching image tag alone is insufficient.
pgvector_container_image_id_matches() {
    local container_name="${DAIA_PGVECTOR_CONTAINER:-daia-pgvector}"
    local expected_id="${DAIA_PGVECTOR_DOCKER_IMAGE_ID:-}"
    local actual_id

    if [[ ! "$container_name" =~ ^[a-zA-Z0-9][a-zA-Z0-9_.-]*$ ]] ||
       [[ ! "$expected_id" =~ ^sha256:[0-9a-f]{64}$ ]]; then
        echo "Invalid pgvector container image identity configuration" >&2
        return 2
    fi

    if ! actual_id="$(docker container inspect         --format '{{.Image}}' "$container_name")"; then
        echo "Unable to inspect pgvector container image ID" >&2
        return 2
    fi

    if [[ "$actual_id" != "$expected_id" ]]; then
        echo "Pgvector container immutable image ID mismatch" >&2
        return 1
    fi

    return 0
}


# Read-only volume ownership verification.
# Return 0 for matching DAIA labels, 1 for mismatch,
# and 2 for invalid configuration or Docker inspection failure.
pgvector_volume_owned() {
    local volume_name="${DAIA_PGVECTOR_VOLUME:-daia-pgvector-data}"
    local component
    local managed

    if [[ ! "$volume_name" =~ ^[a-zA-Z0-9][a-zA-Z0-9_.-]*$ ]]; then
        echo "Invalid pgvector volume name" >&2
        return 2
    fi

    if ! component="$(docker volume inspect \
        --format '{{index .Labels "org.daia.component"}}' \
        "$volume_name")"; then
        echo "Unable to inspect pgvector volume component ownership" >&2
        return 2
    fi

    if ! managed="$(docker volume inspect \
        --format '{{index .Labels "org.daia.managed"}}' \
        "$volume_name")"; then
        echo "Unable to inspect pgvector volume management ownership" >&2
        return 2
    fi

    if [[ "$component" != "pgvector" || "$managed" != "true" ]]; then
        echo "Volume is not DAIA-managed pgvector storage" >&2
        return 1
    fi

    return 0
}



# Verify the two explicitly authorized PostgreSQL mounts.
# No additional mounts are permitted.
pgvector_container_mount_matches() {
    local container_name="${DAIA_PGVECTOR_CONTAINER:-daia-pgvector}"
    local volume_name="${DAIA_PGVECTOR_VOLUME:-daia-pgvector-data}"
    local mounts
    local entry
    local expected_volume=0
    local expected_secret=0
    local expected_pgpass=0
    local count=0

    [[ "$container_name" =~ ^[a-zA-Z0-9][a-zA-Z0-9_.-]*$ ]] || return 2
    [[ "$volume_name" =~ ^[a-zA-Z0-9][a-zA-Z0-9_.-]*$ ]] || return 2

    if ! mounts="$(docker container inspect \
        --format '{{range .Mounts}}{{printf "%s|%s|%s|%s|%t\n" .Type .Name .Source .Destination .RW}}{{end}}' \
        "$container_name")"; then
        return 2
    fi

    while IFS= read -r entry; do
        [[ -z "$entry" ]] && continue
        count=$((count + 1))

        case "$entry" in
            "volume|${volume_name}|"*"|/var/lib/postgresql/data|true")
                # Docker must report a nonempty absolute volume source.
                # Reject malformed and ambiguous mount records.
                local mount_type mount_name mount_source
                local mount_destination mount_writable mount_extra

                IFS='|' read -r mount_type mount_name mount_source \
                    mount_destination mount_writable mount_extra <<< "$entry"

                if [[ -z "$mount_source" ||
                      "$mount_source" != /* ||
                      -n "$mount_extra" ]]; then
                    echo "Invalid PostgreSQL volume mount source" >&2
                    return 1
                fi

                expected_volume=$((expected_volume + 1))
                ;;
            "bind||/var/lib/daia/pgvector/postgres-password|/run/secrets/daia-pgvector-password|false")
                expected_secret=$((expected_secret + 1))
                ;;
            "bind||/var/lib/daia/pgvector/pgpass|/run/secrets/daia-pgvector-pgpass|false")
                expected_pgpass=$((expected_pgpass + 1))
                ;;
            *)
                echo "Unexpected PostgreSQL container mount" >&2
                return 1
                ;;
        esac
    done <<< "$mounts"

    if [[ "$count" -ne 3 ||
          "$expected_volume" -ne 1 ||
          "$expected_secret" -ne 1 ||
          "$expected_pgpass" -ne 1 ]]; then
        echo "PostgreSQL container mount contract mismatch" >&2
        return 1
    fi
}

# Verify the actual Docker image ID, not merely its mutable tag.
# Return 0 for an exact match, 1 for mismatch, 2 for inspection
# or configuration failure. This function never mutates Docker.
pgvector_docker_image_id_matches() {
    local expected_id="${DAIA_PGVECTOR_DOCKER_IMAGE_ID:-}"
    local image_reference="${DAIA_PGVECTOR_IMAGE_REF:-}"
    local actual_id

    if [[ ! "$expected_id" =~ ^sha256:[0-9a-f]{64}$ ]]; then
        echo "Invalid configured pgvector Docker image ID" >&2
        return 2
    fi

    if [[ "$image_reference" != "pgvector/pgvector:pg17" ]]; then
        echo "Unexpected configured pgvector image reference" >&2
        return 2
    fi

    if ! actual_id="$(docker image inspect \
        --format '{{.Id}}' "$image_reference")"; then
        echo "Unable to inspect pgvector Docker image" >&2
        return 2
    fi

    if [[ "$actual_id" != "$expected_id" ]]; then
        echo "Pgvector Docker image ID mismatch" >&2
        return 1
    fi

    return 0
}



# Load the pinned offline image without replacing an existing image tag.
# This function must not create, start, stop, or remove containers.
pgvector_load_verified_offline_image() {
    local payload_root="${DAIA_PAYLOAD_ROOT:-/opt/daia/payload}"
    local archive="${payload_root}/images/pgvector.tar"
    local expected_id="${DAIA_PGVECTOR_DOCKER_IMAGE_ID:-}"
    local expected_sha="${DAIA_PGVECTOR_ARCHIVE_SHA256:-}"
    local image_reference="${DAIA_PGVECTOR_IMAGE_REF:-}"
    local actual_sha
    local inspection
    local inspection_status

    if [[ "$expected_id" != "sha256:366e1863bdeefd735562caf0ebb4c8b05b4cb37bd8eec60d516c9083209746a2" ]] ||
       [[ "$expected_sha" != "5d903322c843abad607f7fa9349b0b7a1c1a53c02b1306eada13451d2a367228" ]] ||
       [[ "$image_reference" != "pgvector/pgvector:pg17" ]]; then
        echo "Unexpected pgvector image configuration" >&2
        return 1
    fi

    # The Python helper verifies the archive through descriptor-relative
    # O_NOFOLLOW traversal, snapshots the bytes and hashes that snapshot.
    # Do not reopen the archive pathname for Docker consumption.
    if [[ ! -f "$archive" || -L "$archive" ]]; then
        echo "Pgvector offline archive is missing or unsafe" >&2
        return 1
    fi
    if [[ ! -f /opt/daia/pgvector_archive.py ]]; then
        echo "Pgvector archive verifier is not installed" >&2
        return 1
    fi

    if ! command -v docker >/dev/null 2>&1; then
        echo "Docker is unavailable" >&2
        return 1
    fi

    if ! docker info >/dev/null 2>&1; then
        echo "Docker daemon is unavailable" >&2
        return 1
    fi

    # Inventory distinguishes a genuinely absent tag from inspection errors.
    if ! inspection="$(docker image ls --format '{{.Repository}}:{{.Tag}}')"; then
        echo "Unable to inspect Docker image inventory" >&2
        return 1
    fi

    if printf '%s\n' "$inspection" | grep -Fxq -- "$image_reference"; then
        pgvector_docker_image_id_matches
        return $?
    fi

    # No existing image tag is overwritten or removed.
    if ! /usr/bin/python3 /opt/daia/pgvector_archive.py load "$archive"; then
        echo "Unable to load verified pgvector offline image" >&2
        return 1
    fi

    pgvector_docker_image_id_matches
}


# Create or reuse the persistent PostgreSQL volume.
# Existing volumes are reused only after DAIA ownership verification.
# No existing volume is deleted or modified.
pgvector_ensure_volume() {
    local volume_name="${DAIA_PGVECTOR_VOLUME:-daia-pgvector-data}"
    local inventory
    local entry
    local found=false

    if [[ ! "$volume_name" =~ ^[a-zA-Z0-9][a-zA-Z0-9_.-]*$ ]]; then
        echo "Invalid pgvector volume name" >&2
        return 2
    fi

    if ! inventory="$(docker volume ls --format '{{.Name}}')"; then
        echo "Unable to inspect Docker volume inventory" >&2
        return 2
    fi

    while IFS= read -r entry; do
        if [[ "$entry" == "$volume_name" ]]; then
            found=true
            break
        fi
    done <<< "$inventory"

    if [[ "$found" == "true" ]]; then
        pgvector_volume_owned
        return $?
    fi

    if ! docker volume create \
        --label org.daia.component=pgvector \
        --label org.daia.managed=true \
        "$volume_name" >/dev/null; then
        echo "Unable to create pgvector persistent volume" >&2
        return 1
    fi

    if ! pgvector_volume_owned; then
        echo "Created pgvector volume failed ownership verification" >&2
        return 1
    fi

    return 0
}

pgvector_ensure_rag_network() {
    local network_name="daia-rag-internal"

    if [[ "$EUID" -ne 0 ]]; then
        echo "RAG network provisioning requires root" >&2
        return 1
    fi

    if ! docker info >/dev/null 2>&1; then
        echo "Docker daemon is unavailable" >&2
        return 1
    fi

    local network_inventory
    if ! network_inventory="$(docker network ls --format '{{.Name}}')"; then
        echo "Unable to inspect Docker networks" >&2
        return 1
    fi

    if printf '%s\n' "$network_inventory" | grep -Fxq "$network_name"; then
        /usr/libexec/daia-rag-verify-network >/dev/null || return 1
        return 0
    fi

    if ! docker network create \
        --driver bridge \
        --internal \
        --label org.daia.component=rag \
        --label org.daia.managed=true \
        "$network_name" >/dev/null; then
        echo "Unable to create DAIA RAG internal network" >&2
        return 1
    fi

    /usr/libexec/daia-rag-verify-network >/dev/null || return 1
}

pgvector_install() {
    local container_name="${DAIA_PGVECTOR_CONTAINER:-daia-pgvector}"
    local volume_name="${DAIA_PGVECTOR_VOLUME:-daia-pgvector-data}"
    local expected_id="${DAIA_PGVECTOR_DOCKER_IMAGE_ID:-}"

    if [[ "$EUID" -ne 0 ]]; then
        echo "PostgreSQL provisioning requires root" >&2
        return 1
    fi

    if [[ "$container_name" != "daia-pgvector" ||
          "$volume_name" != "daia-pgvector-data" ||
          "$expected_id" != "sha256:366e1863bdeefd735562caf0ebb4c8b05b4cb37bd8eec60d516c9083209746a2" ]]; then
        echo "Unexpected PostgreSQL provisioning configuration" >&2
        return 1
    fi

    # Create only when absent; reject existing untrusted networks.
    pgvector_ensure_rag_network || return 1

    pgvector_load_verified_offline_image || return 1

    /usr/bin/python3 /opt/daia/pgvector_credentials.py \
        --state-directory /var/lib/daia/pgvector || return 1

    pgvector_ensure_volume || return 1

    # Provision and register before allowing systemd startup.
    pgvector_provision_container || return 1

    # Systemd owns the running container lifecycle. The service's
    # ExecStart helper independently authorizes the immutable ID.
    if ! systemctl start daia-pgvector.service; then
        echo "PostgreSQL systemd startup failed" >&2
        return 1
    fi

    return 0
}

# Verify the immutable container's Docker security configuration.
# Return failure for missing, unexpected, or privileged settings.
pgvector_container_security_matches() {
    local container_id="${1:-}"
    local security

    if [[ ! "$container_id" =~ ^[0-9a-f]{64}$ ]]; then
        echo "Invalid PostgreSQL container identity for security check" >&2
        return 1
    fi

    if ! security="$(docker container inspect \
        --format '{{join .HostConfig.SecurityOpt ","}}{{printf "|"}}{{.HostConfig.Privileged}}' \
        "$container_id")"; then
        echo "Cannot inspect PostgreSQL container security" >&2
        return 1
    fi

    if [[ "$security" != "no-new-privileges:true|false" ]]; then
        echo "PostgreSQL container security contract mismatch" >&2
        return 1
    fi

    return 0
}


# Container-only provisioning boundary. Called after the privileged
# installation prerequisites have succeeded.
pgvector_provision_container() {
    local container_name="${DAIA_PGVECTOR_CONTAINER:-daia-pgvector}"
    local volume_name="${DAIA_PGVECTOR_VOLUME:-daia-pgvector-data}"
    local expected_id="${DAIA_PGVECTOR_DOCKER_IMAGE_ID:-}"
    local container_id
    local status

    # Keep the immutable production configuration contract.
    if [[ "$container_name" != "daia-pgvector" ||
          "$volume_name" != "daia-pgvector-data" ||
          "$expected_id" != "sha256:366e1863bdeefd735562caf0ebb4c8b05b4cb37bd8eec60d516c9083209746a2" ]]; then
        echo "Unexpected PostgreSQL provisioning configuration" >&2
        return 1
    fi

    status=0
    pgvector_container_status || status=$?

    if [[ "$status" -ne 0 && "$status" -ne 1 ]]; then
        echo "Cannot safely inspect existing PostgreSQL containers" >&2
        return 1
    fi

    if [[ "$status" -eq 0 ]]; then
        # An existing container is never enrolled automatically.
        # DAIA must already have registered its immutable identity.
        if ! container_id="$(docker container inspect \
            --format '{{.Id}}' "$container_name")"; then
            return 1
        fi

        [[ "$container_id" =~ ^[0-9a-f]{64}$ ]] || return 1

        /usr/libexec/daia-pgvector-verify-registration \
            "$container_id" || return 1
    fi

    if [[ "$status" -eq 1 ]]; then
        # Never expose the password through Docker arguments or environment.
        # The container is created but not started.
        if ! container_id="$(docker container create \
            --name "$container_name" \
            --label org.daia.component=pgvector \
            --label org.daia.managed=true \
            --restart no \
            --security-opt no-new-privileges:true \
            --network daia-rag-internal \
            --mount "type=volume,src=${volume_name},dst=/var/lib/postgresql/data" \
            --mount "type=bind,src=/var/lib/daia/pgvector/postgres-password,dst=/run/secrets/daia-pgvector-password,readonly" \
            --mount "type=bind,src=/var/lib/daia/pgvector/pgpass,dst=/run/secrets/daia-pgvector-pgpass,readonly" \
            --env POSTGRES_USER=daia_rag \
            --env POSTGRES_DB=daia_rag \
            --env POSTGRES_PASSWORD_FILE=/run/secrets/daia-pgvector-password \
            --env "POSTGRES_INITDB_ARGS=--auth-local=scram-sha-256 --auth-host=scram-sha-256" \
            "$expected_id")"; then
            echo "Unable to create PostgreSQL container" >&2
            return 1
        fi

        [[ "$container_id" =~ ^[0-9a-f]{64}$ ]] || {
            echo "Docker returned an invalid container ID" >&2
            return 1
        }
    fi

    pgvector_container_owned || return 1
    pgvector_container_image_id_matches || return 1
    pgvector_container_mount_matches || return 1
    pgvector_volume_owned || return 1
    pgvector_container_security_matches "$container_id" || return 1

    # Preserve the ID obtained from creation or trusted pre-verification.
    # Never overwrite it with the result of a later name-based lookup.
    local verified_id
    if ! verified_id="$(docker container inspect \
        --format '{{.Id}}' "$container_name")"; then
        return 1
    fi

    if [[ ! "$verified_id" =~ ^[0-9a-f]{64}$ ]]; then
        echo "Invalid inspected PostgreSQL container identity" >&2
        return 1
    fi

    if [[ "$verified_id" != "$container_id" ]]; then
        echo "PostgreSQL container identity changed during provisioning" >&2
        return 1
    fi

    if [[ "$status" -eq 0 ]]; then
        /usr/libexec/daia-pgvector-verify-registration \
            "$container_id"
        return $?
    fi

    /usr/bin/python3 /opt/daia/pgvector_register_container.py \
        "$container_id"
}

pgvector_verify() {
    local container_name="${DAIA_PGVECTOR_CONTAINER:-daia-pgvector}"
    local container_id
    local running

    [[ "$EUID" -eq 0 ]] || return 1
    [[ "$container_name" == "daia-pgvector" ]] || return 1

    command -v docker >/dev/null 2>&1 || return 1
    docker info >/dev/null 2>&1 || return 1

    container_id="$(docker container inspect         --format '{{.Id}}' "$container_name")" || return 1

    [[ "$container_id" =~ ^[0-9a-f]{64}$ ]] || return 1

    /usr/libexec/daia-pgvector-verify-registration         "$container_id" || return 1

    pgvector_container_owned || return 1
    pgvector_container_image_id_matches || return 1
    pgvector_container_mount_matches || return 1
    pgvector_volume_owned || return 1
    pgvector_container_security_matches "$container_id" || return 1

    running="$(docker container inspect         --format '{{.State.Running}}' "$container_id")" || return 1

    [[ "$running" == "true" ]] || return 1

    docker exec "$container_id" \
        pg_isready -h /var/run/postgresql \
        -U daia_rag -d daia_rag >/dev/null || return 1

    # Existing database volumes must pass the read-only SCRAM audit.
    # Never report pgvector healthy when authentication is unsafe.
    /usr/libexec/daia-pgvector-audit-auth.py "$container_id" >/dev/null || return 1

    # Read-only verification: the PostgreSQL server must have
    # the pgvector extension installed in the appliance database.
    local extension
    extension="$(docker exec "$container_id" \
        psql -X -w -A -t -v ON_ERROR_STOP=1 \
        -d "host=127.0.0.1 port=5432 dbname=daia_rag user=daia_rag passfile=/run/secrets/daia-pgvector-pgpass require_auth=scram-sha-256" \
        -c "SELECT extname FROM pg_extension WHERE extname = 'vector'")" || return 1

    [[ "$extension" == "vector" ]] || return 1

    return 0
}
