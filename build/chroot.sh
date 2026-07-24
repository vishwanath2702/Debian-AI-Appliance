#!/bin/bash

set -e

ROOTFS="$(pwd)/rootfs"

sudo mount --bind /dev "$ROOTFS/dev"
sudo mount --bind /dev/pts "$ROOTFS/dev/pts"
sudo mount --bind /proc "$ROOTFS/proc"
sudo mount --bind /sys "$ROOTFS/sys"

sudo cp /etc/resolv.conf "$ROOTFS/etc/resolv.conf"

sudo chroot "$ROOTFS" /bin/bash
