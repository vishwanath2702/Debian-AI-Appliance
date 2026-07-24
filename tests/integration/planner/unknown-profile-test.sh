#!/usr/bin/env bash

set -u

source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/common.sh"
source "$SCRIPT_DIR/fixtures/unknown-profile.sh"

daia_test_load_planner

output="$(
    daia_planner_build_execution_plan workstation 2>&1
)"
status=$?

if [[ "$status" -eq 0 ]]; then
    printf 'FAIL: planner accepted an unknown profile\n' >&2
    exit 1
fi

if [[ "$output" != *"unknown profile"* ]]; then
    printf 'FAIL: unexpected error message\n' >&2
    printf '%s\n' "$output" >&2
    exit 1
fi

printf 'PASS: Unknown profile integration test\n'
