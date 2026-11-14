#!/usr/bin/env python3
"""Initialize pgvector in an authorized PostgreSQL container."""

import os
import re
import subprocess
import sys

AUTHORIZER = "/usr/libexec/daia-pgvector-authorize-start"
IDENTITY = re.compile(r"[0-9a-f]{64}\Z")
SQL = "CREATE EXTENSION IF NOT EXISTS vector WITH SCHEMA public;\n"
CONNINFO = (
    "host=127.0.0.1 port=5432 dbname=daia_rag user=daia_rag "
    "passfile=/run/secrets/daia-pgvector-pgpass "
    "require_auth=scram-sha-256"
)


class InitializationError(Exception):
    pass


def validate_identity(value):
    if not isinstance(value, str) or not IDENTITY.fullmatch(value):
        raise InitializationError("Invalid PostgreSQL container identity")
    return value


def authorize(runner=subprocess.run):
    result = runner(
        [AUTHORIZER, "runtime"],
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )
    if result.returncode != 0:
        raise InitializationError("PostgreSQL authorization failed")
    output = result.stdout
    if not isinstance(output, str) or not output.endswith("\n"):
        raise InitializationError("Malformed authorization output")
    return validate_identity(output.removesuffix("\n").removesuffix("\r"))


def initialize(container_id, runner=subprocess.run):
    validate_identity(container_id)

    command = [
        "docker", "exec", "-i", container_id,
        "psql", "-X", "-w", "-v", "ON_ERROR_STOP=1",
        "-d", CONNINFO,
    ]

    result = runner(
        command,
        input=SQL,
        capture_output=True,
        text=True,
        check=False,
        timeout=60,
    )
    if result.returncode != 0:
        raise InitializationError("pgvector extension initialization failed")

    verification = runner(
        [
            "docker", "exec", container_id,
            "psql", "-X", "-w", "-A", "-t",
            "-v", "ON_ERROR_STOP=1",
            "-d", CONNINFO,
            "-c", "SELECT extname FROM pg_extension WHERE extname = 'vector'",
        ],
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )
    if verification.returncode != 0 or verification.stdout.strip() != "vector":
        raise InitializationError("pgvector extension verification failed")
    return True


def main(expected_id=None):
    try:
        if os.geteuid() != 0:
            raise InitializationError("Root privileges required")
        validate_identity(expected_id)
        authorized_id = authorize()
        if authorized_id != expected_id:
            raise InitializationError(
                "Authorized container identity changed"
            )
        initialize(authorized_id)
    except (InitializationError, OSError, subprocess.TimeoutExpired) as exc:
        print(
            "DAIA pgvector initialization blocked: "
            + type(exc).__name__,
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) == 2 else None))
