#!/usr/bin/env python3
"""Fail-closed, read-only laptop build permissions preflight."""

import argparse
import os
from pathlib import Path
import stat
import sys

READ_DIRECTORIES = (
    ".",
    "engine",
    "payload",
    "payload/config",
)

WRITE_DIRECTORIES = (
    "build",
    "build/assets",
    "iso",
    "payload/packages",
    "payload/packages/ollama",
    "payload/packages/docker",
    "payload/images",
    "output",
)

READ_SCRIPTS = (
    "build/plan-laptop-acquisition.py",
    "build/check-debian-release.py",
    "build/check-debian-signature.py",
    "build/check-ollama-release.py",
    "build/fetch-oci-image.py",
)

EXECUTABLE_SCRIPTS = (
    "builder.sh",
    "build/build.sh",
    "build/prepare-laptop.sh",
    "build/fetch-ollama.sh",
)

ASSETS = (
    "iso/debian-13.7.0-amd64-netinst.iso",
    "payload/packages/ollama/ollama-linux-amd64.tar.zst",
    "payload/images/open-webui-v0.13.0-slim.tar",
    "payload/images/pgvector-pg17.tar",
    "payload/images/tika-3.3.3.0.tar",
)

OPTIONAL_DIRECTORIES = (
    "registry",
)

def inspect(root, relative, kind, required=True):
    path = root / relative

    current = root
    if root.is_symlink():
        raise ValueError("Repository root is a symlink")

    for part in Path(relative).parts:
        if part in ("..", ""):
            raise ValueError("Unsafe relative path")
        current = current / part
        if current.is_symlink():
            raise ValueError("Symlink: " + relative)

    if not path.exists():
        if required:
            raise ValueError("Missing required path: " + relative)
        print("PERMISSION_OPTIONAL_MISSING=" + relative, flush=True)
        return

    info = path.stat()
    mode = stat.S_IMODE(info.st_mode)

    if kind == "directory":
        if not path.is_dir():
            raise ValueError("Expected directory: " + relative)
        if not os.access(path, os.R_OK | os.X_OK):
            raise ValueError("Directory inaccessible: " + relative)

    elif kind == "writable_directory":
        if not path.is_dir():
            raise ValueError("Expected writable directory: " + relative)
        if not os.access(path, os.R_OK | os.W_OK | os.X_OK):
            raise ValueError("Directory not writable: " + relative)

    elif kind == "script":
        if not path.is_file() or not os.access(path, os.R_OK):
            raise ValueError("Script unreadable: " + relative)

    elif kind == "executable_script":
        if (
            not path.is_file()
            or not os.access(path, os.R_OK | os.X_OK)
        ):
            raise ValueError("Script not executable: " + relative)

    elif kind == "asset":
        if not path.is_file() or not os.access(path, os.R_OK):
            raise ValueError("Asset unreadable: " + relative)

    if mode & 0o002:
        raise ValueError("World-writable path: " + relative)

    print(
        "PERMISSION_CHECK=PASS "
        + relative
        + " MODE="
        + format(mode, "04o"),
        flush=True,
    )


def check(root, mode="build"):
    if not root.is_dir() or root.is_symlink():
        raise ValueError("Unsafe repository root")

    for relative in READ_DIRECTORIES:
        inspect(root, relative, "directory")

    for relative in WRITE_DIRECTORIES:
        inspect(root, relative, "writable_directory")

    for relative in READ_SCRIPTS:
        inspect(root, relative, "script")

    for relative in EXECUTABLE_SCRIPTS:
        inspect(root, relative, "executable_script")

    for relative in ASSETS:
        inspect(
            root,
            relative,
            "asset",
            required=(mode == "build"),
        )

    for relative in OPTIONAL_DIRECTORIES:
        inspect(root, relative, "directory", required=False)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
    )
    parser.add_argument(
        "--mode",
        choices=("prepare", "build"),
        default="build",
    )
    args = parser.parse_args()

    print("PERMISSION_PREFLIGHT=READ_ONLY", flush=True)
    print("PERMISSION_MODE=" + args.mode, flush=True)
    print("PERMISSION_MUTATIONS=NONE", flush=True)

    check(args.root, mode=args.mode)
    print("PERMISSION_PREFLIGHT_STATUS=PASS", flush=True)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        raise SystemExit(
            "PERMISSION_PREFLIGHT_STATUS=FAIL: " + str(error)
        )
