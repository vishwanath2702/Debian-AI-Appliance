#!/usr/bin/env bash
#
# DAIA Planner
#
# Public entry point for the Planner subsystem.
#

if [[ -n "${__DAIA_PLANNER_MODULE_INITIALIZED:-}" ]]; then
    # shellcheck disable=SC2317
    return 0 2>/dev/null || exit 0
fi

readonly __DAIA_PLANNER_MODULE_INITIALIZED=1

###############################################################################
# Load Planner modules
###############################################################################

_daia_planner_directory="$(
    cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd
)" || exit 1

# The path is resolved dynamically relative to this module.
# shellcheck source=/dev/null
source "$_daia_planner_directory/execution-plan-builder.sh"

###############################################################################
# Internal helpers
###############################################################################

_daia_planner_error() {
    printf 'DAIA Planner: %s\n' "$*" >&2
}

_daia_planner_validate_argument_count() {
    local expected="$1"
    local function_name="$2"
    local actual="$3"

    if ((actual != expected)); then
        _daia_planner_error \
            "$function_name expects $expected argument(s); received $actual"
        return 1
    fi
}

_daia_planner_require_api() {
    if ! declare -F daia_execution_plan_builder_build >/dev/null; then
        _daia_planner_error \
            "Execution Plan Builder API is unavailable"
        return 1
    fi
}

###############################################################################
# Public API
###############################################################################

daia_planner_build_execution_plan() {
    _daia_planner_validate_argument_count \
        1 \
        "daia_planner_build_execution_plan" \
        "$#" || return 1

    _daia_planner_require_api || return 1

    daia_execution_plan_builder_build "$1"
}
