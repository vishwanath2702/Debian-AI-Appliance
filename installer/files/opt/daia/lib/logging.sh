#!/usr/bin/env bash

# ==========================================================
# DAIA Logging Compatibility Layer
#
# Provides the legacy installer logging API by wrapping the
# builder logger implementation.
# ==========================================================

set -euo pipefail

readonly DAIA_LOGGER_LIBRARY="${DAIA_HOME}/builder/logger.sh"

# shellcheck source=/dev/null
source "$DAIA_LOGGER_LIBRARY"

if ! daia_logger_is_initialized; then
    daia_logger_init
fi

log_debug() {
    daia_logger_debug "$@"
}

log_info() {
    daia_logger_info "$@"
}

log_warn() {
    daia_logger_warn "$@"
}

log_error() {
    daia_logger_error "$@"
}

log_success() {
    daia_logger_info "$@"
}
