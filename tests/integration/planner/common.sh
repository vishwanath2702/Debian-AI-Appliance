#!/usr/bin/env bash

set -u

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "$SCRIPT_DIR/../../.." && pwd)"

daia_test_load_planner() {
    source "$PROJECT_ROOT/installer/files/opt/daia/planner/registry-adapter.sh"
    source "$PROJECT_ROOT/installer/files/opt/daia/planner/profile-reader.sh"
    source "$PROJECT_ROOT/installer/files/opt/daia/planner/capability-resolver.sh"
    source "$PROJECT_ROOT/installer/files/opt/daia/planner/dependency-resolver.sh"
    source "$PROJECT_ROOT/installer/files/opt/daia/planner/conflict-detector.sh"
    source "$PROJECT_ROOT/installer/files/opt/daia/planner/execution-plan-builder.sh"
    source "$PROJECT_ROOT/installer/files/opt/daia/planner/planner.sh"
}
