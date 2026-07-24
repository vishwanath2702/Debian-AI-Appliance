#!/usr/bin/env bash
#
# Build a validated DAIA plugin execution plan from a profile.
#

if [[ -n "${DAIA_EXECUTION_PLAN_BUILDER_LOADED:-}" ]]; then
    return 0
fi

readonly DAIA_EXECUTION_PLAN_BUILDER_LOADED=1

###############################################################################
# Internal helpers
###############################################################################

_daia_execution_plan_builder_validate_argument_count() {
    local expected_count="$1"
    local function_name="$2"
    local actual_count="$3"

    if ((actual_count != expected_count)); then
        printf '%s: expected %d argument(s), received %d\n' \
            "$function_name" \
            "$expected_count" \
            "$actual_count" >&2
        return 1
    fi
}

_daia_execution_plan_builder_require_function() {
    local function_name="$1"

    if ! declare -F "$function_name" >/dev/null; then
        printf 'daia_execution_plan_builder_build: required API is unavailable: %s\n' \
            "$function_name" >&2
        return 1
    fi
}

_daia_execution_plan_builder_validate_dependencies() {
    local required_function

    for required_function in \
        daia_profile_reader_read \
        daia_capability_resolver_resolve_many \
        daia_dependency_resolver_resolve_many \
        daia_conflict_detector_check
    do
        _daia_execution_plan_builder_require_function \
            "$required_function" || return 1
    done
}

###############################################################################
# Public API
###############################################################################

daia_execution_plan_builder_build() {
    _daia_execution_plan_builder_validate_argument_count \
        1 \
        "daia_execution_plan_builder_build" \
        "$#" || return 1

    _daia_execution_plan_builder_validate_dependencies || return 1

    local profile_id="$1"
    local capability_output
    local provider_output
    local plan_output

    local -a capabilities=()
    local -a providers=()
    local -a execution_plan=()

    capability_output="$(
        daia_profile_reader_read "$profile_id"
    )" || return 1

    # An empty profile is valid and produces an empty execution plan.
    if [[ -z "$capability_output" ]]; then
        return 0
    fi

    mapfile -t capabilities <<<"$capability_output"

    provider_output="$(
        daia_capability_resolver_resolve_many "${capabilities[@]}"
    )" || return 1

    # Capabilities may theoretically resolve to no plugins.
    if [[ -z "$provider_output" ]]; then
        return 0
    fi

    mapfile -t providers <<<"$provider_output"

    plan_output="$(
        daia_dependency_resolver_resolve_many "${providers[@]}"
    )" || return 1

    if [[ -z "$plan_output" ]]; then
        return 0
    fi

    mapfile -t execution_plan <<<"$plan_output"

    daia_conflict_detector_check "${execution_plan[@]}" || return 1

    printf '%s\n' "${execution_plan[@]}"
}
