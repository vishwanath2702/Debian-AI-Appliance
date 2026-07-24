#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "========================================"
echo " DAIA ShellCheck Validation"
echo "========================================"
echo

mapfile -d '' FILES < <(
    find \
        "$ROOT/build" \
        "$ROOT/installer" \
        -type f \
        \( -name '*.sh' -o -name '*.bash' \) \
        -print0
)

if [[ ${#FILES[@]} -eq 0 ]]; then
    echo "No shell scripts found."
    exit 0
fi

status=0

for file in "${FILES[@]}"; do
    echo "Checking: ${file#"$ROOT"/}"

    if ! shellcheck "$file"; then
        status=1
    fi

    echo
done

if [[ "$status" -ne 0 ]]; then
    echo "ShellCheck found issues."
    exit 1
fi

echo "All DAIA shell scripts passed ShellCheck."
