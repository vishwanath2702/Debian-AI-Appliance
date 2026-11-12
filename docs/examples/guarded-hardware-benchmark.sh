#!/usr/bin/env bash
# Loom / DAIA example: guarded hardware diagnostics and CPU microbenchmark.
# Run: bash docs/examples/guarded-hardware-benchmark.sh
# No sudo, package installs, configuration edits, service restarts, or disk-write benchmark.
(
set -u
set -o pipefail
umask 077

if ! command -v mktemp >/dev/null 2>&1 || ! command -v tee >/dev/null 2>&1; then
    printf 'GUARD STOP: mktemp and tee are required\n' >&2
    exit 1
fi
REPORT="$(mktemp /tmp/daia-hardware-benchmark-XXXXXXXX.txt)" || exit 1

section() { printf '\n========== %s ==========\n' "$1"; }
check() {
    local name="$1"
    shift
    printf '\n--- %s ---\n' "$name"
    if ! command -v "$1" >/dev/null 2>&1; then
        printf 'Tool not installed: %s\n' "$1"
    elif ! command -v timeout >/dev/null 2>&1; then
        printf 'GUARD STOP: timeout utility unavailable; skipped %s\n' "$name"
    else
        timeout 15 "$@" 2>&1 || printf '[Check failed or timed out]\n'
    fi
}

{
    printf '\n============================================\n'
    printf '       DAIA HARDWARE BENCHMARK REPORT\n'
    printf '============================================\n'
    printf 'Date: %s\n' "$(date -Is)"
    printf 'Kernel: %s\n' "$(uname -sr)"
    printf 'Report: %s\n' "$REPORT"

    section "CPU"
    check "Processor information" lscpu
    section "MEMORY"
    check "RAM information" free -h
    section "STORAGE"
    check "Block devices" lsblk -o NAME,SIZE,TYPE,MODEL,TRAN,ROTA
    check "Filesystem usage" df -h
    section "GPU"
    if command -v lspci >/dev/null 2>&1 && command -v timeout >/dev/null 2>&1; then
        timeout 15 lspci -nnk 2>&1 | grep -A 4 -Ei 'VGA|3D controller|Display controller' ||
            printf 'GPU information unavailable\n'
    else
        printf 'GPU inspection tools unavailable\n'
    fi
    if command -v nvidia-smi >/dev/null 2>&1; then
        check "NVIDIA GPU" nvidia-smi
    fi
    section "NETWORK"
    check "Network interfaces" ip -brief link
    check "NetworkManager devices" nmcli device status
    section "BLUETOOTH"
    check "Bluetooth controllers" bluetoothctl list
    section "USB AND SDR"
    check "Connected USB devices" lsusb
    section "PCI DEVICES"
    check "PCI hardware" lspci -nn
    section "CPU BENCHMARK"
    if command -v openssl >/dev/null 2>&1 && command -v timeout >/dev/null 2>&1; then
        printf 'Running SHA-256 throughput benchmark (3 seconds per block size)...\n'
        # OpenSSL tests multiple block sizes; 10 seconds is too short for the whole run.
        timeout 35 openssl speed -seconds 3 sha256 2>&1
        status=$?
        if [ "$status" -eq 124 ]; then
            printf '[TIMEOUT: CPU benchmark exceeded 35 seconds]\n'
        elif [ "$status" -ne 0 ]; then
            printf '[FAILED: CPU benchmark exit code %s]\n' "$status"
        fi
    else
        printf 'SKIP: openssl or timeout unavailable\n'
    fi
    section "GUARDED SAFETY CHECK"
    printf 'Root privileges required: NO\nPackage installation: NO\n'
    printf 'Configuration changes: NO\nService restarts: NO\nDisk write benchmark: NO\n'
    printf 'External commands limited by timeouts: YES (where executed)\n'
    printf 'Report created with restricted permissions: YES\n'
    section "COMPLETE"
    printf 'Report saved to: %s\n' "$REPORT"
} 2>&1 | tee "$REPORT"
printf '\nReport location: %s\n' "$REPORT"
)
