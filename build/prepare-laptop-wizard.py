#!/usr/bin/env python3
"""Interactive, read-only Loom laptop preparation guide."""

import argparse
import os
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parent.parent


def heading():
    print()
    print("=" * 52)
    print("             LOOM — LAPTOP BUILD SETUP")
    print("=" * 52)
    print()


def overview():
    print("This wizard prepares the build environment for Pragna.")
    print("It does not install packages, download assets or build an ISO.")
    print()
    print("The existing preparation checks verify:")
    print("  1. Required build tools")
    print("  2. Repository and filesystem permissions")
    print("  3. Offline application archives")
    print("  4. Cargo dependency availability")
    print("  5. Debian ISO integrity and signed provenance")
    print()


def run_checks():
    command = [
        "bash",
        str(ROOT / "build/prepare-laptop.sh"),
    ]
    environment = os.environ.copy()
    environment["DAIA_PREPARE_NONINTERACTIVE"] = "1"
    return subprocess.call(command, cwd=ROOT, env=environment)


def interactive():
    heading()
    overview()

    while True:
        print("Choose an option:")
        print("  1. Run preparation checks")
        print("  2. Explain requirements")
        print("  3. Exit")
        print()

        try:
            choice = input("Selection [1]: ").strip() or "1"
        except (EOFError, KeyboardInterrupt):
            print("\nSetup cancelled.")
            return 130

        if choice == "1":
            print("\nRunning existing read-only checks...\n")
            result = run_checks()
            print()
            print("Preparation result:", "PASS" if result == 0 else "INCOMPLETE")
            return result

        if choice == "2":
            overview()
            continue

        if choice == "3":
            print("No changes made.")
            return 0

        print("Invalid selection.\n")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--noninteractive", action="store_true")
    args = parser.parse_args()

    if args.noninteractive or not sys.stdin.isatty():
        return run_checks()

    return interactive()


if __name__ == "__main__":
    raise SystemExit(main())
