#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : build/generate-payload-report.sh
# Purpose    : Generate a human-readable DAIA payload report.
#
# Version    : 1.0.0
# Codename   : Pragna
# License    : GPL-3.0
#
# Responsibilities
# ----------------
# - Load and validate the DAIA build configuration.
# - Validate the payload workspace and inventory.
# - Read payload statistics from the inventory.
# - Count files and bytes by payload category.
# - Inspect configured source artifacts.
# - Record pending and available components.
# - Write the report atomically.
#
# Dependencies
# ------------
# - Bash 4 or later.
# - build/load-config.sh
# - build/variables.sh
# - runtime/lib/logging.sh
# - runtime/lib/filesystem.sh
# - runtime/lib/validation.sh
# - awk
# - sha256sum
# - mktemp
# - mv
#
# Inputs
# ------
# - work/payload/
# - work/reports/payload-inventory.tsv
# - payload/config/pragna.conf
#
# Outputs
# -------
# - work/reports/payload-report.txt
#
# Failure Modes
# -------------
# The script exits non-zero when:
# - the payload workspace is missing;
# - the payload inventory is missing or invalid;
# - a required external command is unavailable;
# - the report cannot be written.
#
# Reproducibility
# ---------------
# The report includes a generation timestamp and is therefore
# intended for human inspection rather than deterministic
# checksum comparison. The inventory remains the canonical,
# deterministic payload record.
#
# Future Extension
# ----------------
# - Add inventory freshness verification.
# - Add package names and versions.
# - Add model metadata.
# - Add component license summaries.
# - Add release-readiness assessment.
# - Add machine-readable JSON output.
#
# Usage
# -----
# Build the payload and inventory first:
#
#   ./build/build-payload.sh
#   ./build/generate-payload-inventory.sh
#
# Then generate the report:
#
#   ./build/generate-payload-report.sh
#
# ==========================================================

set -euo pipefail

############################################################
#
# Sections
#
#   1. Environment
#   2. Paths and State
#   3. Dependency Validation
#   4. Input Validation
#   5. Inventory Helpers
#   6. Source Readiness Helpers
#   7. Report Generation
#   8. Summary
#   9. Main
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

# Load shared DAIA libraries.
#
# shellcheck disable=SC1091
source "$PROJECT_ROOT/runtime/lib/logging.sh"

# shellcheck disable=SC1091
source "$PROJECT_ROOT/runtime/lib/filesystem.sh"

# shellcheck disable=SC1091
source "$PROJECT_ROOT/runtime/lib/validation.sh"

############################################################
# 2. Paths and State
############################################################

PAYLOAD_WORKSPACE="$WORK_DIR/payload"
PAYLOAD_ROOT="$PAYLOAD_WORKSPACE/daia/opt/daia"

REPORTS_DIRECTORY="$WORK_DIR/reports"
PAYLOAD_INVENTORY="$REPORTS_DIRECTORY/payload-inventory.tsv"
PAYLOAD_REPORT="$REPORTS_DIRECTORY/payload-report.txt"

BUILD_INFO_FILE="$PAYLOAD_ROOT/BUILD-INFO"

EXPECTED_INVENTORY_HEADER=$'path\tsize_bytes\tmode\tsha256'

total_files=0
total_bytes=0

runtime_files=0
runtime_bytes=0

package_files=0
package_bytes=0

image_files=0
image_bytes=0

model_files=0
model_bytes=0

branding_files=0
branding_bytes=0

metadata_files=0
metadata_bytes=0

other_files=0
other_bytes=0

available_components=0
pending_components=0
disabled_components=0

############################################################
# 3. Dependency Validation
############################################################

############################################################
# require_command
#
# Verify that an external command required by this script is
# available on the build host.
#
# Arguments:
#   $1 - Command name
#
# Returns:
#   0 when available.
#   Exits otherwise.
############################################################
require_command()
{
    local command_name="$1"

    if ! validate_command "$command_name"
    then
        log_error "Required command is unavailable: $command_name"
        exit 1
    fi
}

############################################################
# validate_dependencies
#
# Validate every external command used by report generation.
#
# Arguments:
#   None
#
# Returns:
#   0 when all commands are available.
############################################################
validate_dependencies()
{
    log_section "Validating report dependencies"

    require_command awk
    require_command sha256sum
    require_command mktemp
    require_command mv
    require_command date
    require_command du
    require_command find

    log_success "Report dependencies are available."
}

############################################################
# 4. Input Validation
############################################################

############################################################
# validate_inputs
#
# Validate the payload workspace, build metadata, and inventory
# before report generation begins.
#
# Arguments:
#   None
#
# Returns:
#   0 when valid.
#   Exits otherwise.
############################################################
validate_inputs()
{
    local inventory_header

    log_section "Validating report inputs"

    if ! validate_directory "$PAYLOAD_WORKSPACE"
    then
        log_error "Payload workspace is missing:"
        log_error "  $PAYLOAD_WORKSPACE"
        log_error "Run ./build/build-payload.sh first."
        exit 1
    fi

    if ! validate_nonempty_file "$BUILD_INFO_FILE"
    then
        log_error "Payload build metadata is missing or empty:"
        log_error "  $BUILD_INFO_FILE"
        exit 1
    fi

    if ! validate_nonempty_file "$PAYLOAD_INVENTORY"
    then
        log_error "Payload inventory is missing or empty:"
        log_error "  $PAYLOAD_INVENTORY"
        log_error "Run ./build/generate-payload-inventory.sh first."
        exit 1
    fi

    inventory_header="$(
        head -n 1 "$PAYLOAD_INVENTORY"
    )"

    if [[ "$inventory_header" != "$EXPECTED_INVENTORY_HEADER" ]]
    then
        log_error "Payload inventory has an unexpected format."
        log_error "Expected header:"
        log_error "  $EXPECTED_INVENTORY_HEADER"
        exit 1
    fi

    log_success "Payload report inputs are valid."
}

############################################################
# 5. Inventory Helpers
############################################################

############################################################
# classify_inventory_path
#
# Classify one inventory path into a report category.
#
# Arguments:
#   $1 - Relative inventory path
#
# Returns:
#   Category name on standard output.
############################################################
classify_inventory_path()
{
    local inventory_path="$1"

    case "$inventory_path" in
        daia/opt/daia/payload/packages/*)
            printf 'packages\n'
            ;;

        daia/opt/daia/payload/images/*)
            printf 'images\n'
            ;;

        daia/opt/daia/payload/models/*)
            printf 'models\n'
            ;;

        daia/opt/daia/payload/branding/*)
            printf 'branding\n'
            ;;

        daia/opt/daia/BUILD-INFO)
            printf 'metadata\n'
            ;;

        daia/opt/daia/config/*|\
        daia/opt/daia/lib/*|\
        daia/opt/daia/modules/*|\
        daia/opt/daia/services/*|\
        daia/opt/daia/bootstrap.sh)
            printf 'runtime\n'
            ;;

        *)
            printf 'other\n'
            ;;
    esac
}

############################################################
# accumulate_inventory_record
#
# Add one inventory record to the aggregate report totals.
#
# Arguments:
#   $1 - Relative file path
#   $2 - File size in bytes
#
# Returns:
#   0
############################################################
accumulate_inventory_record()
{
    local inventory_path="$1"
    local file_size_bytes="$2"
    local category

    category="$(
        classify_inventory_path "$inventory_path"
    )"

    total_files=$((total_files + 1))
    total_bytes=$((total_bytes + file_size_bytes))

    case "$category" in
        runtime)
            runtime_files=$((runtime_files + 1))
            runtime_bytes=$((runtime_bytes + file_size_bytes))
            ;;

        packages)
            package_files=$((package_files + 1))
            package_bytes=$((package_bytes + file_size_bytes))
            ;;

        images)
            image_files=$((image_files + 1))
            image_bytes=$((image_bytes + file_size_bytes))
            ;;

        models)
            model_files=$((model_files + 1))
            model_bytes=$((model_bytes + file_size_bytes))
            ;;

        branding)
            branding_files=$((branding_files + 1))
            branding_bytes=$((branding_bytes + file_size_bytes))
            ;;

        metadata)
            metadata_files=$((metadata_files + 1))
            metadata_bytes=$((metadata_bytes + file_size_bytes))
            ;;

        *)
            other_files=$((other_files + 1))
            other_bytes=$((other_bytes + file_size_bytes))
            ;;
    esac
}

############################################################
# read_inventory
#
# Read every data row in the inventory and calculate aggregate
# category statistics.
#
# Arguments:
#   None
#
# Returns:
#   0 on success.
############################################################
read_inventory()
{
    local inventory_path
    local file_size_bytes
    local file_mode
    local file_checksum

    log_section "Reading payload inventory"

    while IFS=$'\t' read -r \
        inventory_path \
        file_size_bytes \
        file_mode \
        file_checksum
    do
        if [[ "$inventory_path" == "path" ]]
        then
            continue
        fi

        # These values are read to validate the complete TSV
        # record even though the report aggregates only path
        # and size information.
        : "$file_mode"
        : "$file_checksum"

        accumulate_inventory_record \
            "$inventory_path" \
            "$file_size_bytes"
    done <"$PAYLOAD_INVENTORY"

    log_success "Payload inventory statistics calculated."
}

############################################################
# human_readable_bytes
#
# Convert a byte count to a human-readable IEC value.
#
# Arguments:
#   $1 - Byte count
#
# Returns:
#   Human-readable size on standard output.
############################################################
human_readable_bytes()
{
    local byte_count="$1"

    if validate_command numfmt
    then
        numfmt \
            --to=iec-i \
            --suffix=B \
            "$byte_count"
    else
        printf '%s bytes\n' "$byte_count"
    fi
}

############################################################
# calculate_inventory_checksum
#
# Calculate the SHA-256 checksum of the canonical inventory.
#
# Arguments:
#   None
#
# Returns:
#   SHA-256 checksum on standard output.
############################################################
calculate_inventory_checksum()
{
    sha256sum "$PAYLOAD_INVENTORY" |
        awk '{print $1}'
}

############################################################
# calculate_workspace_size
#
# Return the human-readable on-disk size of the workspace.
#
# Arguments:
#   None
#
# Returns:
#   Human-readable size on standard output.
############################################################
calculate_workspace_size()
{
    du -sh "$PAYLOAD_WORKSPACE" |
        awk '{print $1}'
}

############################################################
# 6. Source Readiness Helpers
############################################################

############################################################
# directory_contains_payload_files
#
# Determine whether a component directory contains at least
# one regular file other than a Git placeholder.
#
# Arguments:
#   $1 - Directory path
#
# Returns:
#   0 when content exists.
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
# component_directory_status
#
# Determine the readiness state of a directory-based payload
# component.
#
# Arguments:
#   $1 - Enabled value
#   $2 - Source directory
#
# Returns:
#   AVAILABLE, PENDING, or DISABLED on standard output.
############################################################
component_directory_status()
{
    local enabled_value="$1"
    local source_directory="$2"

    if [[ "$enabled_value" != "true" ]]
    then
        printf 'DISABLED\n'
        return 0
    fi

    if directory_contains_payload_files "$source_directory"
    then
        printf 'AVAILABLE\n'
    else
        printf 'PENDING\n'
    fi
}

############################################################
# component_file_status
#
# Determine the readiness state of a file-based payload
# component.
#
# Arguments:
#   $1 - Enabled value
#   $2 - Source file
#
# Returns:
#   AVAILABLE, PENDING, or DISABLED on standard output.
############################################################
component_file_status()
{
    local enabled_value="$1"
    local source_file="$2"

    if [[ "$enabled_value" != "true" ]]
    then
        printf 'DISABLED\n'
        return 0
    fi

    if validate_nonempty_file "$source_file"
    then
        printf 'AVAILABLE\n'
    else
        printf 'PENDING\n'
    fi
}

############################################################
# record_component_status
#
# Update aggregate readiness counters.
#
# Arguments:
#   $1 - Component status
#
# Returns:
#   0
############################################################
record_component_status()
{
    local component_status="$1"

    case "$component_status" in
        AVAILABLE)
            available_components=$((available_components + 1))
            ;;

        DISABLED)
            disabled_components=$((disabled_components + 1))
            ;;

        *)
            pending_components=$((pending_components + 1))
            ;;
    esac
}

############################################################
# print_component_line
#
# Write one component readiness line into the report.
#
# Arguments:
#   $1 - Output file
#   $2 - Component description
#   $3 - Component status
#
# Returns:
#   0
############################################################
print_component_line()
{
    local output_file="$1"
    local component_description="$2"
    local component_status="$3"

    printf '%-28s : %s\n' \
        "$component_description" \
        "$component_status" \
        >>"$output_file"

    record_component_status "$component_status"
}

############################################################
# 7. Report Generation
############################################################

############################################################
# write_report
#
# Generate the complete human-readable payload report and move
# it atomically into its final location.
#
# Arguments:
#   None
#
# Returns:
#   0 on success.
############################################################
write_report()
{
    local temporary_report
    local generated_timestamp
    local inventory_checksum
    local workspace_size

    local runtime_size
    local package_size
    local image_size
    local model_size
    local branding_size
    local metadata_size
    local other_size
    local total_size

    local docker_status
    local ollama_status
    local dependencies_status
    local open_webui_status
    local model_status
    local wallpaper_status
    local icon_status

    log_section "Generating payload report"

    ensure_directory "$REPORTS_DIRECTORY"

    temporary_report="$(
        mktemp "$REPORTS_DIRECTORY/.payload-report.XXXXXX"
    )"

    trap 'rm -f "$temporary_report"' EXIT

    generated_timestamp="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
    inventory_checksum="$(calculate_inventory_checksum)"
    workspace_size="$(calculate_workspace_size)"

    runtime_size="$(human_readable_bytes "$runtime_bytes")"
    package_size="$(human_readable_bytes "$package_bytes")"
    image_size="$(human_readable_bytes "$image_bytes")"
    model_size="$(human_readable_bytes "$model_bytes")"
    branding_size="$(human_readable_bytes "$branding_bytes")"
    metadata_size="$(human_readable_bytes "$metadata_bytes")"
    other_size="$(human_readable_bytes "$other_bytes")"
    total_size="$(human_readable_bytes "$total_bytes")"

    docker_status="$(
        component_directory_status \
            "$DAIA_ENABLE_DOCKER" \
            "$DAIA_DOCKER_SOURCE_ABS"
    )"

    ollama_status="$(
        component_directory_status \
            "$DAIA_ENABLE_OLLAMA" \
            "$DAIA_OLLAMA_SOURCE_ABS"
    )"

    dependencies_status="$(
        component_directory_status \
            "$DAIA_OFFLINE_INSTALL" \
            "$DAIA_DEPENDENCIES_SOURCE_ABS"
    )"

    open_webui_status="$(
        component_file_status \
            "$DAIA_ENABLE_OPEN_WEBUI" \
            "$DAIA_OPEN_WEBUI_IMAGE_ABS"
    )"

    model_status="$(
        component_directory_status \
            "$DAIA_ENABLE_DEFAULT_MODEL" \
            "$DAIA_DEFAULT_MODEL_SOURCE_ABS"
    )"

    wallpaper_status="$(
        component_file_status \
            "$DAIA_BRANDING_ENABLED" \
            "$DAIA_DEFAULT_WALLPAPER_ABS"
    )"

    icon_status="$(
        component_file_status \
            "$DAIA_BRANDING_ENABLED" \
            "$DAIA_ASSISTANT_ICON_ABS"
    )"

    {
        printf '%s\n' \
            '============================================================'
        printf '%s\n' \
            ' DAIA Payload Report'
        printf '%s\n' \
            '============================================================'
        printf '\n'

        printf '%s\n' 'Distribution'
        printf '%s\n' '------------------------------------------------------------'
        printf '%-28s : %s\n' "Name" "$DAIA_NAME"
        printf '%-28s : %s\n' "Full name" "$DAIA_FULL_NAME"
        printf '%-28s : %s\n' "Version" "$DAIA_VERSION"
        printf '%-28s : %s\n' "Codename" "$DAIA_CODENAME"
        printf '%-28s : %s\n' "Edition" "$DAIA_EDITION"
        printf '%-28s : %s\n' "Architecture" "$DAIA_ARCHITECTURE"
        printf '%-28s : %s %s\n' \
            "Base distribution" \
            "$DAIA_BASE_DISTRIBUTION" \
            "$DAIA_BASE_VERSION"
        printf '%-28s : %s\n' \
            "Desktop environment" \
            "$DAIA_DESKTOP_ENVIRONMENT"
        printf '%-28s : %s\n' \
            "Generated" \
            "$generated_timestamp"
        printf '\n'

        printf '%s\n' 'Build Inputs'
        printf '%s\n' '------------------------------------------------------------'
        printf '%-28s : %s\n' \
            "Configuration file" \
            "$DAIA_CONFIG_FILE"
        printf '%-28s : %s\n' \
            "Payload workspace" \
            "$PAYLOAD_WORKSPACE"
        printf '%-28s : %s\n' \
            "Inventory file" \
            "$PAYLOAD_INVENTORY"
        printf '%-28s : %s\n' \
            "Inventory SHA-256" \
            "$inventory_checksum"
        printf '\n'

        printf '%s\n' 'Payload Summary'
        printf '%s\n' '------------------------------------------------------------'
        printf '%-28s : %s\n' "Files" "$total_files"
        printf '%-28s : %s\n' "Inventory bytes" "$total_bytes"
        printf '%-28s : %s\n' "Inventory size" "$total_size"
        printf '%-28s : %s\n' "Workspace disk usage" "$workspace_size"
        printf '\n'

        printf '%s\n' 'Payload Categories'
        printf '%s\n' '------------------------------------------------------------'
        printf '%-18s %12s %18s\n' \
            "Category" \
            "Files" \
            "Size"
        printf '%-18s %12s %18s\n' \
            "Runtime" \
            "$runtime_files" \
            "$runtime_size"
        printf '%-18s %12s %18s\n' \
            "Packages" \
            "$package_files" \
            "$package_size"
        printf '%-18s %12s %18s\n' \
            "Images" \
            "$image_files" \
            "$image_size"
        printf '%-18s %12s %18s\n' \
            "Models" \
            "$model_files" \
            "$model_size"
        printf '%-18s %12s %18s\n' \
            "Branding" \
            "$branding_files" \
            "$branding_size"
        printf '%-18s %12s %18s\n' \
            "Metadata" \
            "$metadata_files" \
            "$metadata_size"
        printf '%-18s %12s %18s\n' \
            "Other" \
            "$other_files" \
            "$other_size"
        printf '\n'

        printf '%s\n' 'Configured Component Readiness'
        printf '%s\n' '------------------------------------------------------------'
    } >"$temporary_report"

    print_component_line \
        "$temporary_report" \
        "Docker packages" \
        "$docker_status"

    print_component_line \
        "$temporary_report" \
        "Ollama runtime" \
        "$ollama_status"

    print_component_line \
        "$temporary_report" \
        "Package dependencies" \
        "$dependencies_status"

    print_component_line \
        "$temporary_report" \
        "Open WebUI image" \
        "$open_webui_status"

    print_component_line \
        "$temporary_report" \
        "Default AI model" \
        "$model_status"

    print_component_line \
        "$temporary_report" \
        "Default wallpaper" \
        "$wallpaper_status"

    print_component_line \
        "$temporary_report" \
        "Assistant icon" \
        "$icon_status"

    {
        printf '\n'
        printf '%s\n' 'Readiness Summary'
        printf '%s\n' '------------------------------------------------------------'
        printf '%-28s : %s\n' \
            "Available components" \
            "$available_components"
        printf '%-28s : %s\n' \
            "Pending components" \
            "$pending_components"
        printf '%-28s : %s\n' \
            "Disabled components" \
            "$disabled_components"
        printf '\n'

        printf '%s\n' 'Overall Status'
        printf '%s\n' '------------------------------------------------------------'

        if (( pending_components > 0 ))
        then
            printf '%s\n' \
                'DEVELOPMENT PAYLOAD — configured artifacts remain pending.'
        else
            printf '%s\n' \
                'COMPLETE PAYLOAD — all configured artifacts are available.'
        fi

        printf '\n'
        printf '%s\n' \
            'The payload inventory is the canonical integrity record.'
    } >>"$temporary_report"

    chmod 0644 "$temporary_report"
    mv "$temporary_report" "$PAYLOAD_REPORT"

    trap - EXIT

    log_success "Payload report generated successfully."
}

############################################################
# 8. Summary
############################################################

############################################################
# display_summary
#
# Display the generated report path and final readiness state.
#
# Arguments:
#   None
#
# Returns:
#   0
############################################################
display_summary()
{
    log_header "DAIA Payload Report Summary"

    printf 'Report               : %s\n' "$PAYLOAD_REPORT"
    printf 'Payload files        : %s\n' "$total_files"
    printf 'Available components : %s\n' "$available_components"
    printf 'Pending components   : %s\n' "$pending_components"
    printf 'Disabled components  : %s\n' "$disabled_components"

    echo

    if (( pending_components > 0 ))
    then
        log_warning \
            "Payload report generated with pending component artifacts."
    else
        log_success \
            "Payload report confirms all configured artifacts are available."
    fi
}

############################################################
# 9. Main
############################################################

main()
{
    log_header "DAIA Payload Report Generator"

    log_info \
        "Generating report for $DAIA_NAME $DAIA_VERSION $DAIA_CODENAME."

    validate_dependencies
    validate_inputs
    read_inventory
    write_report
    display_summary
}

main "$@"

############################################################
# End of File
############################################################
