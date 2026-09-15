#!/usr/bin/env bash

set -u

readonly TEST_DAIA_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_SERVICE_FILE="${TEST_DAIA_ROOT}/../../etc/systemd/system/daia-firstboot.service"
readonly TEST_CONFIG_FILE="${TEST_DAIA_ROOT}/config/daia.conf"
readonly TEST_MODULES_FILE="${TEST_DAIA_ROOT}/config/modules.conf"

fail()
{
    printf 'FAIL: %s\n' "$1" >&2
    exit 1
}

grep -Fxq \
    'ExecStart=/opt/daia/install.sh' \
    "$TEST_SERVICE_FILE" \
    || fail "first-boot service does not execute install.sh"

grep -Fxq \
    'ConditionPathExists=!/var/lib/daia/firstboot-complete' \
    "$TEST_SERVICE_FILE" \
    || fail "first-boot service does not use the completion marker"

grep -Fq \
    'DAIA_FIRSTBOOT_STATE="${DAIA_STATE_DIR}/firstboot.state"' \
    "$TEST_CONFIG_FILE" \
    || fail "bootstrap audit state is not separated from completion marker"

grep -Fq \
    'DAIA_FIRSTBOOT_COMPLETE="${DAIA_STATE_DIR}/firstboot-complete"' \
    "$TEST_CONFIG_FILE" \
    || fail "first-boot completion marker is not configured"

if grep -Fq \
    'DAIA_FIRSTBOOT_STATE="${DAIA_STATE_DIR}/firstboot-complete"' \
    "$TEST_CONFIG_FILE"
then
    fail "bootstrap audit state aliases the completion marker"
fi

if [[ ! -f "$TEST_MODULES_FILE" ]]
then
    fail "enabled-module configuration does not exist"
fi

if [[ "$(grep -Ev '^[[:space:]]*(#|$)' "$TEST_MODULES_FILE")" != "desktop" ]]
then
    fail "desktop is not the configured first-boot module"
fi

printf 'PASS: first-boot service contract\n'
