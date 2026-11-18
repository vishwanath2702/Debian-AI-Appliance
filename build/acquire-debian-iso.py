#!/usr/bin/env python3
"""Acquire an explicitly approved, pinned Debian installation ISO.

No automatic upgrades, key imports, evidence downloads, or overwrites.
The caller must supply an official HTTPS archive URL and per-run consent.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import signal
import sys
import tempfile
import time
import urllib.request
from urllib.parse import urlsplit, unquote

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "build/debian-release.json"

MAX_BYTES = 1024 * 1024 * 1024
CHUNK = 1024 * 1024
PINNED_RELEASE = "13.7.0"
PINNED_FILENAME = "debian-13.7.0-amd64-netinst.iso"

def report(message):
    print(message, flush=True)

def validate_manifest(path):
    if path.is_symlink() or not path.is_file():
        raise ValueError("Unsafe or missing Debian manifest")

    data = json.loads(path.read_text(encoding="utf-8"))

    required = {
        "schema_version": 1,
        "distribution": "debian",
        "codename": "trixie",
        "release": PINNED_RELEASE,
        "architecture": "amd64",
        "image_type": "netinst",
        "filename": PINNED_FILENAME,
        "release_policy": "pinned",
        "automatic_upgrade": False,
        "require_signed_provenance": True,
    }

    for key, expected in required.items():
        if type(data.get(key)) is not type(expected):
            raise ValueError("Invalid Debian manifest type: " + key)
        if data[key] != expected:
            raise ValueError("Invalid Debian manifest field: " + key)

    if set(data) != set(required) | {"sha256"}:
        raise ValueError("Unexpected Debian manifest schema")

    digest = data["sha256"]
    if not isinstance(digest, str) or not re.fullmatch(
        r"[0-9a-f]{64}", digest
    ):
        raise ValueError("Invalid pinned Debian SHA256")

    return data

def validate_url(url, filename):
    if not isinstance(url, str):
        raise ValueError("Invalid download URL")

    parsed = urlsplit(url)

    expected_path = (
        "/cdimage/archive/13.7.0/amd64/iso-cd/"
        + PINNED_FILENAME
    )

    if (
        parsed.scheme != "https"
        or parsed.netloc != "cdimage.debian.org"
        or parsed.path != expected_path
        or parsed.query
        or parsed.fragment
        or parsed.username is not None
        or parsed.password is not None
        or parsed.port is not None
        or parsed.hostname != "cdimage.debian.org"
        or filename != PINNED_FILENAME
        or "%" in parsed.path
        or "\\" in parsed.path
        or unquote(parsed.path) != expected_path
    ):
        raise ValueError(
            "URL is not the exact pinned official Debian archive URL"
        )

    return url

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(
        self, request, fp, code, message, headers, new_url
    ):
        raise ValueError("HTTP redirects are prohibited")

def download(url, destination, expected, opener=None):
    if opener is None:
        opener = urllib.request.build_opener(NoRedirect)

    digest = hashlib.sha256()
    total = 0
    last_report = time.monotonic()

    request = urllib.request.Request(
        url,
        headers={
            "User-Agent": "Loom-Pinned-Debian-Acquisition/1",
        },
    )

    with opener.open(request, timeout=30) as response:
        if response.status != 200:
            raise ValueError("Unexpected HTTP response status")

        declared = response.headers.get("Content-Length")
        if declared is not None:
            if not declared.isdecimal():
                raise ValueError("Invalid HTTP content length")
            if int(declared) > MAX_BYTES:
                raise ValueError("Declared download exceeds size limit")

        with destination.open("xb") as output:
            while True:
                chunk = response.read(CHUNK)
                if not chunk:
                    break

                total += len(chunk)
                if total > MAX_BYTES:
                    raise ValueError("ISO exceeds download size limit")

                output.write(chunk)
                digest.update(chunk)

                now = time.monotonic()
                if now - last_report >= 2:
                    report(f"DOWNLOAD_PROGRESS_BYTES={total}")
                    last_report = now

            output.flush()
            os.fsync(output.fileno())

    report(f"DOWNLOAD_PROGRESS_BYTES={total}")

    if total == 0:
        raise ValueError("Empty ISO download")

    if digest.hexdigest() != expected:
        raise ValueError("Pinned Debian ISO SHA256 mismatch")

    report("DOWNLOAD_SHA256=PASS")

def verify(stage, evidence, root=ROOT, timeout=180):
    import selectors

    if evidence.is_symlink() or not evidence.is_dir():
        raise ValueError("Missing or unsafe signed evidence directory")

    for name in (
        "SHA256SUMS",
        "SHA256SUMS.sign",
        "debian-cd-signing-keyring.gpg",
    ):
        item = evidence / name
        if item.is_symlink() or not item.is_file():
            raise ValueError("Missing or unsafe signed evidence: " + name)

    checks = (
        (
            "check-debian-release.py",
            [str(stage)],
            "DEBIAN_LOCAL_INTEGRITY=PASS",
        ),
        (
            "check-debian-signature.py",
            [str(evidence), str(stage)],
            "DEBIAN_SIGNATURE_CHECK=PASS",
        ),
    )

    for script, arguments, expected_marker in checks:
        report("VERIFICATION_STAGE=" + script)

        process = subprocess.Popen(
            [
                sys.executable,
                "-u",
                str(root / "build" / script),
                *arguments,
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            bufsize=0,
            start_new_session=True,
        )

        deadline = time.monotonic() + timeout
        pending = bytearray()
        markers = set()
        selector = selectors.DefaultSelector()
        completed = False

        try:
            selector.register(process.stdout, selectors.EVENT_READ)

            while selector.get_map():
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise TimeoutError(
                        "Debian verifier exceeded timeout: " + script
                    )

                events = selector.select(
                    timeout=min(remaining, 0.25)
                )

                for key, _ in events:
                    chunk = os.read(key.fileobj.fileno(), 65536)

                    if not chunk:
                        selector.unregister(key.fileobj)
                        continue

                    pending.extend(chunk)

                    while b"\n" in pending:
                        raw, _, rest = pending.partition(b"\n")
                        pending = bytearray(rest)

                        line = raw.decode(
                            "utf-8", errors="replace"
                        ).rstrip("\r")

                        report("VERIFICATION_OUTPUT=" + line)

                        if len(line) <= 4096:
                            markers.add(line)

                    if len(pending) > 1024 * 1024:
                        raise ValueError(
                            "Verifier output line too large"
                        )

            if pending:
                line = pending.decode(
                    "utf-8", errors="replace"
                ).rstrip("\r")

                report("VERIFICATION_OUTPUT=" + line)

                if len(line) <= 4096:
                    markers.add(line)

            remaining = deadline - time.monotonic()

            if remaining <= 0:
                raise TimeoutError(
                    "Debian verifier exceeded timeout: " + script
                )

            returncode = process.wait(timeout=remaining)
            completed = True

        finally:
            selector.close()

            if not completed:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass

            if process.poll() is None:
                process.kill()

            process.wait()
            process.stdout.close()

        if returncode != 0 or expected_marker not in markers:
            raise ValueError(
                "Debian verification failed: " + script
            )

        report(expected_marker)


def acquire(
    url,
    destination,
    evidence,
    expected,
    approved,
    opener=None,
    verifier=None,
):
    if not approved:
        raise ValueError("Explicit per-invocation consent required")

    validate_url(url, destination.name)

    if destination.is_symlink() or destination.exists():
        raise ValueError("Destination already exists; overwrite prohibited")

    parent = destination.parent

    if parent.is_symlink() or not parent.is_dir():
        raise ValueError("Unsafe or missing destination directory")

    parent_fd = os.open(
        parent,
        os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
    )

    try:
        parent_identity = os.fstat(parent_fd)

        if stat.S_IMODE(parent_identity.st_mode) & 0o002:
            raise ValueError(
                "World-writable destination directory prohibited"
            )

        if evidence.is_symlink() or not evidence.is_dir():
            raise ValueError("Signed evidence unavailable")

        current = parent.stat()

        if (
            current.st_dev != parent_identity.st_dev
            or current.st_ino != parent_identity.st_ino
        ):
            raise ValueError("Destination directory changed")

        with tempfile.TemporaryDirectory(
            prefix=".daia-debian-acquire-",
            dir=parent,
        ) as directory:
            stage_dir = Path(directory)
            stage = stage_dir / destination.name

            download(url, stage, expected, opener=opener)
            (verifier or verify)(stage, evidence)

            stage_fd = os.open(
                stage_dir,
                os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
            )

            try:
                stage_identity = os.fstat(stage_fd)
                current_stage = stage_dir.stat()

                if (
                    current_stage.st_dev != stage_identity.st_dev
                    or current_stage.st_ino != stage_identity.st_ino
                ):
                    raise ValueError("Staging directory changed")

                current = parent.stat()

                if (
                    current.st_dev != parent_identity.st_dev
                    or current.st_ino != parent_identity.st_ino
                    or parent.is_symlink()
                ):
                    raise ValueError("Destination directory changed")

                staged = os.stat(
                    stage.name,
                    dir_fd=stage_fd,
                    follow_symlinks=False,
                )

                if not stat.S_ISREG(staged.st_mode):
                    raise ValueError("Staged ISO is not a regular file")

                try:
                    os.stat(
                        destination.name,
                        dir_fd=parent_fd,
                        follow_symlinks=False,
                    )
                except FileNotFoundError:
                    pass
                else:
                    raise ValueError(
                        "Destination appeared before publication"
                    )

                os.link(
                    stage.name,
                    destination.name,
                    src_dir_fd=stage_fd,
                    dst_dir_fd=parent_fd,
                    follow_symlinks=False,
                )

                try:
                    os.fsync(parent_fd)
                except Exception:
                    report(
                        "DEBIAN_PUBLICATION_DURABILITY=UNCERTAIN"
                    )
                    raise

            finally:
                os.close(stage_fd)

        report("DEBIAN_ACQUISITION_PUBLISHED=PASS")

    finally:
        os.close(parent_fd)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url")
    parser.add_argument("--approve", action="store_true")
    parser.add_argument("--evidence-dir")
    args = parser.parse_args()

    data = validate_manifest(MANIFEST)

    destination = ROOT / "iso" / data["filename"]
    evidence = Path(
        args.evidence_dir
        or ROOT / "iso/debian-13.7.0-evidence"
    )

    report(
        "DEBIAN_ACQUISITION_RELEASE=" + data["release"]
    )
    report(
        "DEBIAN_ACQUISITION_DESTINATION=" + str(destination)
    )
    report("DEBIAN_ACQUISITION_NETWORK_DEFAULT=DENY")

    if not args.approve:
        report("DEBIAN_ACQUISITION_CONSENT=DENIED")
        return 1

    if not args.url:
        raise ValueError(
            "Explicit official Debian archive URL required"
        )

    report(
        "DEBIAN_ACQUISITION_CONSENT=APPROVED_THIS_INVOCATION"
    )

    acquire(
        args.url,
        destination,
        evidence,
        data["sha256"],
        approved=True,
    )

    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:
        print(
            "DEBIAN_ACQUISITION=FAIL: " + str(error),
            file=sys.stderr,
            flush=True,
        )
        sys.exit(1)
