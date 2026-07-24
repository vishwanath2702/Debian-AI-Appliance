#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : runtime/lib/filesystem.sh
# Purpose    : Provide safe filesystem helper functions.
#
# Version    : 1.0.0
# Codename   : Pragna
# License    : GPL-3.0
#
# This file is intended to be sourced by DAIA runtime scripts.
# It must not be executed directly.
# ==========================================================

############################################################
# ensure_directory
#
# Create a directory when it does not already exist.
#
# Arguments:
#   $1 - Directory path
#   $2 - Optional permissions, default 0755
#
# Returns:
#   0 on success.
############################################################
ensure_directory()
{
    local directory_path="$1"
    local directory_mode="${2:-0755}"

    mkdir -p "$directory_path"
    chmod "$directory_mode" "$directory_path"
}

############################################################
# copy_file
#
# Copy one file and apply the requested permissions.
#
# Arguments:
#   $1 - Source file
#   $2 - Destination file
#   $3 - Optional permissions, default 0644
#
# Returns:
#   0 on success.
#   1 when the source file is missing.
############################################################
copy_file()
{
    local source_file="$1"
    local destination_file="$2"
    local destination_mode="${3:-0644}"
    local destination_directory

    if [[ ! -f "$source_file" ]]
    then
        return 1
    fi

    destination_directory="$(dirname "$destination_file")"

    mkdir -p "$destination_directory"
    cp "$source_file" "$destination_file"
    chmod "$destination_mode" "$destination_file"
}

############################################################
# copy_directory
#
# Replace a destination directory with a copied source tree.
#
# Arguments:
#   $1 - Source directory
#   $2 - Destination directory
#
# Returns:
#   0 on success.
#   1 when the source directory is missing.
############################################################
copy_directory()
{
    local source_directory="$1"
    local destination_directory="$2"
    local destination_parent

    if [[ ! -d "$source_directory" ]]
    then
        return 1
    fi

    destination_parent="$(dirname "$destination_directory")"

    mkdir -p "$destination_parent"
    rm -rf "$destination_directory"
    cp -R "$source_directory" "$destination_directory"
}

############################################################
# remove_path
#
# Remove a file, symbolic link, or directory.
#
# Arguments:
#   $1 - Path to remove
#
# Returns:
#   0
############################################################
remove_path()
{
    local target_path="$1"

    if [[ -e "$target_path" || -L "$target_path" ]]
    then
        rm -rf "$target_path"
    fi
}

############################################################
# create_symbolic_link
#
# Create or replace a symbolic link.
#
# Arguments:
#   $1 - Link target
#   $2 - Link path
#
# Returns:
#   0 on success.
############################################################
create_symbolic_link()
{
    local link_target="$1"
    local link_path="$2"
    local link_directory

    link_directory="$(dirname "$link_path")"

    mkdir -p "$link_directory"
    ln -sfn "$link_target" "$link_path"
}

############################################################
# End of File
############################################################
