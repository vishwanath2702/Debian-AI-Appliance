#!/usr/bin/env python3
"""Register an inspected DAIA pgvector container identity."""

import os
import sys

from pgvector_registration import (
    publish_trusted_registration,
    validate_container_id,
)

STATE_DIRECTORY = "/var/lib/daia/pgvector"


def main() -> int:
    if os.geteuid() != 0:
        raise PermissionError("Container registration requires root")

    if len(sys.argv) != 2:
        raise ValueError("Expected exactly one container ID")

    container_id = validate_container_id(sys.argv[1])

    publish_trusted_registration(
        STATE_DIRECTORY,
        container_id,
    )

    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, PermissionError, ValueError) as error:
        print(
            f"DAIA pgvector registration failed: {error}",
            file=sys.stderr,
        )
        raise SystemExit(1)
