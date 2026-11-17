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


def trusted_rag_identities(runner=subprocess.run):
    """Resolve RAG peers; never confuse Docker failure with absence."""
    result = runner(
        ["docker", "container", "ls", "-a", "--format", "{{.Names}}"],
        capture_output=True, text=True, check=False, timeout=15,
    )
    if result.returncode != 0 or not isinstance(result.stdout, str):
        raise AuditError("Docker inventory unavailable")

    names = result.stdout.splitlines()
    if len(names) != len(set(names)):
        raise AuditError("Ambiguous Docker inventory")

    webui_id = ""
    tika_id = ""

    if "open-webui" in names:
        result = runner(
            ["docker", "container", "inspect", "--format", "{{.Id}}",
             "open-webui"],
            capture_output=True, text=True, check=False, timeout=15,
        )
        if result.returncode != 0 or not isinstance(result.stdout, str):
            raise AuditError("Open WebUI inspection failed")
        if not result.stdout.endswith(chr(10)):
            raise AuditError("Malformed Open WebUI identity")
        webui_id = validate_identity(result.stdout[:-1])

        result = runner(
            ["/usr/libexec/daia-openwebui-register-container",
             "--verify-identity", webui_id],
            capture_output=True, text=True, check=False, timeout=20,
        )
        if result.returncode != 0 or result.stdout != webui_id + chr(10):
            raise AuditError("Open WebUI registration not trusted")

    if "daia-tika" in names:
        result = runner(
            ["/usr/libexec/daia-tika-register-container",
             "--resolve-registered"],
            capture_output=True, text=True, check=False, timeout=20,
        )
        if result.returncode != 0 or not isinstance(result.stdout, str):
            raise AuditError("Tika registration unavailable")
        if not result.stdout.endswith(chr(10)):
            raise AuditError("Malformed Tika identity")
        tika_id = validate_identity(result.stdout[:-1])

    if webui_id and tika_id and webui_id == tika_id:
        raise AuditError("Duplicate RAG identities")

    return webui_id, tika_id


def authorize(runner=subprocess.run, webui_id="", tika_id=""):
    command = [AUTHORIZER, "runtime"]
    if webui_id:
        command.append(validate_identity(webui_id))
    if tika_id:
        command.extend(["--tika-id", validate_identity(tika_id)])
    result = runner(
        command,
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


def main(expected_id=None, postgres_only=False):
    try:
        if os.geteuid() != 0:
            raise AuditError("Root privileges required")
        validate_identity(expected_id)
        if postgres_only:
            # Startup requires PostgreSQL-only runtime authorization.
            # Full peer authorization remains mandatory after recovery.
            authorized_id = authorize()
        else:
            webui_id, tika_id = trusted_rag_identities()
            authorized_id = authorize(
                webui_id=webui_id, tika_id=tika_id
            )
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
    if len(sys.argv) == 3 and sys.argv[1] == "--postgres-only":
        sys.exit(main(sys.argv[2], postgres_only=True))
    elif len(sys.argv) == 2:
        sys.exit(main(sys.argv[1]))
    else:
        sys.exit(1)
