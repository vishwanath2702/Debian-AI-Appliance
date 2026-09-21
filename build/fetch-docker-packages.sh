#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

TARGET_STATUS="$PROJECT_ROOT/work/package-state-rootfs/var/lib/dpkg/status"
DOCKER_PACKAGE_DIR="$PROJECT_ROOT/payload/packages/docker"

if [[ ! -f "$TARGET_STATUS" ]]; then
    echo "DAIA target package status not found: $TARGET_STATUS" >&2
    exit 1
fi

if ! command -v apt-get >/dev/null 2>&1; then
    echo "Required command is unavailable: apt-get" >&2
    exit 1
fi

mkdir -p "$DOCKER_PACKAGE_DIR"

rm -f "$DOCKER_PACKAGE_DIR"/*.deb

apt-get \
    --download-only \
    --yes \
    --no-install-recommends \
    -o "Dir::State::status=$TARGET_STATUS" \
    -o "Dir::Cache::archives=$DOCKER_PACKAGE_DIR" \
    install \
    docker.io \
    docker-cli

rm -f "$DOCKER_PACKAGE_DIR/lock"
rm -rf "$DOCKER_PACKAGE_DIR/partial"
