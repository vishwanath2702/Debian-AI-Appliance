#!/bin/sh
set -eux

TARGET_ROOT="/target"
SOURCE_ROOT="/cdrom/daia"
LOG="$TARGET_ROOT/var/log/daia-late-install.log"

mkdir -p "$TARGET_ROOT/var/log"
exec >"$LOG" 2>&1

echo "DAIA late installation started"
date

# Verify required source files.
test -d "$SOURCE_ROOT/opt/daia"
test -f "$SOURCE_ROOT/opt/daia/install.sh"
test -f "$SOURCE_ROOT/opt/daia/bootstrap.sh"
test -f "$SOURCE_ROOT/etc/systemd/system/daia-firstboot.service"

# Create target locations.
mkdir -p "$TARGET_ROOT/opt"
mkdir -p "$TARGET_ROOT/etc/systemd/system"
mkdir -p "$TARGET_ROOT/etc/systemd/system/multi-user.target.wants"
mkdir -p "$TARGET_ROOT/var/lib/daia"

# Copy the runtime payload.
rm -rf "$TARGET_ROOT/opt/daia"
cp -R "$SOURCE_ROOT/opt/daia" "$TARGET_ROOT/opt/daia"

# Install and enable the first-boot service.
cp "$SOURCE_ROOT/etc/systemd/system/daia-firstboot.service" \
   "$TARGET_ROOT/etc/systemd/system/daia-firstboot.service"

chmod 0755 "$TARGET_ROOT/opt/daia/install.sh"
chmod 0755 "$TARGET_ROOT/opt/daia/bootstrap.sh"

ln -sf /etc/systemd/system/daia-firstboot.service \
   "$TARGET_ROOT/etc/systemd/system/multi-user.target.wants/daia-firstboot.service"

# Installation marker.
{
    echo "DAIA installer hook completed successfully."
    date -u
} >"$TARGET_ROOT/var/lib/daia/installer-complete"

echo "DAIA late installation completed successfully"
exit 0
