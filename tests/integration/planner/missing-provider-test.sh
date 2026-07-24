#!/usr/bin/env bash

set -u

source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/common.sh"
source "$SCRIPT_DIR/fixtures/missing-provider.sh"

daia_test_load_planner

output="$(
    daia_planner_build_execution_plan workstation 2>&1
)"
status=$?

if [[ "$status" -eq 0 ]]; then
    printf 'FAIL: planner accepted a capability with no provider\n' >&2
    printf '%s\n' "$output" >&2
    exit 1
fi

if [[ "$output" != *"provider"* ]]; then
    printf 'FAIL: unexpected error message\n' >&2
    printf '%s\n' "$output" >&2
    exit 1
fi

printf 'PASS: Missing provider integration test\n'
