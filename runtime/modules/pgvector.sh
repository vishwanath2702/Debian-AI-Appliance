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



# Read-only PostgreSQL persistent-volume mount verification.
# Return 0 for exactly one matching writable volume mount,
# 1 for configuration mismatch, 2 for inspection failure.
pgvector_container_mount_matches() {
    local container_name="${DAIA_PGVECTOR_CONTAINER:-daia-pgvector}"
    local volume_name="${DAIA_PGVECTOR_VOLUME:-daia-pgvector-data}"
    local expected_destination="/var/lib/postgresql/data"
    local mounts
    local entry
    local mount_type
    local mount_name
    local destination
    local writable
    local extra
    local count=0

    if [[ ! "$container_name" =~ ^[a-zA-Z0-9][a-zA-Z0-9_.-]*$ ]] ||
       [[ ! "$volume_name" =~ ^[a-zA-Z0-9][a-zA-Z0-9_.-]*$ ]]; then
        echo "Invalid pgvector container or volume name" >&2
        return 2
    fi

    if ! mounts="$(docker container inspect \
        --format '{{range .Mounts}}{{printf "%s\t%s\t%s\t%t\n" .Type .Name .Destination .RW}}{{end}}' \
        "$container_name")"; then
        echo "Unable to inspect pgvector container mounts" >&2
        return 2
    fi

    while IFS= read -r entry; do
        [[ -z "$entry" ]] && continue

        # Require exactly four nonempty tab-separated fields.
        # Avoid IFS whitespace collapsing consecutive delimiters.
        if [[ "$entry" != *$'\t'* ]]; then
            echo "Malformed pgvector mount inspection record" >&2
            return 1
        fi

        mount_type="${entry%%$'\t'*}"
        entry="${entry#*$'\t'}"

        if [[ "$entry" != *$'\t'* ]]; then
            echo "Malformed pgvector mount inspection record" >&2
            return 1
        fi

        mount_name="${entry%%$'\t'*}"
        entry="${entry#*$'\t'}"

        if [[ "$entry" != *$'\t'* ]]; then
            echo "Malformed pgvector mount inspection record" >&2
            return 1
        fi

        destination="${entry%%$'\t'*}"
        writable="${entry#*$'\t'}"

        if [[ -z "$mount_type" ]] ||
           [[ -z "$mount_name" ]] ||
           [[ -z "$destination" ]] ||
           [[ "$writable" != "true" && "$writable" != "false" ]]; then
            echo "Malformed pgvector mount inspection record" >&2
            return 1
        fi

        # The expected volume must never be mounted elsewhere.
        if [[ "$mount_name" == "$volume_name" &&
              "$destination" != "$expected_destination" ]]; then
            echo "Pgvector volume mounted at unexpected destination" >&2
            return 1
        fi

        # Initial DAIA contract permits exactly one mount.
        # Reject unrelated mounts until explicitly reviewed.
        count=$((count + 1))

        if [[ "$mount_type" != "volume" ]] ||
           [[ "$mount_name" != "$volume_name" ]] ||
           [[ "$destination" != "$expected_destination" ]] ||
           [[ "$writable" != "true" ]]; then
            echo "Pgvector persistent mount configuration mismatch" >&2
            return 1
        fi
    done <<< "$mounts"

    if [[ "$count" -ne 1 ]]; then
        echo "Expected exactly one PostgreSQL persistent mount" >&2
        return 1
    fi

    return 0
}


pgvector_install() {
    echo "PostgreSQL pgvector provisioning is not implemented yet" >&2
    return 1
}

pgvector_verify() {
    echo "PostgreSQL pgvector verification is not implemented yet" >&2
    return 1
}
