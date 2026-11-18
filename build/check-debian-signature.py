#!/usr/bin/env python3
"""Verify a pinned Debian ISO against signed Debian checksums.

Usage:
    check-debian-signature.py EVIDENCE_DIRECTORY [ISO_PATH]

Required evidence:
    SHA256SUMS
    SHA256SUMS.sign
    debian-cd-signing-keyring.gpg

This verifier never downloads files or imports keys.
"""

import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "build/debian-release.json"

DEBIAN_CD_FINGERPRINT = (
    "DF9B9C49EAA9298432589D76DA87E80D6294BE9B"
)


def checksum_for_image(checksums, filename):
    matches = []

    for line in checksums.splitlines():
        match = re.fullmatch(
            r"([0-9a-fA-F]{64}) [ *](.+)",
            line,
        )

        if not match:
            continue

        digest, name = match.groups()

        if name in (filename, "./" + filename):
            matches.append(digest.lower())

    if len(matches) != 1:
        raise ValueError(
            "Expected exactly one pinned ISO checksum entry"
        )

    return matches[0]


def verify(evidence_directory, iso_path=None):
    manifest = json.loads(
        MANIFEST.read_text(encoding="utf-8")
    )

    if manifest.get("require_signed_provenance") is not True:
        raise ValueError(
            "Signed provenance policy must be enabled"
        )

    filename = manifest["filename"]
    pinned_digest = manifest["sha256"]

    if not isinstance(filename, str) or not re.fullmatch(
        r"debian-13\.[0-9]+\.[0-9]+-amd64-netinst\.iso",
        filename,
    ):
        raise ValueError("Invalid pinned ISO filename")

    if not isinstance(pinned_digest, str) or not re.fullmatch(
        r"[0-9a-f]{64}",
        pinned_digest,
    ):
        raise ValueError("Invalid pinned SHA256")

    evidence = Path(evidence_directory)

    if evidence.is_symlink() or not evidence.is_dir():
        raise ValueError(
            "Evidence directory missing or unsafe"
        )

    checksums = evidence / "SHA256SUMS"
    signature = evidence / "SHA256SUMS.sign"
    keyring = evidence / "debian-cd-signing-keyring.gpg"

    for item in (checksums, signature, keyring):
        if item.is_symlink() or not item.is_file():
            raise ValueError(
                "Required signed evidence missing or unsafe: "
                + item.name
            )


    # Pinned Debian 13.7.0 evidence, authenticated in Step 0837.
    # Reject changes including trailing data, then verify with GPG.
    expected_evidence = {
        "SHA256SUMS": "de6d320c52b0fa02787e4a6f58115ace89f821865baaeba86115c723f34cbab4",
        "SHA256SUMS.sign": "44e770b3d78c23ae581a9c3153d12b948190c1b9dcfc6d64972a192d2c442ca9",
    }
    for item in (checksums, signature):
        actual = hashlib.sha256(item.read_bytes()).hexdigest()
        if actual != expected_evidence[item.name]:
            raise ValueError("Pinned Debian signed evidence changed: " + item.name)

    iso = (
        Path(iso_path)
        if iso_path is not None
        else ROOT / "iso" / filename
    )

    if (
        iso.name != filename
        or iso.is_symlink()
        or not iso.is_file()
    ):
        raise ValueError(
            "Pinned ISO missing or filename mismatch"
        )

    print("DEBIAN_SIGNATURE_CHECK=START", flush=True)

    result = subprocess.run(
        [
            "gpgv",
            "--status-fd", "1",
            "--keyring", str(keyring),
            str(signature),
            str(checksums),
        ],
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )

    if result.returncode != 0:
        raise ValueError(
            "Debian checksum signature verification failed"
        )

    fingerprints = []

    for line in result.stdout.splitlines():
        if line.startswith("[GNUPG:] VALIDSIG "):
            fields = line.split()

            if len(fields) >= 3:
                fingerprints.append(fields[2].upper())

    if (
        len(fingerprints) != 1
        or fingerprints[0] != DEBIAN_CD_FINGERPRINT
    ):
        raise ValueError(
            "Debian signing fingerprint mismatch"
        )

    print(
        "DEBIAN_SIGNING_FINGERPRINT="
        + fingerprints[0],
        flush=True,
    )

    signed_digest = checksum_for_image(
        checksums.read_text(encoding="utf-8"),
        filename,
    )

    if signed_digest != pinned_digest:
        raise ValueError(
            "Signed Debian checksum differs from pinned manifest"
        )

    digest = hashlib.sha256()

    with iso.open("rb") as stream:
        total = iso.stat().st_size
        processed = 0
        next_report = 256 * 1024 * 1024

        while chunk := stream.read(8 * 1024 * 1024):
            digest.update(chunk)
            processed += len(chunk)

            if processed >= next_report or processed == total:
                print(
                    f"DEBIAN_SIGNATURE_HASH_PROGRESS="
                    f"{processed}/{total}",
                    flush=True,
                )
                next_report += 256 * 1024 * 1024

    if digest.hexdigest() != signed_digest:
        raise ValueError(
            "ISO checksum differs from signed checksum"
        )

    print(
        "DEBIAN_SIGNED_PROVENANCE=VERIFIED",
        flush=True,
    )
    print(
        "DEBIAN_SIGNATURE_CHECK=PASS",
        flush=True,
    )

    return 0


if __name__ == "__main__":
    try:
        if len(sys.argv) not in (2, 3):
            raise ValueError(
                "Usage: check-debian-signature.py "
                "EVIDENCE_DIRECTORY [ISO_PATH]"
            )

        sys.exit(
            verify(
                sys.argv[1],
                sys.argv[2] if len(sys.argv) == 3 else None,
            )
        )

    except (
        OSError,
        ValueError,
        KeyError,
        TypeError,
        subprocess.TimeoutExpired,
    ) as exc:
        print(
            f"DEBIAN_SIGNATURE_CHECK=FAIL: {exc}",
            file=sys.stderr,
        )
        print(
            "DEBIAN_SIGNED_PROVENANCE=NOT_VERIFIED",
            file=sys.stderr,
        )
        sys.exit(1)
