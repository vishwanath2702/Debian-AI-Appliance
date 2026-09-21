#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

TARGET_ROOTFS="$PROJECT_ROOT/work/package-state-rootfs"

if [[ "$EUID" -ne 0 ]]; then
    echo "This script must be run as root." >&2
    exit 1
fi

if ! command -v debootstrap >/dev/null 2>&1 && [[ ! -x /usr/sbin/debootstrap ]]; then
    echo "Required command is unavailable: debootstrap" >&2
    exit 1
fi

DEBOOTSTRAP="$(command -v debootstrap || true)"
DEBOOTSTRAP="${DEBOOTSTRAP:-/usr/sbin/debootstrap}"

rm -rf "$TARGET_ROOTFS"

"$DEBOOTSTRAP" \
    --variant=minbase \
    trixie \
    "$TARGET_ROOTFS" \
    http://deb.debian.org/debian

chroot "$TARGET_ROOTFS" \
    env DEBIAN_FRONTEND=noninteractive \
    apt-get \
    --yes \
    install \
    '?priority(standard)' \
    task-xfce-desktop
