#!/bin/bash
#
# DAIA Memory Hardware Library
#

# shellcheck disable=SC1091
source "$(dirname "$0")/../common.sh"

#
# Total Installed RAM
#
memory_total() {
    local mem

    mem=$(awk '/MemTotal/ {print $2}' /proc/meminfo)

    kb_to_gb "$mem"
}

#
# Available RAM
#
memory_available() {
    local mem

    mem=$(awk '/MemAvailable/ {print $2}' /proc/meminfo)

    kb_to_gb "$mem"
}

#
# Free RAM
#
memory_free() {
    local mem

    mem=$(awk '/MemFree/ {print $2}' /proc/meminfo)

    kb_to_gb "$mem"
}

#
# Total Swap
#
memory_swap_total() {
    local mem

    mem=$(awk '/SwapTotal/ {print $2}' /proc/meminfo)

    kb_to_gb "$mem"
}

#
# Used Swap
#
memory_swap_used() {

    local total
    local free
    local used

    total=$(awk '/SwapTotal/ {print $2}' /proc/meminfo)
    free=$(awk '/SwapFree/ {print $2}' /proc/meminfo)

    used=$((total-free))

    kb_to_gb "$used"
}

#
# Memory Usage Percentage
#
memory_usage_percent() {

    local total
    local available

    total=$(awk '/MemTotal/ {print $2}' /proc/meminfo)
    available=$(awk '/MemAvailable/ {print $2}' /proc/meminfo)

    awk "BEGIN {
        used = $total - $available;
        printf \"%.1f%%\", (used/$total)*100
    }"
}

#
# Swap Usage Percentage
#
memory_swap_usage_percent() {

    local total
    local free

    total=$(awk '/SwapTotal/ {print $2}' /proc/meminfo)
    free=$(awk '/SwapFree/ {print $2}' /proc/meminfo)

    if [ "$total" -eq 0 ]; then
        echo "0%"
        return
    fi

    awk "BEGIN {
        used = $total - $free;
        printf \"%.1f%%\", (used/$total)*100
    }"
}

#
# Memory Summary
#
memory_summary() {

cat <<EOF
Total RAM         : $(memory_total)
Available RAM     : $(memory_available)
Free RAM          : $(memory_free)

RAM Usage         : $(memory_usage_percent)

Swap Total        : $(memory_swap_total)
Swap Used         : $(memory_swap_used)
Swap Usage        : $(memory_swap_usage_percent)
EOF

}
