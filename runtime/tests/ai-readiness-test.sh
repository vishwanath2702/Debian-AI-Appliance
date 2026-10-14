#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
AI_MODULE="${PROJECT_ROOT}/runtime/modules/ai.sh"

# shellcheck source=/dev/null
source "$AI_MODULE"

fail()
{
    printf 'FAIL: %s\n' "$*" >&2
    exit 1
}

make_fake_ollama()
{
    local directory="$1"

    cat > "${directory}/ollama" <<'SCRIPT'
#!/usr/bin/env bash
set -euo pipefail

count=0
if [[ -f "$DAIA_TEST_CALL_COUNT" ]]; then
    count="$(cat "$DAIA_TEST_CALL_COUNT")"
fi

count=$((count + 1))
printf '%s\n' "$count" > "$DAIA_TEST_CALL_COUNT"

if (( count < DAIA_TEST_READY_ON_CALL )); then
    exit 1
fi

exit 0
SCRIPT

    chmod +x "${directory}/ollama"
}

test_ready_immediately()
{
    local directory
    directory="$(mktemp -d)"
    trap 'rm -rf "$directory"' RETURN

    make_fake_ollama "$directory"

    export DAIA_TEST_CALL_COUNT="${directory}/calls"
    export DAIA_TEST_READY_ON_CALL=1
    export DAIA_OLLAMA_READY_ATTEMPTS=3
    export DAIA_OLLAMA_READY_DELAY=0

    _ai_wait_for_ollama_ready "${directory}/ollama" ||
        fail "immediately ready Ollama was rejected"

    [[ "$(cat "$DAIA_TEST_CALL_COUNT")" == "1" ]] ||
        fail "immediately ready Ollama was called more than once"
}

test_retries_until_ready()
{
    local directory
    directory="$(mktemp -d)"
    trap 'rm -rf "$directory"' RETURN

    make_fake_ollama "$directory"

    export DAIA_TEST_CALL_COUNT="${directory}/calls"
    export DAIA_TEST_READY_ON_CALL=3
    export DAIA_OLLAMA_READY_ATTEMPTS=5
    export DAIA_OLLAMA_READY_DELAY=0

    _ai_wait_for_ollama_ready "${directory}/ollama" ||
        fail "Ollama did not succeed after becoming ready"

    [[ "$(cat "$DAIA_TEST_CALL_COUNT")" == "3" ]] ||
        fail "Ollama readiness retry count was incorrect"
}

test_stops_after_attempt_limit()
{
    local directory
    directory="$(mktemp -d)"
    trap 'rm -rf "$directory"' RETURN

    make_fake_ollama "$directory"

    export DAIA_TEST_CALL_COUNT="${directory}/calls"
    export DAIA_TEST_READY_ON_CALL=99
    export DAIA_OLLAMA_READY_ATTEMPTS=3
    export DAIA_OLLAMA_READY_DELAY=0

    if _ai_wait_for_ollama_ready "${directory}/ollama" 2>/dev/null; then
        fail "unready Ollama unexpectedly succeeded"
    fi

    [[ "$(cat "$DAIA_TEST_CALL_COUNT")" == "3" ]] ||
        fail "Ollama readiness exceeded or missed attempt limit"
}

test_ready_without_home()
{
    local directory
    directory="$(mktemp -d)"
    trap 'rm -rf "$directory"' RETURN

    cat > "${directory}/ollama" <<'SCRIPT'
#!/usr/bin/env bash
set -euo pipefail

[[ "${1:-}" == "list" ]] || exit 1
[[ "${HOME:-}" == "/var/lib/ollama" ]] || exit 1
SCRIPT

    chmod +x "${directory}/ollama"

    (
        unset HOME
        export DAIA_OLLAMA_READY_ATTEMPTS=1
        export DAIA_OLLAMA_READY_DELAY=0

        _ai_wait_for_ollama_ready "${directory}/ollama"
    ) || fail "Ollama readiness failed without inherited HOME"
}

test_ready_immediately
test_retries_until_ready
test_stops_after_attempt_limit
test_ready_without_home

printf 'PASS: Ollama readiness verification\n'
