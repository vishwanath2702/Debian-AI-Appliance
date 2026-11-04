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

[[ ! -e "$DAIA_WORK_DIR" && ! -L "$DAIA_WORK_DIR" ]] || {
    echo "ERROR: Fresh Build workspace already exists" >&2
    exit 1
}

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
"$SCRIPT_DIR/clean.sh"

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
echo "[5/5] Building DAIA ISO with Rust engine..."
"$PROJECT_ROOT/engine/target/release/daia"     build-iso     desktop     "$WORK_DIR/rootfs"     "$SOURCE_ISO"     "$WORK_DIR/iso-build"     "$OUTPUT_ISO"     "$WORK_DIR/payload/daia"

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
