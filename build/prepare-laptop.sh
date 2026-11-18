#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

# Read-only acquisition inventory; never grants consent.
python3 "$ROOT/build/plan-laptop-acquisition.py"

# Fail-closed, read-only filesystem permissions gate.
python3 "$ROOT/build/check-laptop-permissions.py" --mode prepare


# SUDO_CAPABILITY_AUDIT_V1
# Read-only policy query; never requests a password or elevation.
check_sudo_capability() {
    printf 'SUDO_AUDIT=START\n'

    if ! command -v sudo >/dev/null 2>&1; then
        printf 'SUDO_INSTALLED=NO\n'
        printf 'SUDO_CAPABILITY=UNAVAILABLE\n'
        return 0
    fi

    printf 'SUDO_INSTALLED=YES\n'

    if sudo -n -l >/dev/null 2>&1; then
        printf 'SUDO_POLICY_QUERY=AUTHORIZED_NONINTERACTIVE\n'

        if sudo -n -v >/dev/null 2>&1; then
            printf 'SUDO_CAPABILITY=PASSWORDLESS_OR_CACHED\n'
        else
            printf 'SUDO_CAPABILITY=POLICY_LIST_ALLOWED_AUTH_REQUIRED\n'
        fi
    else
        printf 'SUDO_POLICY_QUERY=NOT_AUTHORIZED_NONINTERACTIVE\n'
        printf 'SUDO_CAPABILITY=PASSWORD_REQUIRED_OR_DENIED\n'
    fi

    printf 'SUDO_PRIVILEGED_OPERATION=NONE\n'
}

check_sudo_capability



# INTEGRITY_GATE_V1
# Read-only verification. Never downloads or installs anything.
verify_laptop_integrity() {
    local config_file="${ROOT}/payload/config/pragna.conf"
    local ollama_manifest="${ROOT}/build/ollama-release.json"

    printf 'INTEGRITY_GATE=START\n'

    if ! python3 - "$ROOT" "$config_file" "$ollama_manifest" <<'PYVERIFY'
import hashlib
import json
from pathlib import Path
import re
import sys
import time

root = Path(sys.argv[1])
config_path = Path(sys.argv[2])
manifest_path = Path(sys.argv[3])

config = config_path.read_text(encoding="utf-8")

def config_value(name):
    values = re.findall(
        rf'^{re.escape(name)}="([^"]*)"$',
        config, re.MULTILINE,
    )
    if len(values) != 1:
        raise ValueError(f"Invalid config value: {name}")
    return values[0]

def verify(label, relative, expected):
    if not re.fullmatch(r"[0-9a-fA-F]{64}", expected):
        raise ValueError(f"{label}: invalid pinned SHA256")

    path = root / relative
    if not path.is_file() or path.is_symlink():
        raise ValueError(f"{label}: archive missing or unsafe")

    total = path.stat().st_size
    processed = 0
    digest = hashlib.sha256()
    last = time.monotonic()

    print(f"INTEGRITY_COMPONENT={label}", flush=True)
    print(f"INTEGRITY_BYTES_TOTAL={total}", flush=True)

    with path.open("rb") as stream:
        while chunk := stream.read(8 * 1024 * 1024):
            digest.update(chunk)
            processed += len(chunk)
            now = time.monotonic()
            if now - last >= 2 or processed == total:
                print(
                    f"INTEGRITY_PROGRESS={label} "
                    f"{processed}/{total}",
                    flush=True,
                )
                last = now

    if digest.hexdigest().lower() != expected.lower():
        raise ValueError(f"{label}: SHA256 mismatch")

    print(f"INTEGRITY_RESULT={label}:PASS", flush=True)

for label, path_key, hash_key in (
    ("open-webui", "DAIA_OPEN_WEBUI_IMAGE",
     "DAIA_OPEN_WEBUI_ARCHIVE_SHA256"),
    ("pgvector", "DAIA_PGVECTOR_IMAGE",
     "DAIA_PGVECTOR_ARCHIVE_SHA256"),
    ("tika", "DAIA_TIKA_IMAGE",
     "DAIA_TIKA_ARCHIVE_SHA256"),
):
    verify(label, config_value(path_key), config_value(hash_key))

ollama = json.loads(manifest_path.read_text(encoding="utf-8"))
if ollama.get("asset") != "ollama-linux-amd64.tar.zst":
    raise ValueError("Unexpected Ollama archive")

verify(
    "ollama",
    "payload/packages/ollama/" + ollama["asset"],
    ollama["sha256"],
)

for relative in (
    "whichllm/cli.py",
    "whichllm/engine/compatibility.py",
):
    path = root / "build/assets/whichllm-runtime" / relative
    if not path.is_file() or path.is_symlink():
        raise ValueError(f"WhichLLM runtime missing: {relative}")
    print(f"INTEGRITY_RESULT={relative}:PASS", flush=True)

print("INTEGRITY_ARCHIVES=PASS", flush=True)
PYVERIFY
    then
        printf 'INTEGRITY_GATE=FAIL\n'
        return 1
    fi

    printf 'CARGO_OFFLINE_TARGET=x86_64-unknown-linux-gnu\n'

    if ! CARGO_NET_OFFLINE=true cargo metadata \
        --offline \
        --locked \
        --filter-platform x86_64-unknown-linux-gnu \
        --format-version 1 \
        --manifest-path "${ROOT}/engine/Cargo.toml" \
        > /dev/null; then
        printf 'CARGO_OFFLINE_RESOLUTION=FAIL\n'
        return 1
    fi

    printf 'CARGO_OFFLINE_RESOLUTION=PASS\n'
    printf 'INTEGRITY_GATE=PASS\n'
}

missing_tools=0
missing_assets=0

check_command() {
    local tool="$1"
    if command -v "$tool" >/dev/null 2>&1; then
        printf 'PASS TOOL %s\n' "$tool"
    else
        printf 'MISSING TOOL %s\n' "$tool"
        missing_tools=$((missing_tools + 1))
        printf 'INSTALL_PLAN_ITEM=%s\n' "$tool"
        printf 'INSTALL_PERMISSION=NOT_REQUESTED\n'
    fi
}

check_file() {
    local label="$1"
    local path="$2"
    local source="$3"
    local verification="$4"

    if [[ -f "$path" && ! -L "$path" ]]; then
        printf 'PASS ASSET %s\n' "$label"
    else
        printf 'MISSING ASSET %s\n' "$label"
        missing_assets=$((missing_assets + 1))
        printf 'DOWNLOAD_PLAN_ITEM=%s\n' "$label"
        printf 'DOWNLOAD_SOURCE=%s\n' "$source"
        printf 'DOWNLOAD_DESTINATION=%s\n' "$path"
        printf 'DOWNLOAD_VERIFICATION=%s\n' "$verification"
    fi
}

check_directory() {
    local label="$1"
    local path="$2"
    local source="$3"

    if [[ -d "$path" && ! -L "$path" ]]; then
        printf 'PASS ASSET %s\n' "$label"
    else
        printf 'MISSING ASSET %s\n' "$label"
        missing_assets=$((missing_assets + 1))
        printf 'DOWNLOAD_PLAN_ITEM=%s\n' "$label"
        printf 'DOWNLOAD_SOURCE=%s\n' "$source"
        printf 'DOWNLOAD_DESTINATION=%s\n' "$path"
        printf 'DOWNLOAD_VERIFICATION=Trusted source and runtime contract required\n'
    fi
}

printf 'LOOM_PREPARE_PROFILE=laptop\n'
printf 'MODE=READ_ONLY\n'
printf '=== HOST TOOL AUDIT ===\n'

# Docker is not required for staging existing offline image archives.
for tool in cargo rustc xorriso debootstrap mksquashfs \
            grub-mkrescue rsync python3 sha256sum; do
    check_command "$tool"
done

printf '=== OFFLINE ASSET AUDIT ===\n'

if [[ -n "${DAIA_SOURCE_ISO:-}" ]]; then
    check_file \
        "Debian source ISO" \
        "$DAIA_SOURCE_ISO" \
        "Official Debian 13 amd64 netinst ISO" \
        "Official SHA256SUMS and trusted signature"
else
    shopt -s nullglob
    isos=("$ROOT"/iso/debian-*-amd64-netinst.iso)
    shopt -u nullglob

    if [[ "${#isos[@]}" -eq 1 ]]; then
        check_file \
            "Debian source ISO" \
            "${isos[0]}" \
            "Official Debian 13 amd64 netinst ISO" \
            "Official SHA256SUMS and trusted signature"
    else
        printf 'MISSING ASSET Debian source ISO (matches=%s)\n' \
            "${#isos[@]}"
        missing_assets=$((missing_assets + 1))
        printf 'DOWNLOAD_PLAN_ITEM=Debian source ISO\n'
        printf 'DOWNLOAD_SOURCE=Official Debian 13 amd64 netinst ISO\n'
        printf 'DOWNLOAD_DESTINATION=%s/iso/\n' "$ROOT"
        printf 'DOWNLOAD_VERIFICATION=Official SHA256SUMS and trusted signature\n'
    fi
fi

check_directory \
    "WhichLLM runtime" \
    "$ROOT/build/assets/whichllm-runtime" \
    "Trusted WhichLLM source not yet resolved"

check_file \
    "Ollama archive" \
    "$ROOT/payload/packages/ollama/ollama-linux-amd64.tar.zst" \
    "$ROOT/build/ollama-release.json" \
    "Pinned SHA256 release manifest"

check_file \
    "Open WebUI image" \
    "$ROOT/payload/images/open-webui-v0.13.0-slim.tar" \
    "ghcr.io/open-webui/open-webui:v0.13.0-slim" \
    "Pinned archive SHA256 and image identity"

check_file \
    "pgvector image" \
    "$ROOT/payload/images/pgvector-pg17.tar" \
    "pgvector/pgvector:pg17" \
    "Pinned archive SHA256 and image digest"

check_file \
    "Tika image" \
    "$ROOT/payload/images/tika-3.3.3.0.tar" \
    "docker.io/apache/tika:3.3.3.0" \
    "Pinned archive SHA256 and image digest"

check_file \
    "Rust lockfile" \
    "$ROOT/engine/Cargo.lock" \
    "Tracked Git repository content" \
    "Git provenance and locked dependencies"

printf 'PREPARE_MISSING_TOOLS=%s\n' "$missing_tools"
printf 'PREPARE_MISSING_ASSETS=%s\n' "$missing_assets"
printf 'PREPARE_MISSING_COUNT=%s\n' \
    "$((missing_tools + missing_assets))"

if (( missing_assets > 0 )); then
    printf 'DOWNLOAD_PLAN_STATUS=REQUIRES_VERIFIED_SOURCES\n'
    printf 'DOWNLOAD_CONSENT_DEFAULT=NO\n'
    printf 'DOWNLOAD_CONSENT=NOT_REQUESTED_DIAGNOSTIC_ONLY\n'
    printf 'DOWNLOAD_EXECUTION=NOT_IMPLEMENTED\n'
else
    printf 'DOWNLOAD_PLAN_STATUS=NO_MISSING_ASSETS\n'
    printf 'DOWNLOAD_CONSENT=NOT_REQUIRED\n'
    printf 'DOWNLOAD_EXECUTION=NONE\n'
fi

if (( missing_tools > 0 )); then
    printf 'INSTALL_PLAN_STATUS=USER_APPROVAL_REQUIRED\n'
    printf 'INSTALL_EXECUTION=NONE\n'
else
    printf 'INSTALL_PLAN_STATUS=NO_MISSING_TOOLS\n'
    printf 'INSTALL_EXECUTION=NONE\n'
fi

if (( missing_tools + missing_assets > 0 )); then
    printf 'PREPARE_STATUS=INCOMPLETE\n'
    exit 1
fi

if ! verify_laptop_integrity; then
        printf 'PREPARE_STATUS=INTEGRITY_FAILED\n'
        exit 1
    fi
    printf '=== PINNED DEBIAN ISO CHECK ===\n'
# Respect the caller's ISO selection; otherwise let the release
# validator resolve its pinned filename from the manifest.
if [[ -n "${DAIA_SOURCE_ISO:-}" ]]; then
    if ! python3 "$ROOT/build/check-debian-release.py" "$DAIA_SOURCE_ISO"; then
        printf 'PINNED_DEBIAN_ISO_CHECK=FAIL\n'
        printf 'PREPARE_STATUS=INTEGRITY_FAILED\n'
        exit 1
    fi
else
    if ! python3 "$ROOT/build/check-debian-release.py"; then
        printf 'PINNED_DEBIAN_ISO_CHECK=FAIL\n'
        printf 'PREPARE_STATUS=INTEGRITY_FAILED\n'
        exit 1
    fi
fi
printf 'PINNED_DEBIAN_ISO_CHECK=PASS\n'
printf '=== SIGNED DEBIAN PROVENANCE CHECK ===\n'
DAIA_DEBIAN_EVIDENCE_DIR="${DAIA_DEBIAN_EVIDENCE_DIR:-$ROOT/iso/debian-13.7.0-evidence}"

if [[ -n "${DAIA_SOURCE_ISO:-}" ]]; then
    if ! python3 "$ROOT/build/check-debian-signature.py" \
        "$DAIA_DEBIAN_EVIDENCE_DIR" "$DAIA_SOURCE_ISO"; then
        printf 'SIGNED_DEBIAN_PROVENANCE_CHECK=FAIL\n'
        printf 'PREPARE_STATUS=SIGNED_PROVENANCE_FAILED\n'
        exit 1
    fi
else
    if ! python3 "$ROOT/build/check-debian-signature.py" \
        "$DAIA_DEBIAN_EVIDENCE_DIR"; then
        printf 'SIGNED_DEBIAN_PROVENANCE_CHECK=FAIL\n'
        printf 'PREPARE_STATUS=SIGNED_PROVENANCE_FAILED\n'
        exit 1
    fi
fi

printf 'SIGNED_DEBIAN_PROVENANCE_CHECK=PASS\n'
printf 'DEBIAN_SIGNED_PROVENANCE=VERIFIED\n'


printf 'PREPARE_STATUS=READY_FOR_FURTHER_VERIFICATION\n'
