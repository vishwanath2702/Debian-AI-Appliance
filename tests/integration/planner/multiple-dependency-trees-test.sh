#!/usr/bin/env bash

set -u

source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/common.sh"
source "$SCRIPT_DIR/fixtures/multiple-dependency-trees.sh"

daia_test_load_planner

output="$(
    daia_planner_build_execution_plan workstation 2>&1
)"
status=$?

if [[ "$status" -ne 0 ]]; then
    printf 'FAIL: planner rejected valid dependency trees\n' >&2
    printf '%s\n' "$output" >&2
    exit 1
fi

for plugin in \
    'system/graphics-stack' \
    'desktop/window-manager' \
    'desktop/environment' \
    'system/network-stack' \
    'system/network-manager'
do
    if [[ "$output" != *"$plugin"* ]]; then
        printf 'FAIL: execution plan is missing plugin: %s\n' "$plugin" >&2
        printf '%s\n' "$output" >&2
        exit 1
    fi
done

graphics_line="$(
    printf '%s\n' "$output" |
        grep -n -m1 'system/graphics-stack' |
        cut -d: -f1
)"

window_line="$(
    printf '%s\n' "$output" |
        grep -n -m1 'desktop/window-manager' |
        cut -d: -f1
)"

desktop_line="$(
    printf '%s\n' "$output" |
        grep -n -m1 'desktop/environment' |
        cut -d: -f1
)"

network_stack_line="$(
    printf '%s\n' "$output" |
        grep -n -m1 'system/network-stack' |
        cut -d: -f1
)"

network_manager_line="$(
    printf '%s\n' "$output" |
        grep -n -m1 'system/network-manager' |
        cut -d: -f1
)"

if ! (( graphics_line < window_line && window_line < desktop_line )); then
    printf 'FAIL: desktop dependency tree is out of order\n' >&2
    printf '%s\n' "$output" >&2
    exit 1
fi

if ! (( network_stack_line < network_manager_line )); then
    printf 'FAIL: network dependency tree is out of order\n' >&2
    printf '%s\n' "$output" >&2
    exit 1
fi

printf 'PASS: Multiple dependency trees integration test\n'
