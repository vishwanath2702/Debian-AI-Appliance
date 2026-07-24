#!/usr/bin/env bash

set -u

source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/common.sh"
source "$SCRIPT_DIR/fixtures/empty-dependencies.sh"

daia_test_load_planner

output="$(
    daia_planner_build_execution_plan workstation 2>&1
)"
status=$?

if [[ "$status" -ne 0 ]]; then
    printf 'FAIL: planner rejected a valid plugin with no dependencies\n' >&2
    printf '%s\n' "$output" >&2
    exit 1
fi

if [[ "$output" != *'desktop/environment'* ]]; then
    printf 'FAIL: execution plan is missing plugin: desktop/environment\n' >&2
    printf '%s\n' "$output" >&2
    exit 1
fi

for plugin in \
    'desktop/window-manager' \
    'system/graphics-stack' \
    'system/network-manager' \
    'system/network-stack' \
    'legacy/display'
do
    if [[ "$output" == *"$plugin"* ]]; then
        printf 'FAIL: unexpected plugin in execution plan: %s\n' "$plugin" >&2
        printf '%s\n' "$output" >&2
        exit 1
    fi
done

plugin_count="$(
    printf '%s\n' "$output" |
        grep -Ec '^[a-z0-9]+/[a-z0-9._-]+$'
)"

if [[ "$plugin_count" -ne 1 ]]; then
    printf 'FAIL: expected exactly one plugin in execution plan\n' >&2
    printf 'Found %s plugin(s)\n' "$plugin_count" >&2
    printf '%s\n' "$output" >&2
    exit 1
fi

printf 'PASS: Empty dependencies integration test\n'
