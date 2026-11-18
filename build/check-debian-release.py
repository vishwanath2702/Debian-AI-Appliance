#!/usr/bin/env python3
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "build/debian-release.json"

def verify():
    data = json.loads(MANIFEST.read_text(encoding="utf-8"))

    expected_keys = {
        "schema_version", "distribution", "codename",
        "release", "architecture", "image_type",
        "filename", "sha256", "release_policy",
        "automatic_upgrade", "require_signed_provenance",
    }

    if set(data) != expected_keys:
        raise ValueError("Unexpected Debian manifest schema")

    required = {
        "schema_version": 1,
        "distribution": "debian",
        "codename": "trixie",
        "architecture": "amd64",
        "image_type": "netinst",
        "release_policy": "pinned",
        "automatic_upgrade": False,
        "require_signed_provenance": True,
    }

    for key, value in required.items():
        if type(data[key]) is not type(value) or data[key] != value:
            raise ValueError(f"Invalid Debian manifest field: {key}")

    release = data["release"]
    if not isinstance(release, str) or not re.fullmatch(
        r"13\.[0-9]+\.[0-9]+", release
    ):
        raise ValueError("Invalid or unsupported Debian release")

    expected_filename = f"debian-{release}-amd64-netinst.iso"
    if data["filename"] != expected_filename:
        raise ValueError("Debian ISO filename/release mismatch")

    digest = data["sha256"]
    if not isinstance(digest, str) or not re.fullmatch(
        r"[0-9a-f]{64}", digest
    ):
        raise ValueError("Invalid pinned Debian SHA256")

    if len(sys.argv) > 2:
        raise ValueError("Usage: check-debian-release.py [ISO_PATH]")

    iso = (
        Path(sys.argv[1])
        if len(sys.argv) == 2
        else ROOT / "iso" / expected_filename
    )

    if iso.name != expected_filename:
        raise ValueError("Selected ISO does not match pinned release")

    if iso.is_symlink() or not iso.is_file():
        raise ValueError("Pinned Debian ISO missing or unsafe")

    actual = hashlib.sha256()
    size = iso.stat().st_size
    processed = 0

    with iso.open("rb") as stream:
        while chunk := stream.read(8 * 1024 * 1024):
            actual.update(chunk)
            processed += len(chunk)
            if processed == size or processed % (256 * 1024 * 1024) < len(chunk):
                print(
                    f"ISO_HASH_PROGRESS={processed}/{size}",
                    flush=True,
                )

    if actual.hexdigest() != digest:
        raise ValueError("Pinned Debian ISO SHA256 mismatch")

    print(f"DEBIAN_RELEASE={release}")
    print(f"DEBIAN_ISO_SHA256={actual.hexdigest()}")
    print("DEBIAN_LOCAL_INTEGRITY=PASS")
    print("DEBIAN_SIGNED_PROVENANCE=NOT_YET_VERIFIED")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(verify())
    except (OSError, ValueError, KeyError, TypeError) as exc:
        print(f"DEBIAN_RELEASE_CHECK=FAIL: {exc}", file=sys.stderr)
        sys.exit(1)
