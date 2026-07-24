#!/bin/bash

set -euo pipefail

source installer/files/opt/daia/lib/hardware/cpu.sh

echo "========================================"
echo "        DAIA CPU TEST"
echo "========================================"
echo

cpu_summary

echo
echo "========================================"
echo "PASS - CPU module test completed"
echo "========================================"
