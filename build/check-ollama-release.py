#!/usr/bin/env python3
"""Fail-closed Ollama release and offline archive preflight."""

import argparse
import hashlib
import json
import re
import urllib.request
from pathlib import Path


RELEASE_API = "https://api.github.com/repos/ollama/ollama/releases/latest"
ASSET_NAME = "ollama-linux-amd64.tar.zst"


def pinned_release(fetch_script):
    text = fetch_script.read_text()

    version = re.search(r'^OLLAMA_VERSION="(v[^"]+)"$', text, re.M)
    digest = re.search(r'^OLLAMA_SHA256="([0-9a-f]{64})"$', text, re.M)

    if not version or not digest:
        raise ValueError("Missing or invalid pinned Ollama release metadata")

    return version.group(1), digest.group(1)



def manifest_release(manifest_path):
    manifest = json.loads(manifest_path.read_text())

    if not isinstance(manifest, dict):
        raise ValueError("Invalid Ollama release manifest")

    if manifest.get("schema_version") != 1:
        raise ValueError("Unsupported Ollama manifest schema")

    if manifest.get("component") != "ollama":
        raise ValueError("Unexpected release component")

    if manifest.get("architecture") != "amd64":
        raise ValueError("Unexpected Ollama architecture")

    if manifest.get("asset") != ASSET_NAME:
        raise ValueError("Unexpected Ollama archive name")

    version = manifest.get("version")
    digest = manifest.get("sha256")

    if not isinstance(version, str) or not re.fullmatch(
        r"v[0-9]+\.[0-9]+\.[0-9]+", version
    ):
        raise ValueError("Invalid Ollama manifest version")

    if not isinstance(digest, str) or not re.fullmatch(
        r"[0-9a-f]{64}", digest
    ):
        raise ValueError("Invalid Ollama manifest checksum")

    return version, digest


def verify_archive(directory, expected_digest):
    if not directory.is_dir() or directory.is_symlink():
        raise ValueError("Missing or unsafe Ollama payload directory")

    archives = sorted(directory.glob("*.tar.zst"))

    if len(archives) != 1:
        raise ValueError(
            f"Expected exactly one Ollama archive; found {len(archives)}"
        )

    archive = directory / ASSET_NAME

    if archives[0] != archive or archive.is_symlink() or not archive.is_file():
        raise ValueError("Unexpected or unsafe Ollama archive")

    hasher = hashlib.sha256()

    with archive.open("rb") as stream:
        for chunk in iter(lambda: stream.read(8 * 1024 * 1024), b""):
            hasher.update(chunk)

    if hasher.hexdigest() != expected_digest:
        raise ValueError("Ollama archive SHA256 mismatch")

    print(f"PASS: Verified {archive}")


def verify_latest(version, digest):
    request = urllib.request.Request(
        RELEASE_API,
        headers={
            "Accept": "application/vnd.github+json",
            "User-Agent": "DAIA-Ollama-Release-Preflight",
        },
    )

    with urllib.request.urlopen(request, timeout=20) as response:
        release = json.load(response)

    latest = release.get("tag_name")

    if latest != version:
        raise ValueError(
            f"Pinned Ollama {version} is not latest stable release {latest}"
        )

    assets = [
        asset for asset in release.get("assets", [])
        if asset.get("name") == ASSET_NAME
    ]

    if len(assets) != 1:
        raise ValueError("Official Ollama AMD64 asset missing or ambiguous")

    official_digest = assets[0].get("digest")

    if official_digest != f"sha256:{digest}":
        raise ValueError("Pinned Ollama SHA256 differs from official release")

    print(f"PASS: Official latest stable Ollama release is {version}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--fetch-script", type=Path)
    parser.add_argument("--release-manifest", type=Path)
    parser.add_argument("--archive-directory", type=Path, required=True)
    parser.add_argument(
        "--approved-offline",
        action="store_true",
        help="Skip live release lookup; still verify pinned archive SHA256",
    )
    args = parser.parse_args()

    if args.fetch_script is None and args.release_manifest is None:
        raise ValueError("Release manifest or fetch script is required")

    manifest_pin = (
        manifest_release(args.release_manifest)
        if args.release_manifest is not None
        else None
    )

    script_pin = (
        pinned_release(args.fetch_script)
        if args.fetch_script is not None
        else None
    )

    if manifest_pin is not None and script_pin is not None:
        if manifest_pin != script_pin:
            raise ValueError("Manifest and fetch script release pins differ")

    version, digest = manifest_pin or script_pin

    verify_archive(args.archive_directory, digest)

    if args.approved_offline:
        print(f"OFFLINE APPROVAL: Using pinned Ollama {version}")
    else:
        verify_latest(version, digest)

    print("PASS: Ollama release preflight complete")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        raise SystemExit(f"OLLAMA PREFLIGHT FAILED: {error}")
