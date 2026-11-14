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
DAIA_BRANDING_SOURCE="$PROJECT_ROOT/branding"
DAIA_BRANDING_IDENTITY_SOURCE="$DAIA_BRANDING_SOURCE/identity"
DAIA_BRANDING_GRUB_SOURCE="$DAIA_BRANDING_SOURCE/boot/grub"
DAIA_BRANDING_PLYMOUTH_SOURCE="$DAIA_BRANDING_SOURCE/boot/plymouth"
DAIA_BRANDING_WALLPAPER_SOURCE="$DAIA_BRANDING_SOURCE/desktop/wallpaper"
DAIA_BRANDING_PLASMA_SOURCE="$DAIA_BRANDING_SOURCE/desktop/plasma"
DAIA_BRANDING_SDDM_SOURCE="$DAIA_BRANDING_SOURCE/desktop/sddm"
DAIA_BRANDING_INSTALLER_SOURCE="$DAIA_BRANDING_SOURCE/installer"
DAIA_BRANDING_APPLICATION_SOURCE="$DAIA_BRANDING_SOURCE/application"

DAIA_CONFIG_TARGET="$DAIA_ROOT_TARGET/config"
DAIA_LIB_TARGET="$DAIA_ROOT_TARGET/lib"
DAIA_MODULES_TARGET="$DAIA_ROOT_TARGET/modules"
DAIA_SERVICES_TARGET="$DAIA_ROOT_TARGET/services"
DAIA_SYSUSERS_TARGET="$DAIA_ROOT_TARGET/sysusers.d"
DAIA_SYSTEMD_TARGET="$DAIA_WORKSPACE/etc/systemd/system"

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

    if [[ -e "$PAYLOAD_WORKSPACE" || -L "$PAYLOAD_WORKSPACE" ]]
    then
        log_error "Payload workspace already exists; refusing overwrite: $PAYLOAD_WORKSPACE"
        return 1
    fi

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

    copy_file \
        "$DAIA_CONFIG_FILE" \
        "$DAIA_CONFIG_TARGET/pragna.conf" \
        0644

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

    stage_runtime_directory \
        "Runtime system user definitions" \
        "$DAIA_RUNTIME_SOURCE/sysusers.d" \
        "$DAIA_SYSUSERS_TARGET"

    stage_runtime_directory \
        "Runtime polkit authorization policies" \
        "$DAIA_RUNTIME_SOURCE/polkit-1" \
        "$DAIA_ROOT_TARGET/polkit-1"

    if [[ -f "$DAIA_RUNTIME_SOURCE/runtime.sh" ]]
    then
        copy_file \
            "$DAIA_RUNTIME_SOURCE/runtime.sh" \
            "$DAIA_ROOT_TARGET/runtime.sh" \
            0755

        record_staged_component \
            "Runtime orchestrator staged successfully."
    fi

    if [[ ! -f "$DAIA_RUNTIME_SOURCE/pgvector_credentials.py" ]]
    then
        log_error "Required pgvector credential helper is missing:"
        log_error "  $DAIA_RUNTIME_SOURCE/pgvector_credentials.py"
        return 1
    fi

    copy_file \
        "$DAIA_RUNTIME_SOURCE/pgvector_credentials.py" \
        "$DAIA_ROOT_TARGET/pgvector_credentials.py" \
        0755

    record_staged_component \
        "PostgreSQL credential helper staged successfully."

    for helper in \
        pgvector_registration.py \
        pgvector_register_container.py \
        daia-pgvector-verify-registration \
        daia-pgvector-authorize-start \
        daia-pgvector-start \
        daia-pgvector-stop \
        daia-pgvector-init-extension.py \
        daia-pgvector-audit-auth.py
    do
        if [[ ! -f "$DAIA_RUNTIME_SOURCE/$helper" ]] ||
           [[ -L "$DAIA_RUNTIME_SOURCE/$helper" ]]; then
            log_error "Required pgvector helper is missing or unsafe: $helper"
            return 1
        fi
    done

    copy_file \
        "$DAIA_RUNTIME_SOURCE/pgvector_registration.py" \
        "$DAIA_ROOT_TARGET/pgvector_registration.py" \
        0644
    copy_file \
        "$DAIA_RUNTIME_SOURCE/pgvector_register_container.py" \
        "$DAIA_ROOT_TARGET/pgvector_register_container.py" \
        0755
    copy_file \
        "$DAIA_RUNTIME_SOURCE/pgvector_archive.py" \
        "$DAIA_ROOT_TARGET/pgvector_archive.py" \
        0644

    for helper in \
        openwebui_registration.py \
        openwebui_trusted_imports.py \
        daia-openwebui-register-container
    do
        if [[ ! -f "$DAIA_RUNTIME_SOURCE/$helper" ]] ||
           [[ -L "$DAIA_RUNTIME_SOURCE/$helper" ]]; then
            log_error "Required Open WebUI registration helper is missing or unsafe: $helper"
            return 1
        fi
    done

    copy_file \
        "$DAIA_RUNTIME_SOURCE/openwebui_registration.py" \
        "$DAIA_ROOT_TARGET/openwebui_registration.py" \
        0644

    ensure_directory "$DAIA_WORKSPACE/usr/libexec"

    copy_file \
        "$DAIA_RUNTIME_SOURCE/openwebui_trusted_imports.py" \
        "$DAIA_WORKSPACE/usr/libexec/openwebui_trusted_imports.py" \
        0644 || return 1

    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-pgvector-verify-registration" \
        "$DAIA_WORKSPACE/usr/libexec/daia-pgvector-verify-registration" \
        0755

    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-openwebui-register-container" \
        "$DAIA_WORKSPACE/usr/libexec/daia-openwebui-register-container" \
        0755

    # Require and stage Tika trusted registration components.
    for helper in \
        tika_registration.py \
        daia-tika-register-container
    do
        if [[ ! -f "$DAIA_RUNTIME_SOURCE/$helper" ]] ||
           [[ -L "$DAIA_RUNTIME_SOURCE/$helper" ]]; then
            log_error "Required Tika registration helper is missing or unsafe: $helper"
            return 1
        fi
    done

    copy_file \
        "$DAIA_RUNTIME_SOURCE/tika_registration.py" \
        "$DAIA_ROOT_TARGET/tika_registration.py" \
        0644 || return 1

    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-tika-register-container" \
        "$DAIA_WORKSPACE/usr/libexec/daia-tika-register-container" \
        0755 || return 1

    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-openwebui-rag-entrypoint.py" \
        "$DAIA_WORKSPACE/usr/libexec/daia-openwebui-rag-entrypoint.py" \
        0755

    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-openwebui-rag-authorize" \
        "$DAIA_WORKSPACE/usr/libexec/daia-openwebui-rag-authorize" \
        0755 || return 1

    # Provisioning components are staged but not yet activated.
    for helper in \
        daia-openwebui-rag-provision.py \
        daia-openwebui-pgvector-role.py \
        daia-openwebui-pgvector-credentials.py
    do
        if [[ ! -f "$DAIA_RUNTIME_SOURCE/$helper" ]] ||
           [[ -L "$DAIA_RUNTIME_SOURCE/$helper" ]]; then
            log_error "Required Open WebUI RAG helper missing or unsafe: $helper"
            return 1
        fi
    done

    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-openwebui-rag-provision.py" \
        "$DAIA_WORKSPACE/usr/libexec/daia-openwebui-rag-provision.py" \
        0755 || return 1

    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-openwebui-pgvector-role.py" \
        "$DAIA_WORKSPACE/usr/libexec/daia-openwebui-pgvector-role.py" \
        0644 || return 1

    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-openwebui-pgvector-credentials.py" \
        "$DAIA_WORKSPACE/usr/libexec/daia-openwebui-pgvector-credentials.py" \
        0755 || return 1

    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-rag-verify-network" \
        "$DAIA_WORKSPACE/usr/libexec/daia-rag-verify-network" \
        0755

    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-pgvector-authorize-start" \
        "$DAIA_WORKSPACE/usr/libexec/daia-pgvector-authorize-start" \
        0755

    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-pgvector-start" \
        "$DAIA_WORKSPACE/usr/libexec/daia-pgvector-start" \
        0755

    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-pgvector-stop" \
        "$DAIA_WORKSPACE/usr/libexec/daia-pgvector-stop" \
        0755

    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-pgvector-init-extension.py" \
        "$DAIA_WORKSPACE/usr/libexec/daia-pgvector-init-extension.py" \
        0755

    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-pgvector-audit-auth.py" \
        "$DAIA_WORKSPACE/usr/libexec/daia-pgvector-audit-auth.py" \
        0755

    record_staged_component \
        "PostgreSQL registration, startup and shutdown helpers staged successfully."

    if [[ -f "$DAIA_RUNTIME_SOURCE/daia-model-importer.sh" ]]
    then
        ensure_directory "$DAIA_WORKSPACE/usr/bin"

        copy_file \
            "$DAIA_RUNTIME_SOURCE/daia-model-importer.sh" \
            "$DAIA_WORKSPACE/usr/bin/daia-model-importer" \
            0755

        record_staged_component \
            "Graphical model importer staged successfully."
    fi

    if [[ ! -f "$DAIA_RUNTIME_SOURCE/daia-model-manager.py" ]]
    then
        log_error "Required Model Manager executable source is missing:"
        log_error "  $DAIA_RUNTIME_SOURCE/daia-model-manager.py"
        return 1
    fi

    if [[ ! -f "$DAIA_RUNTIME_SOURCE/daia-model-manager.desktop" ]]
    then
        log_error "Required Model Manager desktop launcher is missing:"
        log_error "  $DAIA_RUNTIME_SOURCE/daia-model-manager.desktop"
        return 1
    fi

    ensure_directory "$DAIA_WORKSPACE/usr/bin"

    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-model-manager.py" \
        "$DAIA_WORKSPACE/usr/bin/daia-model-manager" \
        0755

    record_staged_component \
        "Graphical model manager staged successfully."

    if [[ ! -f "$DAIA_RUNTIME_SOURCE/daia-whichllm-offline.py" ]]; then
        log_error "Required offline recommendation helper is missing"
        return 1
    fi
    ensure_directory "$DAIA_WORKSPACE/usr/libexec/daia"
    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-whichllm-offline.py" \
        "$DAIA_WORKSPACE/usr/libexec/daia/daia-whichllm-offline.py" \
        0755
    if [[ ! -f "$DAIA_RUNTIME_SOURCE/daia-whichllm-online.py" ]]; then
        log_error "Required online recommendation helper is missing"
        return 1
    fi
    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-whichllm-online.py" \
        "$DAIA_WORKSPACE/usr/libexec/daia/daia-whichllm-online.py" \
        0755

    # Use the verified local build asset unless explicitly overridden.
    # No downloads or host Python site-packages are used.
    local whichllm_source="${DAIA_WHICHLLM_RUNTIME_SOURCE:-$PROJECT_ROOT/build/assets/whichllm-runtime}"

    if [[ ! -d "$whichllm_source" || -L "$whichllm_source" ]]; then
        log_error "Prepared WhichLLM runtime source is missing or unsafe"
        return 1
    fi

    if [[ -d "$whichllm_source" ]]; then

        if [[ ! -d "$whichllm_source/whichllm" ]] ||
           [[ ! -f "$whichllm_source/whichllm/cli.py" ]] ||
           [[ ! -f "$whichllm_source/whichllm/engine/compatibility.py" ]]
        then
            log_error "Prepared WhichLLM runtime is incomplete"
            return 1
        fi

        ensure_directory "$DAIA_WORKSPACE/usr/libexec/daia"

        if [[ -e "$DAIA_WORKSPACE/usr/libexec/daia/whichllm-runtime" ]]; then
            log_error "WhichLLM runtime destination already exists"
            return 1
        fi

        cp -a -- "$whichllm_source" \
            "$DAIA_WORKSPACE/usr/libexec/daia/whichllm-runtime" ||
            return 1

        record_staged_component \
            "Prepared WhichLLM recommendation runtime staged successfully."
    else
        record_staged_component \
            "WhichLLM online helper staged; runtime not supplied."
    fi

    ensure_directory "$DAIA_WORKSPACE/usr/share/applications"

    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-model-manager.desktop" \
        "$DAIA_WORKSPACE/usr/share/applications/daia-model-manager.desktop" \
        0644

    record_staged_component \
        "Graphical model manager desktop launcher staged successfully."

    # Benchmark web files are installed together because the server
    # loads its controller and JavaScript relative to its own location.
    for benchmark_file in \
        daia-benchmark-web.py \
        daia-benchmark-controller.py \
        daia-benchmark-report.py \
        benchmark.js
    do
        if [[ ! -f "$DAIA_RUNTIME_SOURCE/$benchmark_file" ]]; then
            log_error "Required benchmark web source is missing:"
            log_error "  $DAIA_RUNTIME_SOURCE/$benchmark_file"
            return 1
        fi
    done

    ensure_directory "$DAIA_WORKSPACE/usr/libexec/daia"

    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-benchmark-web.py" \
        "$DAIA_WORKSPACE/usr/libexec/daia/daia-benchmark-web.py" \
        0755

    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-benchmark-controller.py" \
        "$DAIA_WORKSPACE/usr/libexec/daia/daia-benchmark-controller.py" \
        0644

    copy_file \
        "$DAIA_RUNTIME_SOURCE/daia-benchmark-report.py" \
        "$DAIA_WORKSPACE/usr/libexec/daia/daia-benchmark-report.py" \
        0644

    copy_file \
        "$DAIA_RUNTIME_SOURCE/benchmark.js" \
        "$DAIA_WORKSPACE/usr/libexec/daia/benchmark.js" \
        0644

    record_staged_component \
        "Local benchmark web interface staged successfully."

    if [[ -f "$DAIA_RUNTIME_SOURCE/firstboot.sh" ]]
    then
        copy_file \
            "$DAIA_RUNTIME_SOURCE/firstboot.sh" \
            "$DAIA_ROOT_TARGET/firstboot.sh" \
            0755

        record_staged_component \
            "First-boot orchestrator staged successfully."
    fi

    # Install the pgvector unit without enabling or starting it.
    if [[ ! -f "$DAIA_RUNTIME_SOURCE/services/daia-pgvector.service" ]] ||
       [[ -L "$DAIA_RUNTIME_SOURCE/services/daia-pgvector.service" ]]; then
        log_error "Required pgvector systemd service is missing or unsafe"
        return 1
    fi

    ensure_directory "$DAIA_SYSTEMD_TARGET"

    copy_file \
        "$DAIA_RUNTIME_SOURCE/services/daia-pgvector.service" \
        "$DAIA_SYSTEMD_TARGET/daia-pgvector.service" \
        0644

    record_staged_component \
        "PostgreSQL pgvector systemd service staged without activation."

    if [[ -f "$DAIA_RUNTIME_SOURCE/systemd/daia-firstboot.service" ]]
    then
        ensure_directory "$DAIA_SYSTEMD_TARGET"

        copy_file \
            "$DAIA_RUNTIME_SOURCE/systemd/daia-firstboot.service" \
            "$DAIA_SYSTEMD_TARGET/daia-firstboot.service" \
            0644

        record_staged_component \
            "First-boot systemd service staged successfully."
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
stage_verified_artifact()
{
    local component="$1"
    local manifest="$2"
    local trusted_source="$3"
    local trusted_sha256="$4"
    local expected_source="$5"
    local expected_destination="$6"
    local success_message="$7"

    local planner="$PROJECT_ROOT/engine/target/debug/daia-artifact-plan"
    local plan
    local source
    local destination
    local checksum
    local mode
    local source_hash
    local staged_hash
    local target
    local temporary

    if [[ ! -f "$planner" || ! -x "$planner" ]]
    then
        stage_warning "$component: validated artifact planner unavailable"
        return 1
    fi

    if ! plan="$("$planner" "$manifest")"
    then
        stage_warning "$component: artifact manifest validation failed"
        return 1
    fi

    IFS=$'\t' read -r source destination checksum mode <<< "$plan"

    if [[ "$plan" != "$source"$'\t'"$destination"$'\t'"$checksum"$'\t'"$mode" ||
          "$source" != "$expected_source" ||
          "$destination" != "$expected_destination" ||
          "$mode" != "0644" ||
          "$checksum" != "$trusted_sha256" ||
          "$trusted_source" != "$PROJECT_ROOT/$source" ]]
    then
        stage_warning "$component: artifact staging contract mismatch"
        return 1
    fi

    if [[ ! "$trusted_sha256" =~ ^[0-9a-f]{64}$ ]]
    then
        stage_warning "$component: invalid trusted archive checksum"
        return 1
    fi

    if [[ ! -f "$trusted_source" || -L "$trusted_source" ||
          ! -s "$trusted_source" ]]
    then
        stage_warning "$component: source archive missing or unsafe"
        return 1
    fi

    if ! source_hash="$(sha256sum -- "$trusted_source")"
    then
        stage_warning "$component: cannot hash source archive"
        return 1
    fi

    if [[ "${source_hash%% *}" != "$trusted_sha256" ]]
    then
        stage_warning "$component: source archive SHA-256 mismatch"
        return 1
    fi

    target="$DAIA_WORKSPACE/$destination"

    if [[ "$target" != "$DAIA_IMAGES_TARGET/"* ||
          ! -d "$DAIA_IMAGES_TARGET" ||
          -L "$DAIA_IMAGES_TARGET" ]]
    then
        stage_warning "$component: unsafe staging directory"
        return 1
    fi

    if [[ -e "$target" || -L "$target" ]]
    then
        stage_warning "$component: staging destination already exists"
        return 1
    fi

    if ! temporary="$(mktemp "$DAIA_IMAGES_TARGET/.daia-artifact.XXXXXXXX")"
    then
        stage_warning "$component: cannot create staging temporary"
        return 1
    fi

    if ! cp -- "$trusted_source" "$temporary"
    then
        rm -f -- "$temporary"
        stage_warning "$component: archive copy failed"
        return 1
    fi

    if ! staged_hash="$(sha256sum -- "$temporary")"
    then
        rm -f -- "$temporary"
        stage_warning "$component: cannot hash staged archive"
        return 1
    fi

    if [[ "${staged_hash%% *}" != "$trusted_sha256" ]]
    then
        rm -f -- "$temporary"
        stage_warning "$component: staged archive SHA-256 mismatch"
        return 1
    fi

    if ! chmod 0644 "$temporary"
    then
        rm -f -- "$temporary"
        stage_warning "$component: cannot set archive permissions"
        return 1
    fi

    if ! ln -- "$temporary" "$target"
    then
        rm -f -- "$temporary"
        stage_warning "$component: atomic publication failed"
        return 1
    fi

    rm -f -- "$temporary"
    record_staged_component "$success_message"
}

stage_offline_components()
{
    log_section "Staging offline components"

    stage_directory_component \
        "Docker packages" \
        "$DAIA_ENABLE_DOCKER" \
        "$DAIA_DOCKER_SOURCE_ABS" \
        "$DAIA_PACKAGES_TARGET/docker"

    if [[ "$DAIA_ENABLE_OLLAMA" == "true" ]]
    then
        local ollama_checker="$PROJECT_ROOT/build/check-ollama-release.py"
        local ollama_manifest="$PROJECT_ROOT/build/ollama-release.json"
        local ollama_fetch_script="$PROJECT_ROOT/build/fetch-ollama.sh"

        python3 "$ollama_checker" \
            --release-manifest "$ollama_manifest" \
            --fetch-script "$ollama_fetch_script" \
            --archive-directory "$DAIA_OLLAMA_SOURCE_ABS" \
            --approved-offline || return 1

        stage_directory_component \
            "Ollama runtime" \
            "true" \
            "$DAIA_OLLAMA_SOURCE_ABS" \
            "$DAIA_PACKAGES_TARGET/ollama" || return 1

        copy_file \
            "$ollama_manifest" \
            "$DAIA_PACKAGES_TARGET/ollama/ollama-release.json" \
            0644 || return 1

        python3 "$ollama_checker" \
            --release-manifest "$DAIA_PACKAGES_TARGET/ollama/ollama-release.json" \
            --fetch-script "$ollama_fetch_script" \
            --archive-directory "$DAIA_PACKAGES_TARGET/ollama" \
            --approved-offline || return 1
    fi

    stage_directory_component \
        "Package dependencies" \
        "$DAIA_OFFLINE_INSTALL" \
        "$DAIA_DEPENDENCIES_SOURCE_ABS" \
        "$DAIA_PACKAGES_TARGET/dependencies"

    if [[ "$DAIA_ENABLE_OPEN_WEBUI" == "true" ]]
    then
        stage_verified_artifact \
            "Open WebUI" \
            "$PROJECT_ROOT/payload/artifact-manifests/open-webui.yaml" \
            "$DAIA_OPEN_WEBUI_IMAGE_ABS" \
            "$DAIA_OPEN_WEBUI_ARCHIVE_SHA256" \
            "payload/images/open-webui-v0.13.0-slim.tar" \
            "opt/daia/payload/images/open-webui.tar" \
            "Open WebUI container image staged successfully." || return 1
    fi

    if [[ "$DAIA_ENABLE_PGVECTOR" == "true" ]]
    then
        stage_verified_artifact \
            "PostgreSQL pgvector" \
            "$PROJECT_ROOT/payload/artifact-manifests/pgvector.yaml" \
            "$DAIA_PGVECTOR_IMAGE_ABS" \
            "$DAIA_PGVECTOR_ARCHIVE_SHA256" \
            "payload/images/pgvector-pg17.tar" \
            "opt/daia/payload/images/pgvector.tar" \
            "PostgreSQL pgvector container image staged successfully." || return 1
    fi

    if [[ "$DAIA_ENABLE_TIKA" == "true" ]]
    then
        stage_verified_artifact \
            "Apache Tika" \
            "$PROJECT_ROOT/payload/artifact-manifests/tika.yaml" \
            "$DAIA_TIKA_IMAGE_ABS" \
            "$DAIA_TIKA_ARCHIVE_SHA256" \
            "payload/images/tika-3.3.3.0.tar" \
            "opt/daia/payload/images/tika.tar" \
            "Apache Tika container image staged successfully." || return 1
    fi

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

    stage_directory_component \
        "DAIA identity branding" \
        "$DAIA_BRANDING_ENABLED" \
        "$DAIA_BRANDING_IDENTITY_SOURCE" \
        "$DAIA_BRANDING_TARGET/identity"

    stage_directory_component \
        "DAIA GRUB branding" \
        "$DAIA_BRANDING_ENABLED" \
        "$DAIA_BRANDING_GRUB_SOURCE" \
        "$DAIA_BRANDING_TARGET/boot/grub"

    stage_directory_component \
        "DAIA Plymouth branding" \
        "$DAIA_BRANDING_ENABLED" \
        "$DAIA_BRANDING_PLYMOUTH_SOURCE" \
        "$DAIA_BRANDING_TARGET/boot/plymouth"

    stage_directory_component \
        "DAIA wallpaper branding" \
        "$DAIA_BRANDING_ENABLED" \
        "$DAIA_BRANDING_WALLPAPER_SOURCE" \
        "$DAIA_BRANDING_TARGET/desktop/wallpaper"

    stage_directory_component \
        "DAIA Plasma branding" \
        "$DAIA_BRANDING_ENABLED" \
        "$DAIA_BRANDING_PLASMA_SOURCE" \
        "$DAIA_BRANDING_TARGET/desktop/plasma"

    stage_directory_component \
        "DAIA SDDM branding" \
        "$DAIA_BRANDING_ENABLED" \
        "$DAIA_BRANDING_SDDM_SOURCE" \
        "$DAIA_BRANDING_TARGET/desktop/sddm"

    stage_directory_component \
        "DAIA installer branding" \
        "$DAIA_BRANDING_ENABLED" \
        "$DAIA_BRANDING_INSTALLER_SOURCE" \
        "$DAIA_BRANDING_TARGET/installer"

    stage_directory_component \
        "DAIA application branding" \
        "$DAIA_BRANDING_ENABLED" \
        "$DAIA_BRANDING_APPLICATION_SOURCE" \
        "$DAIA_BRANDING_TARGET/application"

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

prepare_artifact_planner()
{
    if [[ "$DAIA_ENABLE_TIKA" != "true" &&
          "$DAIA_ENABLE_PGVECTOR" != "true" &&
          "$DAIA_ENABLE_OPEN_WEBUI" != "true" ]]
    then
        return 0
    fi

    if ! cargo build --offline \
        --manifest-path "$PROJECT_ROOT/engine/Cargo.toml" \
        -p registry --bin daia-artifact-plan
    then
        stage_warning "Unable to build validated artifact-plan CLI"
        return 1
    fi
}

main()
{
    log_header "DAIA Payload Workspace Builder"

    log_info \
        "Building payload for $DAIA_NAME $DAIA_VERSION $DAIA_CODENAME."

    prepare_workspace
    stage_runtime
    create_payload_directories
    prepare_artifact_planner || return 1
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
