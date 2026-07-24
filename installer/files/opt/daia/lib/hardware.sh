#!/bin/bash
#
# DAIA Hardware Abstraction Layer
#

HAL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck disable=SC1091
source "$HAL_DIR/common.sh"

# shellcheck disable=SC1091
source "$HAL_DIR/hardware/cpu.sh"

# shellcheck disable=SC1091
source "$HAL_DIR/hardware/memory.sh"
