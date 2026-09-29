#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : build/inject.sh
# Purpose    : Inject the assembled DAIA payload workspace
#              into the extracted ISO.
#
# Version    : 1.2.0
# Codename   : Pragna
# License    : GPL-3.0
#
# Responsibilities
# ----------------
# - Load the project build environment and DAIA configuration.
# - Validate the extracted Debian ISO workspace.
# - Validate the generated DAIA payload workspace.
# - Remove any previously injected DAIA content.
# - Copy the assembled payload workspace into the ISO.
# - Apply executable permissions.
# - Verify all required files after injection.
#
# Architecture
# ------------
# work/payload/daia/ is the canonical DAIA distribution
# payload assembled by build/build-payload.sh.
#
# The Rust installer consumes the resulting ISO payload from
# /run/live/medium/daia and deploys it into the installed target.
#
# Dependencies
# ------------
# - Bash 4 or later.
# - build/variables.sh
# - build/load-config.sh
# - runtime/lib/logging.sh
# - runtime/lib/filesystem.sh
# - runtime/lib/validation.sh
# - A completed ISO extraction workspace.
# - A completed payload workspace.
#
# Inputs
# ------
# - work/payload/daia/
# - work/extract/
#
# Outputs
# -------
# - work/extract/daia/
#
# Failure Modes
# -------------
# The script exits non-zero when:
# - the extracted ISO directory is missing;
# - the payload workspace is missing;
# - required payload content is missing;
# - a required copied file cannot be verified.
#
# Usage
# -----
# Run after extraction, patching, and payload assembly:
#
#   ./build/build-payload.sh
#   ./build/generate-payload-inventory.sh
#   ./build/generate-payload-report.sh
#   ./build/inject.sh
#
# ==========================================================

set -euo pipefail

############################################################
#
# Sections
#
#   1. Environment
#   2. Source and Destination Paths
#   3. Input Validation
#   4. Injection Cleanup
#   5. Installer Configuration Injection
#   6. Runtime and Payload Injection
#   7. Installer Hook Injection
#   8. Permission Management
#   9. Post-Injection Verification
#  10. Summary
#  11. Main
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

# Load the established project directory variables.
#
# shellcheck disable=SC1091
source "$SCRIPT_DIR/variables.sh"

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
# 2. Source and Destination Paths
############################################################

PAYLOAD_WORKSPACE="$WORK_DIR/payload"
PAYLOAD_DAIA_SOURCE="$PAYLOAD_WORKSPACE/daia"
PAYLOAD_BUILD_INFO="$PAYLOAD_DAIA_SOURCE/opt/daia/BUILD-INFO"

ISO_DAIA_TARGET="$EXTRACT_DIR/daia"

injected_files_verified=0

############################################################
# 3. Input Validation
############################################################

############################################################
# require_directory
#
# Verify that a required source directory exists.
#
# Arguments:
#   $1 - Description
#   $2 - Directory path
#
# Returns:
#   0 when present.
#   Exits otherwise.
############################################################
require_directory()
{
    local description="$1"
    local directory_path="$2"

    if ! validate_directory "$directory_path"
    then
        log_error "$description is missing:"
        log_error "  $directory_path"
        exit 1
    fi

    log_success "$description is available."
}

############################################################
# require_nonempty_file
#
# Verify that a required source file exists and is not empty.
#
# Arguments:
#   $1 - Description
#   $2 - File path
#
# Returns:
#   0 when valid.
#   Exits otherwise.
############################################################
require_nonempty_file()
{
    local description="$1"
    local file_path="$2"

    if ! validate_nonempty_file "$file_path"
    then
        log_error "$description is missing or empty:"
        log_error "  $file_path"
        exit 1
    fi

    log_success "$description is available."
}

############################################################
# validate_injection_inputs
#
# Validate every input required before modifying the extracted
# ISO workspace.
#
# Arguments:
#   None
#
# Returns:
#   0 when all inputs are valid.
############################################################
validate_injection_inputs()
{
    log_section "Validating injection inputs"

    require_directory \
        "Extracted Debian ISO workspace" \
        "$EXTRACT_DIR"

    require_directory \
        "Generated payload workspace" \
        "$PAYLOAD_DAIA_SOURCE"

    require_nonempty_file \
        "Payload build metadata" \
        "$PAYLOAD_BUILD_INFO"

    log_success "All injection inputs are valid."
}

############################################################
# 4. Injection Cleanup
############################################################

############################################################
# clean_previous_injection
#
# Remove DAIA files from a previous injection without touching
# the extracted Debian base filesystem.
#
# Arguments:
#   None
#
# Returns:
#   0 on success.
############################################################
clean_previous_injection()
{
    log_section "Cleaning previous DAIA injection"

    remove_path "$ISO_DAIA_TARGET"

    # Remove artifacts produced by the retired Debian Installer
    # integration so they cannot survive from an older workspace.
    remove_path "$EXTRACT_DIR/preseed.cfg"
    remove_path "$EXTRACT_DIR/installer"

    log_success "Previous injected DAIA content removed."
}

############################################################
# 6. Runtime and Payload Injection
############################################################

############################################################
# overlay_payload_workspace
#
# Copy the assembled DAIA distribution workspace into the
# extracted ISO.
#
# rsync preserves the assembled payload hierarchy and file
# metadata while excluding source-control placeholder files.
#
# Arguments:
#   None
#
# Returns:
#   0 on success.
############################################################
overlay_payload_workspace()
{
    log_section "Overlaying assembled DAIA payload"

    if ! validate_command rsync
    then
        log_error "Required command is unavailable: rsync"
        exit 1
    fi

    rsync \
        --archive \
        --exclude='.gitkeep' \
        "$PAYLOAD_DAIA_SOURCE/" \
        "$ISO_DAIA_TARGET/"

    log_success "Assembled payload workspace overlaid successfully."
}

############################################################
# 8. Permission Management
############################################################

############################################################
# apply_injected_permissions
#
# Apply executable permissions to scripts that must run during
# installation or first boot.
#
# Arguments:
#   None
#
# Returns:
#   0 on success.
############################################################
apply_injected_permissions()
{
    log_section "Applying injected file permissions"

    chmod 0755 \
        "$ISO_DAIA_TARGET/opt/daia/runtime.sh" \
        "$ISO_DAIA_TARGET/opt/daia/firstboot.sh"

    if [[ -d "$ISO_DAIA_TARGET/opt/daia/modules" ]]
    then
        find "$ISO_DAIA_TARGET/opt/daia/modules" \
            -type f \
            -name '*.sh' \
            -exec chmod 0755 {} +
    fi

    find "$ISO_DAIA_TARGET/opt/daia/lib" \
        -type f \
        -name '*.sh' \
        -exec chmod 0644 {} +

    log_success "Injected file permissions applied."
}

############################################################
# 9. Post-Injection Verification
############################################################

############################################################
# verify_injected_file
#
# Verify one required file after injection.
#
# Arguments:
#   $1 - Description
#   $2 - File path
#
# Returns:
#   0 when valid.
#   Exits otherwise.
############################################################
verify_injected_file()
{
    local description="$1"
    local file_path="$2"

    if ! validate_nonempty_file "$file_path"
    then
        log_error "Required injected file is missing or empty:"
        log_error "  $description"
        log_error "  $file_path"
        exit 1
    fi

    injected_files_verified=$((injected_files_verified + 1))

    log_success "$description verified."
}

############################################################
# verify_injected_content
#
# Verify the established installer files and the newly staged
# payload metadata after overlaying both sources.
#
# Arguments:
#   None
#
# Returns:
#   0 when all required content is present.
############################################################
verify_injected_content()
{
    log_section "Verifying injected DAIA content"

    verify_injected_file \
        "DAIA runtime orchestrator" \
        "$ISO_DAIA_TARGET/opt/daia/runtime.sh"

    verify_injected_file \
        "DAIA first-boot orchestrator" \
        "$ISO_DAIA_TARGET/opt/daia/firstboot.sh"

    verify_injected_file \
        "DAIA first-boot service" \
        "$ISO_DAIA_TARGET/etc/systemd/system/daia-firstboot.service"

    verify_injected_file \
        "DAIA payload build metadata" \
        "$ISO_DAIA_TARGET/opt/daia/BUILD-INFO"

    verify_injected_file \
        "DAIA runtime logging library" \
        "$ISO_DAIA_TARGET/opt/daia/lib/logging.sh"

    verify_injected_file \
        "DAIA runtime validation library" \
        "$ISO_DAIA_TARGET/opt/daia/lib/validation.sh"

    log_success "All required injected content verified."
}

############################################################
# 10. Summary
############################################################

############################################################
# display_summary
#
# Display the final injection result and destination paths.
#
# Arguments:
#   None
#
# Returns:
#   0
############################################################
display_summary()
{
    log_header "DAIA Injection Summary"

    printf 'Distribution        : %s %s\n' \
        "$DAIA_NAME" \
        "$DAIA_VERSION"

    printf 'Codename            : %s\n' "$DAIA_CODENAME"
    printf 'Extracted ISO       : %s\n' "$EXTRACT_DIR"
    printf 'DAIA ISO directory  : %s\n' "$ISO_DAIA_TARGET"
    printf 'Verified files      : %s\n' "$injected_files_verified"

    echo
    log_success "DAIA project files injected successfully."
}

############################################################
# 11. Main
############################################################

main()
{
    log_header "DAIA ISO Payload Injection"

    log_info \
        "Injecting $DAIA_NAME $DAIA_VERSION $DAIA_CODENAME into the ISO workspace."

    validate_injection_inputs
    clean_previous_injection
    overlay_payload_workspace
    apply_injected_permissions
    verify_injected_content
    display_summary
}

main "$@"

############################################################
# End of File
############################################################
