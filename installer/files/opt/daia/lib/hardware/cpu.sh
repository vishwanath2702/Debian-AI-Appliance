#!/bin/bash
#
# DAIA CPU Hardware Library
#

#
# CPU Vendor
#
# shellcheck disable=SC1091
source "$(dirname "$0")/../common.sh"
cpu_vendor() {
    lscpu | awk -F: '/Vendor ID:/ {print $2}' | xargs
}

#
# CPU Model
#
# shellcheck disable=SC1091
source "$(dirname "$0")/../common.sh"

cpu_model() {
    lscpu | awk -F: '/Model name:/ {print $2}' | xargs
}

#
# Architecture
#
# shellcheck disable=SC1091
source "$(dirname "$0")/../common.sh"
cpu_architecture() {
    uname -m
}

#
# Physical / Logical CPUs
#
# shellcheck disable=SC1091
source "$(dirname "$0")/../common.sh"
cpu_cores() {
    nproc
}
# shellcheck disable=SC1091
source "$(dirname "$0")/../common.sh"
cpu_threads() {
    lscpu | awk -F: '/CPU\(s\):/ {print $2; exit}' | xargs
}

#
# Maximum CPU Frequency
#
# shellcheck disable=SC1091
source "$(dirname "$0")/../common.sh"
cpu_frequency() {
    local freq

    freq=$(lscpu | awk -F: '/CPU max MHz:/ {print $2}' | xargs)

    if [ -z "$freq" ]; then
        echo "Unknown"
    else
        printf "%.0f MHz\n" "$freq"
    fi
}

#
# L3 Cache
#
# shellcheck disable=SC1091
source "$(dirname "$0")/../common.sh"
cpu_cache() {
    lscpu | awk -F: '/L3 cache:/ {print $2}' \
        | sed 's/(.*)//' \
        | xargs

}

#
# CPU Flags
#
# shellcheck disable=SC1091
source "$(dirname "$0")/../common.sh"
cpu_flags() {
    lscpu | awk -F: '/Flags:/ {print $2}' | xargs
}

#
# Virtualization
#
# shellcheck disable=SC1091
source "$(dirname "$0")/../common.sh"
cpu_virtualization() {

    if lscpu | grep -qi "Virtualization"; then
        lscpu | awk -F: '/Virtualization:/ {print $2}' | xargs
    else
        echo "NONE"
    fi
}

#
# Instruction Sets
#
# shellcheck disable=SC1091
source "$(dirname "$0")/../common.sh"
cpu_has_avx() {

    if lscpu | grep -qw avx; then
        echo "YES"
    else
        echo "NO"
    fi
}

# shellcheck disable=SC1091
source "$(dirname "$0")/../common.sh"
cpu_has_avx2() {

    if lscpu | grep -qw avx2; then
        echo "YES"
    else
        echo "NO"
    fi
}

# shellcheck disable=SC1091
source "$(dirname "$0")/../common.sh"
cpu_has_avx512() {

    if lscpu | grep -qw avx512f; then
        echo "YES"
    else
        echo "NO"
    fi
}

#
# Complete Summary
#
cpu_summary() {

cat <<EOF
Vendor            : $(cpu_vendor)
Model             : $(cpu_model)
Architecture      : $(cpu_architecture)
CPU Cores         : $(cpu_cores)
CPU Threads       : $(cpu_threads)
CPU Frequency     : $(cpu_frequency)
L3 Cache          : $(cpu_cache)
Virtualization    : $(cpu_virtualization)
AVX               : $(cpu_has_avx)
AVX2              : $(cpu_has_avx2)
AVX-512           : $(cpu_has_avx512)
EOF

}
