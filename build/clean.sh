#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck disable=SC1091
source "$SCRIPT_DIR/variables.sh"

die() {
    printf 'CLEANUP REFUSED: %s\n' "$*" >&2
    exit 1
}

# Cleanup must never implicitly target the repository's shared work directory.
[[ -n "${DAIA_WORK_DIR:-}" ]] ||
    die "DAIA_WORK_DIR must be explicitly set"

[[ "$DAIA_WORK_DIR" = /* ]] ||
    die "DAIA_WORK_DIR must be absolute"

[[ "$DAIA_WORK_DIR" != *"/../"* &&
   "$DAIA_WORK_DIR" != *"/./"* &&
   "$DAIA_WORK_DIR" != */.. &&
   "$DAIA_WORK_DIR" != */. &&
   "$DAIA_WORK_DIR" != *"//"* ]] ||
    die "workspace path contains unsafe components"

work_parent="$PROJECT_ROOT/work"

[[ -d "$work_parent" && ! -L "$work_parent" ]] ||
    die "repository work directory is missing or a symlink"

canonical_parent="$(realpath -e -- "$work_parent")" ||
    die "cannot resolve repository work directory"

[[ "$(dirname "$DAIA_WORK_DIR")" == "$work_parent" ]] ||
    die "workspace must be a direct child of repository work"

workspace_name="$(basename "$DAIA_WORK_DIR")"

[[ "$workspace_name" == daia-fresh-* ]] ||
    die "workspace name must begin with daia-fresh-"

[[ ! -L "$DAIA_WORK_DIR" ]] ||
    die "workspace must not be a symlink"

[[ ! -e "$DAIA_WORK_DIR" || -d "$DAIA_WORK_DIR" ]] ||
    die "workspace exists but is not a directory"

if [[ -d "$DAIA_WORK_DIR" ]]; then
    canonical_workspace="$(realpath -e -- "$DAIA_WORK_DIR")" ||
        die "cannot resolve workspace"

    [[ "$canonical_workspace" == "$canonical_parent/$workspace_name" ]] ||
        die "workspace resolves outside the approved directory"

    command -v findmnt >/dev/null ||
        die "findmnt is required for mount safety checks"

    while IFS= read -r mount_target; do
        [[ -n "$mount_target" ]] || continue

        if [[ "$mount_target" == "$canonical_workspace" ||
              "$mount_target" == "$canonical_workspace/"* ]]; then
            die "workspace contains a mounted filesystem"
        fi
    done < <(findmnt -rn -o TARGET)
fi

printf 'CLEANUP_MODE=DRY_RUN\n'
printf 'WORKSPACE=%s\n' "$DAIA_WORK_DIR"
printf 'WORKSPACE_EXISTS=%s\n' "$([[ -d "$DAIA_WORK_DIR" ]] && echo yes || echo no)"
printf 'OUTPUT_ISO_PRESERVED=%s\n' "$OUTPUT_ISO"
printf 'NO_CHANGES_MADE=True\n'
