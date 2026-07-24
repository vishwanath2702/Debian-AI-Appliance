#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "$SCRIPT_DIR/../../.." && pwd)"

source "$SCRIPT_DIR/fixtures/happy-path.sh"

source "$PROJECT_ROOT/installer/files/opt/daia/planner/registry-adapter.sh"
source "$PROJECT_ROOT/installer/files/opt/daia/planner/profile-reader.sh"
source "$PROJECT_ROOT/installer/files/opt/daia/planner/capability-resolver.sh"
source "$PROJECT_ROOT/installer/files/opt/daia/planner/dependency-resolver.sh"
source "$PROJECT_ROOT/installer/files/opt/daia/planner/conflict-detector.sh"
source "$PROJECT_ROOT/installer/files/opt/daia/planner/execution-plan-builder.sh"
source "$PROJECT_ROOT/installer/files/opt/daia/planner/planner.sh"

expected="$(
    printf '%s\n' \
        filesystem/base \
        package-manager/apt \
        desktop/xfce
)"

actual="$(daia_planner_build_execution_plan workstation)"

if [[ "$actual" != "$expected" ]]; then
    printf 'FAIL: unexpected execution plan\n' >&2
    printf 'Expected:\n%s\n' "$expected" >&2
    printf 'Actual:\n%s\n' "$actual" >&2
    exit 1
fi

printf 'PASS: Planner happy-path integration test\n'
