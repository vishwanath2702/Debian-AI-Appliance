#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck disable=SC1091
source "$SCRIPT_DIR/variables.sh"

# Fresh Build must never fall back to the shared repository workspace.
[[ -n "${DAIA_WORK_DIR:-}" ]] || {
    echo "ERROR: DAIA_WORK_DIR must be explicitly set" >&2
    exit 1
}

# Validate workspace and output paths before any build operation.
[[ "$DAIA_WORK_DIR" = /* ]] || {
    echo "ERROR: Fresh Build workspace must be absolute" >&2
    exit 1
}

[[ "$DAIA_WORK_DIR" != *"/../"* &&
   "$DAIA_WORK_DIR" != *"/./"* &&
   "$DAIA_WORK_DIR" != */.. &&
   "$DAIA_WORK_DIR" != */. &&
   "$DAIA_WORK_DIR" != *"//"* ]] || {
    echo "ERROR: Unsafe workspace path components" >&2
    exit 1
}

work_parent="$PROJECT_ROOT/work"

[[ -d "$work_parent" && ! -L "$work_parent" ]] || {
    echo "ERROR: Repository work directory is missing or unsafe" >&2
    exit 1
}

[[ "$(dirname "$DAIA_WORK_DIR")" == "$work_parent" ]] || {
    echo "ERROR: Fresh Build workspace must be directly under work/" >&2
    exit 1
}

[[ "$(basename "$DAIA_WORK_DIR")" == daia-fresh-* ]] || {
    echo "ERROR: Fresh Build workspace must use daia-fresh-* naming" >&2
    exit 1
}

# The outer laptop builder creates the workspace under its build lock.
# A direct invocation must still reject any pre-existing workspace.
if [[ "${DAIA_BUILD_HANDOFF:-}" == 1 ]]; then
    # The laptop builder passes its exclusive build lock on FD 9.
    # Validate the descriptor before accepting an existing workspace.
    lock_file="$work_parent/.daia-build-cleanup.lock"

    [[ -f "$lock_file" && ! -L "$lock_file" &&
       -e /proc/self/fd/9 ]] || {
        echo "ERROR: Missing or unsafe build handoff lock" >&2
        exit 1
    }

    lock_identity="$(stat -Lc '%d:%i' /proc/self/fd/9)" || exit 1
    expected_identity="$(stat -c '%d:%i' -- "$lock_file")" || exit 1

    [[ "$lock_identity" == "$expected_identity" ]] || {
        echo "ERROR: Build handoff lock descriptor mismatch" >&2
        exit 1
    }

    flock -n -x 9 || {
        echo "ERROR: Build handoff lock unavailable" >&2
        exit 1
    }

    [[ -d "$DAIA_WORK_DIR" && ! -L "$DAIA_WORK_DIR" ]] || {
        echo "ERROR: Invalid Fresh Build handoff workspace" >&2
        exit 1
    }

    [[ "$(stat -c '%a' -- "$DAIA_WORK_DIR")" == 700 ]] || {
        echo "ERROR: Fresh Build handoff workspace permissions" >&2
        exit 1
    }

    [[ -f "$DAIA_WORK_DIR/.daia-build-status" &&
       ! -L "$DAIA_WORK_DIR/.daia-build-status" &&
       "$(cat "$DAIA_WORK_DIR/.daia-build-status")" == running ]] || {
        echo "ERROR: Fresh Build handoff status invalid" >&2
        exit 1
    }
else
    [[ ! -e "$DAIA_WORK_DIR" && ! -L "$DAIA_WORK_DIR" ]] || {
        echo "ERROR: Fresh Build workspace already exists" >&2
        exit 1
    }
fi

[[ ! -e "$OUTPUT_ISO" && ! -L "$OUTPUT_ISO" ]] || {
    echo "ERROR: Output ISO already exists; refusing overwrite" >&2
    exit 1
}

echo "========================================"
echo "      DAIA BUILD PIPELINE"
echo "========================================"
echo

echo "[1/5] Validating project..."
"$SCRIPT_DIR/check.sh"

echo
echo "[2/5] Cleaning workspace..."
if [[ "${DAIA_BUILD_HANDOFF:-}" == 1 ]]; then
    # FD 9 was verified against the outer builder lock above.
    # Reuse it instead of attempting to acquire a competing lock.
    DAIA_CLEANUP_LOCK_FD=9 "$SCRIPT_DIR/clean.sh" --lock-held
else
    "$SCRIPT_DIR/clean.sh"
fi

echo
echo "[3/5] Building DAIA payload..."
"$SCRIPT_DIR/build-payload.sh"

echo
echo "[4/5] Building DAIA release binaries..."
cargo build \
    --release \
    --manifest-path "$PROJECT_ROOT/engine/Cargo.toml" \
    -p cli \
    -p tui

echo
ollama_approval_args=()
case "${DAIA_OLLAMA_OFFLINE_APPROVED:-false}" in
    true)
        ollama_approval_args=(--ollama-approved-offline)
        ;;
    false)
        ;;
    *)
        echo "ERROR: DAIA_OLLAMA_OFFLINE_APPROVED must be true or false" >&2
        exit 1
        ;;
esac

echo "[5/5] Building DAIA ISO with Rust engine..."
"$PROJECT_ROOT/engine/target/release/daia"     build-iso     desktop     "$WORK_DIR/rootfs"     "$SOURCE_ISO"     "$WORK_DIR/iso-build"     "$OUTPUT_ISO"     "$WORK_DIR/payload/daia" "${ollama_approval_args[@]}"

echo
echo "Calculating SHA256..."
sha256sum "$OUTPUT_ISO"

echo
echo "ISO size:"
du -h "$OUTPUT_ISO"

echo
echo "========================================"
echo " DAIA build completed successfully."
echo "========================================"
echo "ISO:"
echo "  $OUTPUT_ISO"
