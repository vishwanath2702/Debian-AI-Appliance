#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : build/status.sh
# Purpose    : Display the current health and readiness of the
#              DAIA build environment.
#
# Version    : 1.0.0
# Codename   : Pragna
# License    : GPL-3.0
#
# Responsibilities
# ----------------
# - Load the selected DAIA build configuration.
# - Validate the repository structure.
# - Inspect the payload source directories.
# - Inspect generated workspaces and reports.
# - Inspect the Debian source ISO and generated DAIA ISO.
# - Present a concise, human-readable status summary.
# - Return a non-zero exit status when blocking problems exist.
#
# Dependencies
# ------------
# - Bash 4 or later.
# - build/load-config.sh
# - build/variables.sh
# - runtime/lib/validation.sh
# - find
# - sha256sum
# - stat
#
# Inputs
# ------
# - Repository files and directories.
# - payload/config/pragna.conf
# - work/payload/
# - work/reports/payload-inventory.tsv
# - iso/
# - output/
#
# Outputs
# -------
# - A status report written to standard output.
#
# Exit Codes
# ----------
#   0 - No blocking problems were detected.
#   1 - One or more blocking problems were detected.
#
# Status Meanings
# ---------------
# READY
#   The required item exists and appears valid.
#
# PENDING
#   The item is expected in a later milestone or has not yet
#   been generated. It does not currently block development.
#
# MISSING
#   A required item is absent and blocks the relevant build
#   operation.
#
# INVALID
#   The item exists but does not satisfy basic validation.
#
# Failure Modes
# -------------
# The script exits non-zero when:
# - the DAIA configuration cannot be loaded;
# - a required repository directory is missing;
# - a required build script or runtime library is missing;
# - the Debian source ISO is missing;
# - an existing inventory or ISO is invalid.
#
# Future Extension
# ----------------
# - Add JSON output for CI systems.
# - Add quiet and verbose modes.
# - Add report freshness checks.
# - Add payload inventory verification.
# - Add ISO content verification.
# - Add release-readiness mode.
#
# Usage
# -----
# Run from any directory:
#
#   ./build/status.sh
#
# Select another configuration:
#
#   DAIA_CONFIG_FILE=/path/to/custom.conf \
#       ./build/status.sh
#
# ==========================================================

set -euo pipefail

############################################################
#
# Sections
#
#   1. Environment
#   2. Paths and State
#   3. Output Functions
#   4. General Validation Helpers
#   5. Project Checks
#   6. Configuration Checks
#   7. Runtime Checks
#   8. Payload Source Checks
#   9. Workspace Checks
#  10. Inventory Checks
#  11. ISO Checks
#  12. Summary
#  13. Main
#
############################################################

############################################################
# 1. Environment
############################################################

SCRIPT_DIR="$(
    cd "$(dirname "${BASH_SOURCE[0]}")" &&
    pwd
)"

PROJECT_ROOT="$(
    cd "$SCRIPT_DIR/.." &&
    pwd
)"

# Load and validate the selected DAIA build configuration.
#
# shellcheck disable=SC1091
source "$SCRIPT_DIR/load-config.sh"

# Load reusable validation helpers.
#
# shellcheck disable=SC1091
source "$PROJECT_ROOT/runtime/lib/validation.sh"

############################################################
# 2. Paths and State
############################################################

PAYLOAD_SOURCE_DIR="$PROJECT_ROOT/payload"
RUNTIME_SOURCE_DIR="$PROJECT_ROOT/runtime"
INSTALLER_SOURCE_DIR="$PROJECT_ROOT/installer"

PAYLOAD_WORKSPACE="$WORK_DIR/payload"
PAYLOAD_ROOT="$PAYLOAD_WORKSPACE/daia/opt/daia"

REPORTS_DIRECTORY="$WORK_DIR/reports"
PAYLOAD_INVENTORY="$REPORTS_DIRECTORY/payload-inventory.tsv"
PAYLOAD_REPORT="$REPORTS_DIRECTORY/payload-report.txt"

BUILD_INFO_FILE="$PAYLOAD_ROOT/BUILD-INFO"

blocking_failures=0
ready_items=0
pending_items=0
missing_items=0
invalid_items=0

############################################################
# 3. Output Functions
############################################################

############################################################
# print_header
#
# Print the main DAIA status heading.
#
# Arguments:
#   $1 - Heading text
#
# Returns:
#   0
############################################################
print_header()
{
    local heading="$1"

    printf '\n'
    printf '%s\n' '============================================================'
    printf ' %s\n' "$heading"
    printf '%s\n' '============================================================'
}

############################################################
# print_section
#
# Print a status report section heading.
#
# Arguments:
#   $1 - Section text
#
# Returns:
#   0
############################################################
print_section()
{
    local section_name="$1"

    printf '\n'
    printf '%s\n' "$section_name"
    printf '%s\n' '------------------------------------------------------------'
}

############################################################
# print_status
#
# Print one aligned status line.
#
# Arguments:
#   $1 - Status label
#   $2 - Item description
#   $3 - Optional detail
#
# Returns:
#   0
############################################################
print_status()
{
    local status_label="$1"
    local item_description="$2"
    local item_detail="${3:-}"

    if [[ -n "$item_detail" ]]
    then
        printf '[%-7s] %-30s %s\n' \
            "$status_label" \
            "$item_description" \
            "$item_detail"
    else
        printf '[%-7s] %s\n' \
            "$status_label" \
            "$item_description"
    fi
}

############################################################
# mark_ready
#
# Record and display a ready item.
#
# Arguments:
#   $1 - Item description
#   $2 - Optional detail
#
# Returns:
#   0
############################################################
mark_ready()
{
    local item_description="$1"
    local item_detail="${2:-}"

    ready_items=$((ready_items + 1))
    print_status "READY" "$item_description" "$item_detail"
}

############################################################
# mark_pending
#
# Record and display a pending, non-blocking item.
#
# Arguments:
#   $1 - Item description
#   $2 - Optional detail
#
# Returns:
#   0
############################################################
mark_pending()
{
    local item_description="$1"
    local item_detail="${2:-}"

    pending_items=$((pending_items + 1))
    print_status "PENDING" "$item_description" "$item_detail"
}

############################################################
# mark_missing
#
# Record and display a missing, blocking item.
#
# Arguments:
#   $1 - Item description
#   $2 - Optional detail
#
# Returns:
#   0
############################################################
mark_missing()
{
    local item_description="$1"
    local item_detail="${2:-}"

    missing_items=$((missing_items + 1))
    blocking_failures=$((blocking_failures + 1))

    print_status "MISSING" "$item_description" "$item_detail"
}

############################################################
# mark_invalid
#
# Record and display an invalid, blocking item.
#
# Arguments:
#   $1 - Item description
#   $2 - Optional detail
#
# Returns:
#   0
############################################################
mark_invalid()
{
    local item_description="$1"
    local item_detail="${2:-}"

    invalid_items=$((invalid_items + 1))
    blocking_failures=$((blocking_failures + 1))

    print_status "INVALID" "$item_description" "$item_detail"
}

############################################################
# 4. General Validation Helpers
############################################################

############################################################
# directory_contains_payload_files
#
# Determine whether a directory contains at least one regular
# file other than a Git placeholder.
#
# Arguments:
#   $1 - Directory path
#
# Returns:
#   0 when a payload file exists.
#   1 otherwise.
############################################################
directory_contains_payload_files()
{
    local directory_path="$1"

    if [[ ! -d "$directory_path" ]]
    then
        return 1
    fi

    find "$directory_path" \
        -type f \
        ! -name '.gitkeep' \
        -print \
        -quit |
        grep -q .
}

############################################################
# count_regular_files
#
# Count regular files beneath a directory.
#
# Arguments:
#   $1 - Directory path
#
# Returns:
#   File count on standard output.
############################################################
count_regular_files()
{
    local directory_path="$1"

    if [[ ! -d "$directory_path" ]]
    then
        printf '0\n'
        return 0
    fi

    find "$directory_path" \
        -type f \
        ! -name '.gitkeep' \
        -print |
        wc -l
}

############################################################
# calculate_human_size
#
# Return the human-readable size of a file or directory.
#
# Arguments:
#   $1 - File or directory path
#
# Returns:
#   Human-readable size on standard output.
############################################################
calculate_human_size()
{
    local target_path="$1"

    if [[ ! -e "$target_path" ]]
    then
        printf '0B\n'
        return 0
    fi

    du -sh "$target_path" |
        awk '{print $1}'
}

############################################################
# check_required_directory
#
# Check a required project directory.
#
# Arguments:
#   $1 - Description
#   $2 - Directory path
#
# Returns:
#   0
############################################################
check_required_directory()
{
    local description="$1"
    local directory_path="$2"

    if validate_directory "$directory_path"
    then
        mark_ready "$description" "$directory_path"
    else
        mark_missing "$description" "$directory_path"
    fi
}

############################################################
# check_required_file
#
# Check a required project file.
#
# Arguments:
#   $1 - Description
#   $2 - File path
#
# Returns:
#   0
############################################################
check_required_file()
{
    local description="$1"
    local file_path="$2"

    if validate_nonempty_file "$file_path"
    then
        mark_ready "$description" "$file_path"
    elif validate_file "$file_path"
    then
        mark_invalid "$description" "File is empty: $file_path"
    else
        mark_missing "$description" "$file_path"
    fi
}

############################################################
# check_optional_directory_payload
#
# Check whether an optional component directory currently
# contains usable payload files.
#
# Arguments:
#   $1 - Description
#   $2 - Directory path
#   $3 - Enabled value
#
# Returns:
#   0
############################################################
check_optional_directory_payload()
{
    local description="$1"
    local directory_path="$2"
    local enabled_value="$3"
    local file_count

    if [[ "$enabled_value" != "true" ]]
    then
        mark_pending "$description" "Disabled by configuration"
        return 0
    fi

    if directory_contains_payload_files "$directory_path"
    then
        file_count="$(count_regular_files "$directory_path")"
        mark_ready "$description" "$file_count file(s)"
    elif [[ -d "$directory_path" ]]
    then
        mark_pending "$description" "Directory exists but is empty"
    else
        mark_pending "$description" "Source directory not created"
    fi
}

############################################################
# check_optional_file_payload
#
# Check whether an optional component file exists and contains
# data.
#
# Arguments:
#   $1 - Description
#   $2 - File path
#   $3 - Enabled value
#
# Returns:
#   0
############################################################
check_optional_file_payload()
{
    local description="$1"
    local file_path="$2"
    local enabled_value="$3"
    local file_size

    if [[ "$enabled_value" != "true" ]]
    then
        mark_pending "$description" "Disabled by configuration"
        return 0
    fi

    if validate_nonempty_file "$file_path"
    then
        file_size="$(calculate_human_size "$file_path")"
        mark_ready "$description" "$file_size"
    elif validate_file "$file_path"
    then
        mark_pending "$description" "File exists but is empty"
    else
        mark_pending "$description" "Artifact not added"
    fi
}

############################################################
# 5. Project Checks
############################################################

############################################################
# check_project_structure
#
# Validate the repository directories required by the current
# DAIA development architecture.
#
# Arguments:
#   None
#
# Returns:
#   0
############################################################
check_project_structure()
{
    print_section "Project Structure"

    check_required_directory \
        "Build directory" \
        "$PROJECT_ROOT/build"

    check_required_directory \
        "Installer directory" \
        "$INSTALLER_SOURCE_DIR"

    check_required_directory \
        "Payload directory" \
        "$PAYLOAD_SOURCE_DIR"

    check_required_directory \
        "Runtime directory" \
        "$RUNTIME_SOURCE_DIR"

    check_required_directory \
        "Documentation directory" \
        "$PROJECT_ROOT/docs"
}

############################################################
# check_build_components
#
# Validate the primary build utilities currently expected in
# the repository.
#
# Arguments:
#   None
#
# Returns:
#   0
############################################################
check_build_components()
{
    print_section "Build Components"

    check_required_file \
        "Build environment" \
        "$SCRIPT_DIR/variables.sh"

    check_required_file \
        "Configuration loader" \
        "$SCRIPT_DIR/load-config.sh"

    check_required_file \
        "Payload workspace builder" \
        "$SCRIPT_DIR/build-payload.sh"

    check_required_file \
        "Payload inventory generator" \
        "$SCRIPT_DIR/generate-payload-inventory.sh"

    check_required_file \
        "Main build pipeline" \
        "$SCRIPT_DIR/build.sh"

    check_required_file \
        "ISO rebuild utility" \
        "$SCRIPT_DIR/rebuild.sh"
}

############################################################
# 6. Configuration Checks
############################################################

############################################################
# check_configuration
#
# Display the active DAIA build identity and confirm that the
# selected configuration file is valid.
#
# Arguments:
#   None
#
# Returns:
#   0
############################################################
check_configuration()
{
    print_section "Configuration"

    mark_ready \
        "Configuration file" \
        "$DAIA_CONFIG_FILE"

    mark_ready \
        "Distribution" \
        "$DAIA_NAME $DAIA_VERSION"

    mark_ready \
        "Codename" \
        "$DAIA_CODENAME"

    mark_ready \
        "Edition" \
        "$DAIA_EDITION"

    mark_ready \
        "Architecture" \
        "$DAIA_ARCHITECTURE"

    mark_ready \
        "Desktop" \
        "$DAIA_DESKTOP_ENVIRONMENT"

    if [[ "$DAIA_OFFLINE_INSTALL" == "true" ]]
    then
        mark_ready "Offline installation" "Enabled"
    else
        mark_pending "Offline installation" "Disabled"
    fi
}

############################################################
# 7. Runtime Checks
############################################################

############################################################
# check_runtime
#
# Validate the reusable runtime library and module structure.
#
# Arguments:
#   None
#
# Returns:
#   0
############################################################
check_runtime()
{
    print_section "Runtime"

    check_required_directory \
        "Runtime libraries" \
        "$RUNTIME_SOURCE_DIR/lib"

    check_required_directory \
        "Runtime modules" \
        "$RUNTIME_SOURCE_DIR/modules"

    check_required_directory \
        "Runtime configuration" \
        "$RUNTIME_SOURCE_DIR/config"

    check_required_directory \
        "Runtime services" \
        "$RUNTIME_SOURCE_DIR/services"

    check_required_file \
        "Logging library" \
        "$RUNTIME_SOURCE_DIR/lib/logging.sh"

    check_required_file \
        "Common library" \
        "$RUNTIME_SOURCE_DIR/lib/common.sh"

    check_required_file \
        "Filesystem library" \
        "$RUNTIME_SOURCE_DIR/lib/filesystem.sh"

    check_required_file \
        "Validation library" \
        "$RUNTIME_SOURCE_DIR/lib/validation.sh"
}

############################################################
# 8. Payload Source Checks
############################################################

############################################################
# check_payload_sources
#
# Inspect the source artifacts configured for the DAIA
# distribution.
#
# Missing component artifacts are pending at this milestone,
# not blocking failures.
#
# Arguments:
#   None
#
# Returns:
#   0
############################################################
check_payload_sources()
{
    print_section "Payload Sources"

    check_optional_directory_payload \
        "Docker packages" \
        "$DAIA_DOCKER_SOURCE_ABS" \
        "$DAIA_ENABLE_DOCKER"

    check_optional_directory_payload \
        "Ollama runtime" \
        "$DAIA_OLLAMA_SOURCE_ABS" \
        "$DAIA_ENABLE_OLLAMA"

    check_optional_directory_payload \
        "Package dependencies" \
        "$DAIA_DEPENDENCIES_SOURCE_ABS" \
        "$DAIA_OFFLINE_INSTALL"

    check_optional_file_payload \
        "Open WebUI image" \
        "$DAIA_OPEN_WEBUI_IMAGE_ABS" \
        "$DAIA_ENABLE_OPEN_WEBUI"

    check_optional_directory_payload \
        "Default AI model" \
        "$DAIA_DEFAULT_MODEL_SOURCE_ABS" \
        "$DAIA_ENABLE_DEFAULT_MODEL"

    check_optional_file_payload \
        "Default wallpaper" \
        "$DAIA_DEFAULT_WALLPAPER_ABS" \
        "$DAIA_BRANDING_ENABLED"

    check_optional_file_payload \
        "Assistant icon" \
        "$DAIA_ASSISTANT_ICON_ABS" \
        "$DAIA_BRANDING_ENABLED"
}

############################################################
# 9. Workspace Checks
############################################################

############################################################
# check_payload_workspace
#
# Inspect the generated payload workspace.
#
# The workspace is considered pending when it has not yet been
# built. An existing workspace without BUILD-INFO is invalid.
#
# Arguments:
#   None
#
# Returns:
#   0
############################################################
check_payload_workspace()
{
    local workspace_file_count
    local workspace_size

    print_section "Generated Workspace"

    if [[ ! -d "$PAYLOAD_WORKSPACE" ]]
    then
        mark_pending \
            "Payload workspace" \
            "Run ./build/build-payload.sh"

        return 0
    fi

    workspace_file_count="$(
        count_regular_files "$PAYLOAD_WORKSPACE"
    )"

    workspace_size="$(
        calculate_human_size "$PAYLOAD_WORKSPACE"
    )"

    mark_ready \
        "Payload workspace" \
        "$workspace_file_count file(s), $workspace_size"

    if validate_nonempty_file "$BUILD_INFO_FILE"
    then
        mark_ready \
            "Payload build metadata" \
            "$BUILD_INFO_FILE"
    elif validate_file "$BUILD_INFO_FILE"
    then
        mark_invalid \
            "Payload build metadata" \
            "BUILD-INFO is empty"
    else
        mark_invalid \
            "Payload build metadata" \
            "BUILD-INFO is missing"
    fi
}

############################################################
# 10. Inventory Checks
############################################################

############################################################
# check_payload_inventory
#
# Inspect the generated payload inventory and verify its
# expected header.
#
# Arguments:
#   None
#
# Returns:
#   0
############################################################
check_payload_inventory()
{
    local inventory_header
    local inventory_entries
    local inventory_checksum

    print_section "Payload Inventory and Reports"

    if [[ ! -f "$PAYLOAD_INVENTORY" ]]
    then
        mark_pending \
            "Payload inventory" \
            "Run ./build/generate-payload-inventory.sh"
    elif [[ ! -s "$PAYLOAD_INVENTORY" ]]
    then
        mark_invalid \
            "Payload inventory" \
            "Inventory file is empty"
    else
        inventory_header="$(
            head -n 1 "$PAYLOAD_INVENTORY"
        )"

        if [[ "$inventory_header" != $'path\tsize_bytes\tmode\tsha256' ]]
        then
            mark_invalid \
                "Payload inventory" \
                "Unexpected inventory format"
        else
            inventory_entries="$(
                tail -n +2 "$PAYLOAD_INVENTORY" |
                wc -l
            )"

            inventory_checksum="$(
                sha256sum "$PAYLOAD_INVENTORY" |
                awk '{print $1}'
            )"

            mark_ready \
                "Payload inventory" \
                "$inventory_entries entries"

            mark_ready \
                "Inventory checksum" \
                "${inventory_checksum:0:16}..."
        fi
    fi

    if validate_nonempty_file "$PAYLOAD_REPORT"
    then
        mark_ready \
            "Payload report" \
            "$PAYLOAD_REPORT"
    elif validate_file "$PAYLOAD_REPORT"
    then
        mark_invalid \
            "Payload report" \
            "Report file is empty"
    else
        mark_pending \
            "Payload report" \
            "Not implemented yet"
    fi
}

############################################################
# 11. ISO Checks
############################################################

############################################################
# check_iso_files
#
# Validate the Debian source ISO and inspect the generated
# DAIA ISO when available.
#
# Arguments:
#   None
#
# Returns:
#   0
############################################################
check_iso_files()
{
    local source_iso_size
    local output_iso_size

    print_section "ISO Images"

    if validate_nonempty_file "$SOURCE_ISO"
    then
        source_iso_size="$(calculate_human_size "$SOURCE_ISO")"

        mark_ready \
            "Debian source ISO" \
            "$source_iso_size"
    elif validate_file "$SOURCE_ISO"
    then
        mark_invalid \
            "Debian source ISO" \
            "Source ISO is empty"
    else
        mark_missing \
            "Debian source ISO" \
            "$SOURCE_ISO"
    fi

    if validate_nonempty_file "$OUTPUT_ISO"
    then
        output_iso_size="$(calculate_human_size "$OUTPUT_ISO")"

        mark_ready \
            "Generated DAIA ISO" \
            "$output_iso_size"
    elif validate_file "$OUTPUT_ISO"
    then
        mark_invalid \
            "Generated DAIA ISO" \
            "Output ISO is empty"
    else
        mark_pending \
            "Generated DAIA ISO" \
            "Run ./build/build.sh"
    fi
}

############################################################
# 12. Summary
############################################################

############################################################
# display_summary
#
# Display aggregate status counts and the final result.
#
# Arguments:
#   None
#
# Returns:
#   0
############################################################
display_summary()
{
    print_header "DAIA Build Status Summary"

    printf 'Ready items       : %s\n' "$ready_items"
    printf 'Pending items     : %s\n' "$pending_items"
    printf 'Missing items     : %s\n' "$missing_items"
    printf 'Invalid items     : %s\n' "$invalid_items"
    printf 'Blocking failures : %s\n' "$blocking_failures"

    echo

    if (( blocking_failures > 0 ))
    then
        printf '%s\n' \
            "DAIA build environment has blocking problems."

        return 1
    fi

    if (( pending_items > 0 ))
    then
        printf '%s\n' \
            "DAIA foundation is healthy; some planned artifacts are pending."

        return 0
    fi

    printf '%s\n' \
        "DAIA build environment is fully ready."

    return 0
}

############################################################
# 13. Main
############################################################

main()
{
    print_header "DAIA Build Status"

    printf 'Project root : %s\n' "$PROJECT_ROOT"
    printf 'Configuration: %s\n' "$DAIA_CONFIG_FILE"

    check_project_structure
    check_build_components
    check_configuration
    check_runtime
    check_payload_sources
    check_payload_workspace
    check_payload_inventory
    check_iso_files

    display_summary
}

main "$@"

############################################################
# End of File
############################################################
