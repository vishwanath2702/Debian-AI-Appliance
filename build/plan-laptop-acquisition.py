#!/usr/bin/env python3
"""Read-only, fail-closed laptop acquisition inventory."""

import argparse
import json
from pathlib import Path
import re
import sys

METHODS = {
    "debian": "official_signed_release",
    "ollama": "pinned_release",
    "open-webui": "none",
    "pgvector": "pinned_oci",
    "tika": "pinned_oci",
    "whichllm": "none",
}

BLOCKED = {"open-webui", "whichllm"}

POLICY = {
    "schema_version": 1,
    "profile": "laptop",
    "architecture": "amd64",
    "automatic_upgrade": False,
    "network_default": "deny",
    "download_consent": "explicit_per_invocation",
    "installation_consent": "explicit_per_invocation",
    "acquisition_execution": "not_implemented",
}


def load_manifest(path):
    if path.is_symlink() or not path.is_file():
        raise ValueError("Missing or unsafe acquisition manifest")

    data = json.loads(path.read_text(encoding="utf-8"))

    if not isinstance(data, dict):
        raise ValueError("Invalid acquisition manifest")

    for key, expected in POLICY.items():
        actual = data.get(key)
        if type(actual) is not type(expected) or actual != expected:
            raise ValueError("Unsafe policy: " + key)

    assets = data.get("assets")
    if not isinstance(assets, list) or len(assets) != len(METHODS):
        raise ValueError("Unexpected asset inventory")

    seen = set()

    for asset in assets:
        if not isinstance(asset, dict):
            raise ValueError("Invalid asset")

        name = asset.get("id")
        if name not in METHODS or name in seen:
            raise ValueError("Unknown or duplicate asset")

        seen.add(name)

        if asset.get("method") != METHODS[name]:
            raise ValueError("Unsafe acquisition method: " + name)

        expected_status = (
            "blocked_unresolved_provenance"
            if name in BLOCKED
            else "verification_defined"
        )
        if asset.get("status") != expected_status:
            raise ValueError("Unsafe asset status: " + name)

        policy = asset.get("policy")
        if not isinstance(policy, str) or not re.fullmatch(
            r"(build|payload/config)/[A-Za-z0-9._/-]+", policy
        ):
            raise ValueError("Unsafe policy path")

    if seen != set(METHODS):
        raise ValueError("Incomplete inventory")

    return data


def config_values(path):
    if path.is_symlink() or not path.is_file():
        raise ValueError("Missing or unsafe Pragna configuration")

    values = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        match = re.fullmatch(
            r'([A-Z][A-Z0-9_]*)="([^"]*)"', line
        )
        if match:
            key, value = match.groups()
            if key in values:
                raise ValueError("Duplicate configuration key: " + key)
            values[key] = value

    return values


def safe_path(root, relative):
    if not isinstance(relative, str) or not relative:
        raise ValueError("Invalid asset destination")

    path = Path(relative)
    if path.is_absolute() or ".." in path.parts:
        raise ValueError("Unsafe asset destination")

    current = root
    for part in path.parts:
        current = current / part
        if current.is_symlink():
            raise ValueError("Symlink in asset destination")

    return current


def plan(root, manifest):
    config = config_values(root / "payload/config/pragna.conf")
    results = []

    for asset in manifest["assets"]:
        name = asset["id"]

        if "destination" in asset:
            relative = asset["destination"]
        else:
            key = asset.get("destination_key")
            if not isinstance(key, str) or key not in config:
                raise ValueError("Missing destination: " + name)
            relative = config[key]

        path = safe_path(root, relative)

        if name == "whichllm" and path.is_dir():
            presence = "directory_present_unverified"
            action = "VERIFY_EXISTING_DIRECTORY"
        elif path.is_file():
            presence = "present_unverified"
            action = "VERIFY_EXISTING_ASSET"
        elif path.exists():
            presence = "unexpected_path_type"
            action = "STOP_UNSAFE_ASSET_PATH"
        else:
            presence = "missing"
            action = (
                "BLOCKED_UNRESOLVED_PROVENANCE"
                if name in BLOCKED
                else "APPROVAL_REQUIRED_BEFORE_ACQUISITION"
            )

        results.append({
            "id": name,
            "presence": presence,
            "method": asset["method"],
            "action": action,
            "acquisition_allowed_now": False,
        })

    return results


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
    )
    parser.add_argument("--manifest", type=Path)
    args = parser.parse_args()

    root = args.root.resolve()
    manifest_path = (
        args.manifest
        if args.manifest is not None
        else root / "build/laptop-acquisition.json"
    )

    manifest = load_manifest(manifest_path)
    entries = plan(root, manifest)

    print("ACQUISITION_PLANNER=READ_ONLY")
    print("ACQUISITION_EXECUTION=NONE")
    print("NETWORK_ACCESS=NONE")

    for entry in entries:
        print(
            "ACQUISITION_ASSET={id} PRESENCE={presence} "
            "METHOD={method} ACTION={action}".format(**entry),
            flush=True,
        )

    print("ACQUISITION_PLAN_STATUS=PASS")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        raise SystemExit(
            "ACQUISITION_PLAN_STATUS=FAIL: " + str(error)
        )
