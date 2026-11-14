#!/usr/bin/env python3
"""Read-only PostgreSQL authentication audit for an authorized container.

Checks the current pg_hba.conf text and a SCRAM-authenticated TCP
connection. Does not alter PostgreSQL or prove historical configuration.
"""

import os
import re
import subprocess
import sys

AUTHORIZER = "/usr/libexec/daia-pgvector-authorize-start"
IDENTITY = re.compile(r"[0-9a-f]{64}\Z")

CONNINFO = (
    "host=127.0.0.1 port=5432 dbname=daia_rag user=daia_rag "
    "passfile=/run/secrets/daia-pgvector-pgpass "
    "require_auth=scram-sha-256"
)

# Reject weak rules, parsing errors, included files and a primary
# HBA file changed since the last configuration load.
QUERY = """SELECT CASE WHEN
    EXISTS (SELECT 1 FROM pg_hba_file_rules)
    AND NOT EXISTS (
        SELECT 1 FROM pg_hba_file_rules
        WHERE error IS NOT NULL
           OR file_name IS DISTINCT FROM current_setting('hba_file')
           OR auth_method IS NULL
           OR auth_method NOT IN ('scram-sha-256', 'reject')
    )
    AND EXISTS (
        SELECT 1 FROM pg_hba_file_rules
        WHERE type = 'local' AND auth_method = 'scram-sha-256'
    )
    AND EXISTS (
        SELECT 1 FROM pg_hba_file_rules
        WHERE type = 'host' AND auth_method = 'scram-sha-256'
    )
    AND pg_conf_load_time() >=
        (pg_stat_file(current_setting('hba_file'))).modification
THEN 'DAIA_SCRAM_OK' ELSE 'DAIA_SCRAM_DENY' END;
"""


class AuditError(Exception):
    pass


def validate_identity(value):
    if not isinstance(value, str) or not IDENTITY.fullmatch(value):
        raise AuditError("Invalid container identity")
    return value


def authorize(runner=subprocess.run):
    result = runner(
        [AUTHORIZER, "runtime"],
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )
    if result.returncode != 0 or not isinstance(result.stdout, str):
        raise AuditError("Container authorization failed")
    if not result.stdout.endswith("\n"):
        raise AuditError("Malformed authorization output")
    return validate_identity(result.stdout[:-1])


def audit(container_id, runner=subprocess.run):
    validate_identity(container_id)

    command = [
        "docker", "exec", container_id,
        "psql", "-X", "-w", "-A", "-t",
        "-v", "ON_ERROR_STOP=1",
        "-d", CONNINFO,
        "-c", QUERY,
    ]

    result = runner(
        command,
        capture_output=True,
        text=True,
        check=False,
        timeout=45,
    )

    if result.returncode != 0 or result.stdout != "DAIA_SCRAM_OK\n":
        raise AuditError(
            "PostgreSQL authentication contract not verified"
        )
    return True


def main(expected_id=None):
    try:
        if os.geteuid() != 0:
            raise AuditError("Root privileges required")
        validate_identity(expected_id)
        authorized_id = authorize()
        if authorized_id != expected_id:
            raise AuditError(
                "Authorized container identity changed"
            )
        audit(authorized_id)
    except (AuditError, OSError, subprocess.TimeoutExpired) as exc:
        print(
            "DAIA pgvector auth audit blocked: " + type(exc).__name__,
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) == 2 else None))
