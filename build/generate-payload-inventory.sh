#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : build/generate-payload-inventory.sh
# Purpose    : Generate an inventory of the staged payload.
#
# Version    : 1.0.0
# Codename   : Pragna
# License    : GPL-3.0
#
# Responsibilities
# ----------------
# - Load the validated DAIA build configuration.
# - Verify that the payload workspace exists.
# - Enumerate all regular files in deterministic order.
# - Record path, size, mode and SHA-256 checksum.
# - Write the inventory atomically.
# - Display a concise generation summary.
#
# Dependencies
# ------------
# - Bash 4 or later.
# - build/load-config.sh
# - build/variables.sh
# - runtime/lib/logging.sh
# - runtime/lib/filesystem.sh
# - runtime/lib/validation.sh
# - find
# - sort
# - stat
# - sha256sum
# - mktemp
#
# Inputs
# ------
# - work/payload/
#
# Outputs
# -------
# - work/reports/payload-inventory.tsv
#
# Inventory Format
# ----------------
# The output is tab-separated and contains these columns:
#
#   path
#   size_bytes
#   mode
#   sha256
#
# Paths are relative to work/payload.
#
# Failure Modes
# -------------
# The script exits non-zero when:
# - the payload workspace is missing;
# - a required command is unavailable;
# - the reports directory cannot be created;
# - file metadata or checksums cannot be calculated;
# - the inventory cannot be written.
#
# Reproducibility
# ---------------
# Files are sorted by relative path using the C locale.
# The inventory contains no generation timestamp, ensuring
# identical payload contents produce identical inventories.
#
# Future Extension
# ----------------
# - Add symbolic-link inventory support.
# - Add file ownership metadata.
# - Add detached inventory signatures.
# - Add comparison and verification tools.
#
# Usage
# -----
# Build the payload workspace first:
#
#   ./build/build-payload.sh
#
# Then generate its inventory:
#
#   ./build/generate-payload-inventory.sh
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
#   4. Workspace Validation
#   5. Inventory Helpers
#   6. Inventory Generation
#   7. Summary
#   8. Main
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

# Load the validated build configuration and shared paths.
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
REPORTS_DIRECTORY="$WORK_DIR/reports"

INVENTORY_FILE="$REPORTS_DIRECTORY/payload-inventory.tsv"

inventory_file_count=0
inventory_total_bytes=0

############################################################
# 3. Dependency Validation
############################################################

############################################################
# require_command
#
# Verify that a command required by inventory generation is
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
# Validate all external commands used by this script.
#
# Arguments:
#   None
#
# Returns:
#   0 when all dependencies are available.
############################################################
validate_dependencies()
{
    log_section "Validating inventory dependencies"

    require_command find
    require_command sort
    require_command stat
    require_command sha256sum
    require_command mktemp
    require_command mv

    log_success "Inventory dependencies are available."
}

############################################################
# 4. Workspace Validation
############################################################

############################################################
# validate_payload_workspace
#
# Confirm that the payload workspace exists and contains at
# least one regular file.
#
# Arguments:
#   None
#
# Returns:
#   0 when valid.
#   Exits otherwise.
############################################################
validate_payload_workspace()
{
    log_section "Validating payload workspace"

    if ! validate_directory "$PAYLOAD_WORKSPACE"
    then
        log_error "Payload workspace does not exist:"
        log_error "  $PAYLOAD_WORKSPACE"
        log_error "Run ./build/build-payload.sh first."
        exit 1
    fi

    if ! find "$PAYLOAD_WORKSPACE" \
        -type f \
        -print \
        -quit |
        grep -q .
    then
        log_error "Payload workspace contains no regular files:"
        log_error "  $PAYLOAD_WORKSPACE"
        exit 1
    fi

    log_success "Payload workspace is ready for inventory."
}

############################################################
# 5. Inventory Helpers
############################################################

############################################################
# relative_payload_path
#
# Convert an absolute workspace file path into a path relative
# to the payload workspace.
#
# Arguments:
#   $1 - Absolute file path
#
# Returns:
#   Relative path on standard output.
############################################################
relative_payload_path()
{
    local absolute_file_path="$1"

    printf '%s\n' \
        "${absolute_file_path#"$PAYLOAD_WORKSPACE"/}"
}

############################################################
# calculate_file_checksum
#
# Calculate the SHA-256 checksum of one regular file.
#
# Arguments:
#   $1 - File path
#
# Returns:
#   SHA-256 checksum on standard output.
############################################################
calculate_file_checksum()
{
    local file_path="$1"

    sha256sum "$file_path" |
        awk '{print $1}'
}

############################################################
# write_inventory_header
#
# Write the inventory column names.
#
# Arguments:
#   $1 - Output file
#
# Returns:
#   0 on success.
############################################################
write_inventory_header()
{
    local output_file="$1"

    printf 'path\tsize_bytes\tmode\tsha256\n' \
        >"$output_file"
}

############################################################
# append_inventory_record
#
# Calculate metadata for one file and append it to the
# inventory.
#
# Arguments:
#   $1 - Absolute file path
#   $2 - Output inventory file
#
# Returns:
#   0 on success.
############################################################
append_inventory_record()
{
    local absolute_file_path="$1"
    local output_file="$2"

    local relative_file_path
    local file_size_bytes
    local file_mode
    local file_checksum

    relative_file_path="$(
        relative_payload_path "$absolute_file_path"
    )"

    file_size_bytes="$(
        stat --format='%s' "$absolute_file_path"
    )"

    file_mode="$(
        stat --format='%a' "$absolute_file_path"
    )"

    file_checksum="$(
        calculate_file_checksum "$absolute_file_path"
    )"

    printf '%s\t%s\t%s\t%s\n' \
        "$relative_file_path" \
        "$file_size_bytes" \
        "$file_mode" \
        "$file_checksum" \
        >>"$output_file"

    inventory_file_count=$((inventory_file_count + 1))
    inventory_total_bytes=$((inventory_total_bytes + file_size_bytes))
}

############################################################
# 6. Inventory Generation
############################################################

############################################################
# generate_inventory
#
# Generate a complete, sorted payload inventory.
#
# The inventory is first written to a temporary file and then
# atomically moved into place. This prevents a failed run from
# leaving a partially written inventory.
#
# Arguments:
#   None
#
# Returns:
#   0 on success.
############################################################
generate_inventory()
{
    local temporary_inventory
    local absolute_file_path

    log_section "Generating payload inventory"

    ensure_directory "$REPORTS_DIRECTORY"

    temporary_inventory="$(
        mktemp "$REPORTS_DIRECTORY/.payload-inventory.XXXXXX"
    )"

    # Remove the temporary file if the script exits before the
    # final atomic move completes.
    trap 'rm -f "$temporary_inventory"' EXIT

    write_inventory_header "$temporary_inventory"

    while IFS= read -r -d '' absolute_file_path
    do
        append_inventory_record \
            "$absolute_file_path" \
            "$temporary_inventory"
    done < <(
        LC_ALL=C find "$PAYLOAD_WORKSPACE" \
            -type f \
            -print0 |
            LC_ALL=C sort -z
    )

    chmod 0644 "$temporary_inventory"
    mv "$temporary_inventory" "$INVENTORY_FILE"

    # The temporary path no longer exists after the move.
    trap - EXIT

    log_success "Payload inventory generated successfully."
}

############################################################
# 7. Summary
############################################################

############################################################
# human_readable_total_size
#
# Convert the accumulated byte count into a human-readable
# value using numfmt when available. Fall back to bytes when
# numfmt is unavailable.
#
# Arguments:
#   None
#
# Returns:
#   Human-readable size on standard output.
############################################################
human_readable_total_size()
{
    if validate_command numfmt
    then
        numfmt \
            --to=iec-i \
            --suffix=B \
            "$inventory_total_bytes"
    else
        printf '%s bytes\n' "$inventory_total_bytes"
    fi
}

############################################################
# display_summary
#
# Display the inventory output and aggregate statistics.
#
# Arguments:
#   None
#
# Returns:
#   0
############################################################
display_summary()
{
    local readable_size

    readable_size="$(
        human_readable_total_size
    )"

    log_header "DAIA Payload Inventory Summary"

    printf 'Distribution : %s %s\n' \
        "$DAIA_NAME" \
        "$DAIA_VERSION"

    printf 'Codename     : %s\n' "$DAIA_CODENAME"
    printf 'Workspace    : %s\n' "$PAYLOAD_WORKSPACE"
    printf 'Inventory    : %s\n' "$INVENTORY_FILE"
    printf 'Files        : %s\n' "$inventory_file_count"
    printf 'Total bytes  : %s\n' "$inventory_total_bytes"
    printf 'Total size   : %s\n' "$readable_size"

    echo
    log_success "Payload inventory is ready."
}

############################################################
# 8. Main
############################################################

main()
{
    log_header "DAIA Payload Inventory Generator"

    log_info \
        "Inventorying payload for $DAIA_NAME $DAIA_VERSION $DAIA_CODENAME."

    validate_dependencies
    validate_payload_workspace
    generate_inventory
    display_summary
}

main "$@"

############################################################
# End of File
############################################################
