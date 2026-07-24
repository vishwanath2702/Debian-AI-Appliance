#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : build/build-payload.sh
# Purpose    : Assemble the DAIA payload workspace.
#
# Version    : 1.0.1
# Codename   : Pragna
# License    : GPL-3.0
#
# Responsibilities
# ----------------
# - Load and validate the selected DAIA build configuration.
# - Create a clean payload workspace under work/payload.
# - Stage DAIA runtime libraries, modules and services.
# - Stage available offline packages, images and models.
# - Stage available branding resources.
# - Generate build metadata for the assembled payload.
# - Display a concise payload build summary.
#
# Dependencies
# ------------
# - Bash 4 or later.
# - build/load-config.sh
# - build/variables.sh
# - runtime/lib/logging.sh
# - runtime/lib/filesystem.sh
# - runtime/lib/validation.sh
#
# Inputs
# ------
# - runtime/
# - payload/
# - payload/config/pragna.conf
#
# Outputs
# -------
# - work/payload/
# - work/payload/daia/opt/daia/BUILD-INFO
#
# Failure Modes
# -------------
# The script exits non-zero when:
# - the build configuration cannot be loaded;
# - the runtime source directory is missing;
# - the workspace cannot be created;
# - a required filesystem operation fails.
#
# Missing component artifacts are currently warnings because
# Docker, Ollama, Open WebUI, the model and branding will be
# populated during later milestones.
#
# Future Extension
# ----------------
# - Add strict release-mode validation.
# - Add component checksums.
# - Add payload inventory generation.
# - Add reproducible timestamp support.
# - Add payload signing.
#
# Usage
# -----
# Run from any directory:
#
#   ./build/build-payload.sh
#
# Select an alternative configuration:
#
#   DAIA_CONFIG_FILE=/path/to/custom.conf \
#       ./build/build-payload.sh
#
# ==========================================================

set -euo pipefail

############################################################
#
# Sections
#
#   1. Environment
#   2. Workspace Paths
#   3. General Helpers
#   4. Workspace Management
#   5. Runtime Staging
#   6. Component Staging
#   7. Branding Staging
#   8. Metadata Generation
#   9. Summary
#  10. Main
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

# Load and validate the selected build configuration.
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
# 2. Workspace Paths
############################################################

PAYLOAD_WORKSPACE="$WORK_DIR/payload"
DAIA_WORKSPACE="$PAYLOAD_WORKSPACE/daia"

DAIA_ROOT_TARGET="$DAIA_WORKSPACE/opt/daia"
DAIA_RUNTIME_SOURCE="$PROJECT_ROOT/runtime"

DAIA_CONFIG_TARGET="$DAIA_ROOT_TARGET/config"
DAIA_LIB_TARGET="$DAIA_ROOT_TARGET/lib"
DAIA_MODULES_TARGET="$DAIA_ROOT_TARGET/modules"
DAIA_SERVICES_TARGET="$DAIA_ROOT_TARGET/services"

DAIA_PAYLOAD_TARGET="$DAIA_ROOT_TARGET/payload"
DAIA_PACKAGES_TARGET="$DAIA_PAYLOAD_TARGET/packages"
DAIA_IMAGES_TARGET="$DAIA_PAYLOAD_TARGET/images"
DAIA_MODELS_TARGET="$DAIA_PAYLOAD_TARGET/models"
DAIA_BRANDING_TARGET="$DAIA_PAYLOAD_TARGET/branding"

BUILD_INFO_FILE="$DAIA_ROOT_TARGET/BUILD-INFO"

staged_components=0
skipped_components=0

############################################################
# 3. General Helpers
############################################################

############################################################
# stage_warning
#
# Record a component that could not be staged because its
# source artifact is not available.
#
# Arguments:
#   $1 - Warning message
#
# Returns:
#   0
############################################################
stage_warning()
{
    local message="$1"

    log_warning "$message"
    skipped_components=$((skipped_components + 1))
}

############################################################
# record_staged_component
#
# Record a successfully staged component.
#
# Arguments:
#   $1 - Success message
#
# Returns:
#   0
############################################################
record_staged_component()
{
    local message="$1"

    log_success "$message"
    staged_components=$((staged_components + 1))
}

############################################################
# directory_contains_files
#
# Determine whether a directory contains at least one regular
# file other than a Git placeholder.
#
# Arguments:
#   $1 - Directory path
#
# Returns:
#   0 when at least one payload file exists.
#   1 otherwise.
############################################################
directory_contains_files()
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
# 4. Workspace Management
############################################################

############################################################
# prepare_workspace
#
# Remove any previous workspace and create a clean DAIA root.
#
# Payload subdirectories are created after runtime staging so
# they cannot be removed when runtime directories are copied.
#
# Arguments:
#   None
#
# Returns:
#   0 on success.
############################################################
prepare_workspace()
{
    log_section "Preparing payload workspace"

    remove_path "$PAYLOAD_WORKSPACE"

    ensure_directory "$DAIA_ROOT_TARGET"

    log_success "Payload workspace created: $PAYLOAD_WORKSPACE"
}

############################################################
# create_payload_directories
#
# Create the directory hierarchy used for offline artifacts.
#
# This function runs after runtime staging. Keeping this order
# prevents the runtime copy operation from deleting the
# payload directories.
#
# Arguments:
#   None
#
# Returns:
#   0 on success.
############################################################
create_payload_directories()
{
    log_section "Creating offline payload structure"

    ensure_directory "$DAIA_PACKAGES_TARGET/docker"
    ensure_directory "$DAIA_PACKAGES_TARGET/ollama"
    ensure_directory "$DAIA_PACKAGES_TARGET/dependencies"

    ensure_directory "$DAIA_IMAGES_TARGET"
    ensure_directory "$DAIA_MODELS_TARGET/default"

    ensure_directory "$DAIA_BRANDING_TARGET/wallpapers"
    ensure_directory "$DAIA_BRANDING_TARGET/icons"

    log_success "Offline payload directory structure created."
}

############################################################
# 5. Runtime Staging
############################################################

############################################################
# stage_runtime_directory
#
# Copy one optional runtime directory into its destination.
#
# Arguments:
#   $1 - Runtime component name
#   $2 - Source directory
#   $3 - Destination directory
#
# Returns:
#   0
############################################################
stage_runtime_directory()
{
    local component_name="$1"
    local source_directory="$2"
    local destination_directory="$3"

    if ! validate_directory "$source_directory"
    then
        stage_warning \
            "$component_name source directory is missing: $source_directory"
        return 0
    fi

    copy_directory \
        "$source_directory" \
        "$destination_directory"

    record_staged_component \
        "$component_name staged successfully."
}

############################################################
# stage_runtime
#
# Stage each runtime category independently.
#
# Copying individual runtime directories avoids replacing the
# complete /opt/daia target and protects other staged content.
#
# Arguments:
#   None
#
# Returns:
#   0
############################################################
stage_runtime()
{
    log_section "Staging DAIA runtime"

    if ! validate_directory "$DAIA_RUNTIME_SOURCE"
    then
        log_error "Runtime source directory is missing:"
        log_error "  $DAIA_RUNTIME_SOURCE"
        exit 1
    fi

    stage_runtime_directory \
        "Runtime configuration" \
        "$DAIA_RUNTIME_SOURCE/config" \
        "$DAIA_CONFIG_TARGET"

    stage_runtime_directory \
        "Runtime libraries" \
        "$DAIA_RUNTIME_SOURCE/lib" \
        "$DAIA_LIB_TARGET"

    stage_runtime_directory \
        "Runtime modules" \
        "$DAIA_RUNTIME_SOURCE/modules" \
        "$DAIA_MODULES_TARGET"

    stage_runtime_directory \
        "Runtime service definitions" \
        "$DAIA_RUNTIME_SOURCE/services" \
        "$DAIA_SERVICES_TARGET"

    if [[ -f "$DAIA_RUNTIME_SOURCE/bootstrap.sh" ]]
    then
        copy_file \
            "$DAIA_RUNTIME_SOURCE/bootstrap.sh" \
            "$DAIA_ROOT_TARGET/bootstrap.sh" \
            0755

        record_staged_component \
            "Runtime bootstrap staged successfully."
    fi
}

############################################################
# 6. Component Staging
############################################################

############################################################
# stage_directory_component
#
# Stage a component represented by a source directory.
#
# Empty source directories are warnings during development,
# rather than fatal errors.
#
# Arguments:
#   $1 - Component name
#   $2 - Enabled value
#   $3 - Source directory
#   $4 - Destination directory
#
# Returns:
#   0
############################################################
stage_directory_component()
{
    local component_name="$1"
    local component_enabled="$2"
    local source_directory="$3"
    local destination_directory="$4"

    if [[ "$component_enabled" != "true" ]]
    then
        log_info "$component_name is disabled by configuration."
        return 0
    fi

    if ! validate_directory "$source_directory"
    then
        stage_warning \
            "$component_name source directory is missing: $source_directory"
        return 0
    fi

    if ! directory_contains_files "$source_directory"
    then
        stage_warning \
            "$component_name source directory is empty: $source_directory"
        return 0
    fi

    copy_directory \
        "$source_directory" \
        "$destination_directory"

    record_staged_component \
        "$component_name staged successfully."
}

############################################################
# stage_file_component
#
# Stage a component represented by one regular file.
#
# Arguments:
#   $1 - Component name
#   $2 - Enabled value
#   $3 - Source file
#   $4 - Destination file
#   $5 - Optional destination mode
#
# Returns:
#   0
############################################################
stage_file_component()
{
    local component_name="$1"
    local component_enabled="$2"
    local source_file="$3"
    local destination_file="$4"
    local destination_mode="${5:-0644}"

    if [[ "$component_enabled" != "true" ]]
    then
        log_info "$component_name is disabled by configuration."
        return 0
    fi

    if ! validate_nonempty_file "$source_file"
    then
        stage_warning \
            "$component_name source file is missing or empty: $source_file"
        return 0
    fi

    copy_file \
        "$source_file" \
        "$destination_file" \
        "$destination_mode"

    record_staged_component \
        "$component_name staged successfully."
}

############################################################
# stage_offline_components
#
# Stage all configured offline runtime components.
#
# Arguments:
#   None
#
# Returns:
#   0
############################################################
stage_offline_components()
{
    log_section "Staging offline components"

    stage_directory_component \
        "Docker packages" \
        "$DAIA_ENABLE_DOCKER" \
        "$DAIA_DOCKER_SOURCE_ABS" \
        "$DAIA_PACKAGES_TARGET/docker"

    stage_directory_component \
        "Ollama runtime" \
        "$DAIA_ENABLE_OLLAMA" \
        "$DAIA_OLLAMA_SOURCE_ABS" \
        "$DAIA_PACKAGES_TARGET/ollama"

    stage_directory_component \
        "Package dependencies" \
        "$DAIA_OFFLINE_INSTALL" \
        "$DAIA_DEPENDENCIES_SOURCE_ABS" \
        "$DAIA_PACKAGES_TARGET/dependencies"

    stage_file_component \
        "Open WebUI container image" \
        "$DAIA_ENABLE_OPEN_WEBUI" \
        "$DAIA_OPEN_WEBUI_IMAGE_ABS" \
        "$DAIA_IMAGES_TARGET/open-webui.tar"

    stage_directory_component \
        "Default AI model" \
        "$DAIA_ENABLE_DEFAULT_MODEL" \
        "$DAIA_DEFAULT_MODEL_SOURCE_ABS" \
        "$DAIA_MODELS_TARGET/default"
}

############################################################
# 7. Branding Staging
############################################################

############################################################
# stage_branding
#
# Stage configured desktop branding assets.
#
# Arguments:
#   None
#
# Returns:
#   0
############################################################
stage_branding()
{
    log_section "Staging branding resources"

    if [[ "$DAIA_BRANDING_ENABLED" != "true" ]]
    then
        log_info "DAIA branding is disabled by configuration."
        return 0
    fi

    stage_file_component \
        "Default wallpaper" \
        "$DAIA_BRANDING_ENABLED" \
        "$DAIA_DEFAULT_WALLPAPER_ABS" \
        "$DAIA_BRANDING_TARGET/wallpapers/daia-default.png"

    stage_file_component \
        "DAIA Assistant icon" \
        "$DAIA_BRANDING_ENABLED" \
        "$DAIA_ASSISTANT_ICON_ABS" \
        "$DAIA_BRANDING_TARGET/icons/daia-assistant.png"
}

############################################################
# 8. Metadata Generation
############################################################

############################################################
# write_build_information
#
# Write human-readable build metadata into the staged DAIA
# runtime.
#
# Arguments:
#   None
#
# Returns:
#   0
############################################################
write_build_information()
{
    local build_timestamp

    log_section "Generating payload metadata"

    build_timestamp="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"

    {
        printf 'DAIA_NAME=%s\n' "$DAIA_NAME"
        printf 'DAIA_FULL_NAME=%s\n' "$DAIA_FULL_NAME"
        printf 'DAIA_VERSION=%s\n' "$DAIA_VERSION"
        printf 'DAIA_CODENAME=%s\n' "$DAIA_CODENAME"
        printf 'DAIA_EDITION=%s\n' "$DAIA_EDITION"
        printf 'DAIA_ARCHITECTURE=%s\n' "$DAIA_ARCHITECTURE"
        printf 'DAIA_BASE=%s-%s\n' \
            "$DAIA_BASE_DISTRIBUTION" \
            "$DAIA_BASE_VERSION"
        printf 'DAIA_DESKTOP=%s\n' "$DAIA_DESKTOP_ENVIRONMENT"
        printf 'DAIA_BUILD_TIMESTAMP=%s\n' "$build_timestamp"
        printf 'DAIA_CONFIG_FILE=%s\n' "$DAIA_CONFIG_FILE"
    } >"$BUILD_INFO_FILE"

    chmod 0644 "$BUILD_INFO_FILE"

    record_staged_component \
        "Payload build metadata generated."
}

############################################################
# 9. Summary
############################################################

############################################################
# count_workspace_files
#
# Count regular files within the payload workspace.
############################################################
count_workspace_files()
{
    find "$PAYLOAD_WORKSPACE" \
        -type f \
        -print |
        wc -l
}

############################################################
# calculate_workspace_size
#
# Calculate the human-readable payload workspace size.
############################################################
calculate_workspace_size()
{
    du -sh "$PAYLOAD_WORKSPACE" |
        awk '{print $1}'
}

############################################################
# display_summary
#
# Display the final payload workspace status.
############################################################
display_summary()
{
    local file_count
    local workspace_size

    file_count="$(count_workspace_files)"
    workspace_size="$(calculate_workspace_size)"

    log_header "DAIA Payload Build Summary"

    printf 'Distribution       : %s %s\n' \
        "$DAIA_NAME" \
        "$DAIA_VERSION"

    printf 'Codename           : %s\n' "$DAIA_CODENAME"
    printf 'Edition            : %s\n' "$DAIA_EDITION"
    printf 'Workspace          : %s\n' "$PAYLOAD_WORKSPACE"
    printf 'Files staged       : %s\n' "$file_count"
    printf 'Workspace size     : %s\n' "$workspace_size"
    printf 'Components staged  : %s\n' "$staged_components"
    printf 'Components pending : %s\n' "$skipped_components"

    echo

    if (( skipped_components > 0 ))
    then
        log_warning \
            "The workspace was built with pending component artifacts."
    else
        log_success \
            "The payload workspace contains all configured artifacts."
    fi
}

############################################################
# 10. Main
############################################################

main()
{
    log_header "DAIA Payload Workspace Builder"

    log_info \
        "Building payload for $DAIA_NAME $DAIA_VERSION $DAIA_CODENAME."

    prepare_workspace
    stage_runtime
    create_payload_directories
    stage_offline_components
    stage_branding
    write_build_information
    display_summary

    echo
    log_success "DAIA payload workspace assembled successfully."
}

main "$@"

############################################################
# End of File
############################################################
