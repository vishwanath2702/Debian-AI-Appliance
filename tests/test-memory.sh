#!/bin/bash

set -euo pipefail

source installer/files/opt/daia/lib/ui.sh
source installer/files/opt/daia/lib/hardware/memory.sh

ui_header "DAIA MEMORY TEST"

memory_summary

ui_pass "Memory module test completed"
