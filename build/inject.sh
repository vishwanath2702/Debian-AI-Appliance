#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : build/inject.sh
# Purpose    : Inject the DAIA installer files and assembled
#              payload workspace into the extracted ISO.
#
# Version    : 1.1.0
# Codename   : Pragna
# License    : GPL-3.0
#
# Responsibilities
# ----------------
# - Load the project build environment and DAIA configuration.
# - Validate the extracted Debian ISO workspace.
# - Validate the installer configuration and hooks.
# - Validate the generated DAIA payload workspace.
# - Remove any previously injected DAIA content.
# - Copy the Debian Preseed configuration.
# - Copy the established installer runtime.
# - Overlay the assembled payload workspace.
# - Copy installer hooks.
# - Apply executable permissions.
# - Verify all required files after injection.
#
# Transitional Architecture
# -------------------------
# DAIA currently has two runtime sources:
#
#   installer/files/
#       Contains the proven installation runtime, including
#       install.sh, bootstrap.sh, and the first-boot service.
#
#   work/payload/daia/
#       Contains the newly assembled distribution payload,
#       runtime libraries, modules, packages, images, models,
#       branding, and BUILD-INFO metadata.
#
# The installer runtime is copied first. The payload workspace
# is then overlaid on top of it. This preserves the working
# installer while allowing the new distribution architecture
# to enter the ISO safely.
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
# - installer/preseed.cfg
# - installer/hooks/
# - installer/files/
# - work/payload/daia/
# - work/extract/
#
# Outputs
# -------
# - work/extract/preseed.cfg
# - work/extract/installer/hooks/
# - work/extract/daia/
#
# Failure Modes
# -------------
# The script exits non-zero when:
# - the extracted ISO directory is missing;
# - the payload workspace is missing;
# - the Preseed configuration is missing;
# - installer hooks are missing;
# - the established installer runtime is missing;
# - a required copied file cannot be verified.
#
# Future Migration
# ----------------
# Once install.sh, bootstrap.sh, systemd services, and all
# installer runtime files have moved into runtime/ and are
# assembled entirely by build-payload.sh, the direct copy from
# installer/files/ can be removed in a dedicated migration
# task.
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

PRESEED_SOURCE="$INSTALLER_DIR/preseed.cfg"
INSTALLER_FILES_SOURCE="$INSTALLER_DIR/files"
INSTALLER_HOOKS_SOURCE="$INSTALLER_DIR/hooks"

PAYLOAD_WORKSPACE="$WORK_DIR/payload"
PAYLOAD_DAIA_SOURCE="$PAYLOAD_WORKSPACE/daia"
PAYLOAD_BUILD_INFO="$PAYLOAD_DAIA_SOURCE/opt/daia/BUILD-INFO"

ISO_PRESEED_TARGET="$EXTRACT_DIR/preseed.cfg"
ISO_INSTALLER_TARGET="$EXTRACT_DIR/installer"
ISO_HOOKS_TARGET="$ISO_INSTALLER_TARGET/hooks"
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

    require_nonempty_file \
        "Debian Preseed configuration" \
        "$PRESEED_SOURCE"

    require_directory \
        "Established installer runtime" \
        "$INSTALLER_FILES_SOURCE"

    require_directory \
        "Installer hooks" \
        "$INSTALLER_HOOKS_SOURCE"

    require_nonempty_file \
        "Late-install hook" \
        "$INSTALLER_HOOKS_SOURCE/late-install.sh"

    require_directory \
        "Generated payload workspace" \
        "$PAYLOAD_DAIA_SOURCE"

    require_nonempty_file \
        "Payload build metadata" \
        "$PAYLOAD_BUILD_INFO"

    require_nonempty_file \
        "Installer entry point" \
        "$INSTALLER_FILES_SOURCE/opt/daia/install.sh"

    require_nonempty_file \
        "First-boot bootstrap" \
        "$INSTALLER_FILES_SOURCE/opt/daia/bootstrap.sh"

    require_nonempty_file \
        "First-boot systemd service" \
        "$INSTALLER_FILES_SOURCE/etc/systemd/system/daia-firstboot.service"

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
    remove_path "$ISO_INSTALLER_TARGET"
    remove_path "$ISO_PRESEED_TARGET"

    log_success "Previous injected DAIA content removed."
}

############################################################
# 5. Installer Configuration Injection
############################################################

############################################################
# inject_preseed_configuration
#
# Copy the Debian installer automation configuration into the
# root of the extracted ISO.
#
# Arguments:
#   None
#
# Returns:
#   0 on success.
############################################################
inject_preseed_configuration()
{
    log_section "Injecting Debian installer configuration"

    copy_file \
        "$PRESEED_SOURCE" \
        "$ISO_PRESEED_TARGET" \
        0644

    log_success "Preseed configuration injected."
}

############################################################
# 6. Runtime and Payload Injection
############################################################

############################################################
# inject_established_runtime
#
# Copy the currently proven installer runtime into the ISO.
#
# This remains the base layer until all installation runtime
# files have been migrated to the new runtime architecture.
#
# Arguments:
#   None
#
# Returns:
#   0 on success.
############################################################
inject_established_runtime()
{
    log_section "Injecting established DAIA installer runtime"

    copy_directory \
        "$INSTALLER_FILES_SOURCE" \
        "$ISO_DAIA_TARGET"

    log_success "Established installer runtime injected."
}

############################################################
# overlay_payload_workspace
#
# Overlay the assembled DAIA distribution workspace onto the
# established runtime already copied into the ISO.
#
# rsync is used for overlay semantics because copy_directory
# intentionally replaces its destination.
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
# 7. Installer Hook Injection
############################################################

############################################################
# inject_installer_hooks
#
# Copy the installer-time DAIA hooks into the extracted ISO.
#
# Arguments:
#   None
#
# Returns:
#   0 on success.
############################################################
inject_installer_hooks()
{
    log_section "Injecting installer hooks"

    ensure_directory "$ISO_HOOKS_TARGET"

    rsync \
        --archive \
        --exclude='.gitkeep' \
        "$INSTALLER_HOOKS_SOURCE/" \
        "$ISO_HOOKS_TARGET/"

    log_success "Installer hooks injected."
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
        "$ISO_HOOKS_TARGET/late-install.sh" \
        "$ISO_DAIA_TARGET/opt/daia/install.sh" \
        "$ISO_DAIA_TARGET/opt/daia/bootstrap.sh"

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
        "Preseed configuration" \
        "$ISO_PRESEED_TARGET"

    verify_injected_file \
        "Late-install hook" \
        "$ISO_HOOKS_TARGET/late-install.sh"

    verify_injected_file \
        "DAIA installer entry point" \
        "$ISO_DAIA_TARGET/opt/daia/install.sh"

    verify_injected_file \
        "DAIA first-boot bootstrap" \
        "$ISO_DAIA_TARGET/opt/daia/bootstrap.sh"

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
    inject_preseed_configuration
    inject_established_runtime
    overlay_payload_workspace
    inject_installer_hooks
    apply_injected_permissions
    verify_injected_content
    display_summary
}

main "$@"

############################################################
# End of File
############################################################
