#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_DIR="$ROOT_DIR/tests"

passed=0
failed=0

run_suite() {
    local suite="$1"

    if [[ ! -d "$suite" ]]; then
        return
    fi

    while IFS= read -r -d '' test; do
        printf '==> %s\n' "${test#$ROOT_DIR/}"

        if bash "$test"; then
            ((passed++))
        else
            ((failed++))
        fi

        printf '\n'
    done < <(find "$suite" -type f -name '*-test.sh' -print0 | sort -z)
}

run_suite "$TEST_DIR/unit"
run_suite "$TEST_DIR/integration"

printf '========================================\n'
printf 'Passed : %d\n' "$passed"
printf 'Failed : %d\n' "$failed"
printf 'Total  : %d\n' "$((passed + failed))"

if (( failed > 0 )); then
    exit 1
fi

printf '\nAll tests passed.\n'
