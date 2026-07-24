#!/usr/bin/env bash

set -u

source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/common.sh"
source "$SCRIPT_DIR/fixtures/dependency-chain.sh"

daia_test_load_planner

output="$(
    daia_planner_build_execution_plan workstation 2>&1
)"
status=$?

if [[ "$status" -ne 0 ]]; then
    printf 'FAIL: planner rejected a valid dependency chain\n' >&2
    printf '%s\n' "$output" >&2
    exit 1
fi

for plugin in \
    'system/graphics-stack' \
    'desktop/window-manager' \
    'desktop/environment'
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

if [[ -z "$graphics_line" || -z "$window_line" || -z "$desktop_line" ]]; then
    printf 'FAIL: could not determine plugin ordering\n' >&2
    printf '%s\n' "$output" >&2
    exit 1
fi

if ! (( graphics_line < window_line && window_line < desktop_line )); then
    printf 'FAIL: plugins are not in dependency order\n' >&2
    printf 'Expected: graphics-plugin -> window-plugin -> desktop-plugin\n' >&2
    printf '%s\n' "$output" >&2
    exit 1
fi

printf 'PASS: Dependency chain integration test\n'
