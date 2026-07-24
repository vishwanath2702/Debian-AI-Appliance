#!/bin/bash
#
# ==========================================================
# DAIA - Debian AI Assistant
#
# File       : build/test-load-config.sh
# Purpose    : Test the DAIA build configuration loader.
#
# Version    : 1.0.0
# Codename   : Pragna
# License    : GPL-3.0
#
# Responsibilities
# ----------------
# - Load the selected DAIA build configuration.
# - Display the validated values.
# - Confirm generated absolute payload paths.
#
# Dependencies
# ------------
# - build/load-config.sh
#
# Outputs
# -------
# A readable configuration summary.
#
# Failure Modes
# -------------
# Exits non-zero if the loader rejects the configuration.
#
# Future Extension
# ----------------
# Add automated assertions as the configuration schema grows.
# ==========================================================

set -euo pipefail

############################################################
# Configuration
############################################################

SCRIPT_DIR="$(
    cd "$(dirname "${BASH_SOURCE[0]}")" &&
    pwd
)"

# shellcheck disable=SC1091
source "$SCRIPT_DIR/load-config.sh"

############################################################
# Main
############################################################

echo
echo "========================================"
echo " DAIA Configuration Test"
echo "========================================"
echo

printf 'Name              : %s\n' "$DAIA_NAME"
printf 'Full name         : %s\n' "$DAIA_FULL_NAME"
printf 'Version           : %s\n' "$DAIA_VERSION"
printf 'Codename          : %s\n' "$DAIA_CODENAME"
printf 'Edition           : %s\n' "$DAIA_EDITION"
printf 'Architecture      : %s\n' "$DAIA_ARCHITECTURE"
printf 'Base              : %s %s\n' \
    "$DAIA_BASE_DISTRIBUTION" \
    "$DAIA_BASE_VERSION"
printf 'Desktop           : %s\n' "$DAIA_DESKTOP_ENVIRONMENT"
printf 'Docker enabled    : %s\n' "$DAIA_ENABLE_DOCKER"
printf 'Ollama enabled    : %s\n' "$DAIA_ENABLE_OLLAMA"
printf 'Open WebUI enabled: %s\n' "$DAIA_ENABLE_OPEN_WEBUI"
printf 'Offline install   : %s\n' "$DAIA_OFFLINE_INSTALL"

echo
echo "Payload paths:"
printf '  Docker          : %s\n' "$DAIA_DOCKER_SOURCE_ABS"
printf '  Ollama          : %s\n' "$DAIA_OLLAMA_SOURCE_ABS"
printf '  Dependencies    : %s\n' "$DAIA_DEPENDENCIES_SOURCE_ABS"
printf '  Open WebUI      : %s\n' "$DAIA_OPEN_WEBUI_IMAGE_ABS"
printf '  Default model   : %s\n' "$DAIA_DEFAULT_MODEL_SOURCE_ABS"

echo
echo "Configuration loader test completed successfully."

############################################################
# End of File
############################################################
