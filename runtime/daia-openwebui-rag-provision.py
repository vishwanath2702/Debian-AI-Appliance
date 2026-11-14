#!/usr/bin/env python3
"""Fail-closed DAIA Open WebUI PostgreSQL provisioning boundary.

No database mutation is permitted until the offline image's
execution and credential-transport contracts are verified.
"""

import os
import re
import stat
import subprocess
import sys

STATE_DIRECTORY = "/var/lib/daia/pgvector"
SECRET_NAME = "openwebui-password"
AUTHORIZER = "/usr/libexec/daia-pgvector-authorize-start"


class ProvisioningError(Exception):
    pass


def validate_container_id(value):
    if not re.fullmatch(r"[0-9a-f]{64}", value):
        raise ProvisioningError("Invalid authorized container identity")
    return value


def authorize_container(runner=subprocess.run, webui_id=None, tika_id=None):
    command = [AUTHORIZER, "runtime"]
    if webui_id is not None:
        command.append(validate_container_id(webui_id))
    if tika_id is not None:
        command.extend(["--tika-id", validate_container_id(tika_id)])
    result = runner(
        command,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise ProvisioningError("PostgreSQL authorization failed")
    return validate_container_id(result.stdout.strip())


def validate_secret(directory_fd):
    flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC
    fd = os.open(SECRET_NAME, flags, dir_fd=directory_fd)
    try:
        metadata = os.fstat(fd)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != 0
            or metadata.st_gid != 0
            or stat.S_IMODE(metadata.st_mode) != 0o600
            or metadata.st_nlink != 1
            or not 32 <= metadata.st_size <= 256
        ):
            raise ProvisioningError("Unsafe application credential")
        data = os.read(fd, 257)
        if len(data) != metadata.st_size:
            raise ProvisioningError("Application credential changed")
        if not re.fullmatch(rb"[A-Za-z0-9_-]+", data):
            raise ProvisioningError("Invalid application credential format")
        after = os.fstat(fd)
        if (metadata.st_dev, metadata.st_ino, metadata.st_size,
            metadata.st_mtime_ns, metadata.st_ctime_ns) != (
            after.st_dev, after.st_ino, after.st_size,
            after.st_mtime_ns, after.st_ctime_ns):
            raise ProvisioningError("Application credential changed")
        return data
    finally:
        os.close(fd)


def open_state_directory():
    """Open root-owned state by directory descriptors, rejecting symlinks."""
    if STATE_DIRECTORY != "/var/lib/daia/pgvector":
        raise ProvisioningError("Unexpected credential state directory")

    flags = (
        os.O_RDONLY | os.O_DIRECTORY |
        os.O_NOFOLLOW | os.O_CLOEXEC
    )
    fd = os.open("/", flags)
    try:
        for component in ("var", "lib", "daia", "pgvector"):
            metadata = os.fstat(fd)
            if (
                not stat.S_ISDIR(metadata.st_mode)
                or metadata.st_uid != 0
                or metadata.st_gid != 0
                or metadata.st_mode & 0o022
            ):
                raise ProvisioningError(
                    "Untrusted credential directory ancestor"
                )

            next_fd = os.open(component, flags, dir_fd=fd)
            os.close(fd)
            fd = next_fd

        metadata = os.fstat(fd)
        if (
            not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid != 0
            or metadata.st_gid != 0
            or metadata.st_mode & 0o077
        ):
            raise ProvisioningError("Unsafe credential state directory")
        return fd
    except BaseException:
        os.close(fd)
        raise


def validate_application_credential():
    """Return verified secret bytes without loading external Python code."""
    fd = open_state_directory()
    try:
        return validate_secret(fd)
    finally:
        os.close(fd)


def verify_postgres_ready(container_id, runner=subprocess.run):
    """Check readiness using only the authorized immutable container ID."""
    validate_container_id(container_id)
    result = runner(
        [
            "docker", "exec", container_id,
            "pg_isready",
            "-h", "/var/run/postgresql",
            "-U", "daia_rag",
            "-d", "daia_rag",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise ProvisioningError("PostgreSQL is not ready")
    return True


from hashlib import sha256

ROLE_SQL_SHA256 = "03a30f4cb76f3def015f38de5e11550bb70f69e452fa1e069f861e2e80afd644"
ROLE_SQL_STATEMENTS = ('\\set ON_ERROR_STOP on', '\\set ECHO none', '\\set QUIET on', 'BEGIN;', "DO $$ BEGIN IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'daia_openwebui') THEN RAISE EXCEPTION 'DAIA Open WebUI role already exists'; END IF; END $$;", "SELECT format('CREATE ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', 'daia_openwebui', :'app_password') \\gexec", 'GRANT CONNECT ON DATABASE daia_rag TO daia_openwebui;', 'CREATE SCHEMA daia_openwebui AUTHORIZATION daia_openwebui;', 'ALTER ROLE daia_openwebui IN DATABASE daia_rag SET search_path = daia_openwebui, public;', 'COMMIT;')

PGVECTOR_CONNINFO = (
    "host=127.0.0.1 port=5432 dbname=daia_rag user=daia_rag "
    "passfile=/run/secrets/daia-pgvector-pgpass "
    "require_auth=scram-sha-256"
)


def execute_role_sql(container_id, password, statements,
                     runner=subprocess.run):
    """Execute a validated role contract through authenticated psql stdin.

    This function does not authorize its own invocation. The caller must
    first verify the immutable PostgreSQL identity, the secret, and the
    trusted role contract. No credential is passed through argv or env.
    """
    validate_container_id(container_id)

    if (
        not isinstance(password, bytes)
        or not re.fullmatch(rb"[A-Za-z0-9_-]{32,256}", password)
    ):
        raise ProvisioningError("Invalid role credential")

    if (
        not isinstance(statements, tuple)
        or len(statements) < 7
        or statements[:4] != (
            r"\set ON_ERROR_STOP on",
            r"\set ECHO none",
            r"\set QUIET on",
            "BEGIN;",
        )
        or statements[-1] != "COMMIT;"
        or not all(isinstance(line, str) for line in statements)
    ):
        raise ProvisioningError("Invalid role SQL contract")

    actual_digest = sha256(
        "\n".join(statements).encode("utf-8")
    ).hexdigest()
    if actual_digest != ROLE_SQL_SHA256:
        raise ProvisioningError("Unapproved role SQL contract")

    script = (
        "\\set QUIET on\n"
        "\\set ECHO none\n"
        "\\set ON_ERROR_STOP on\n"
        "\\set app_password "
        + password.decode("ascii")
        + "\n"
        + "\n".join(statements)
        + "\n"
    )

    command = [
        "docker", "exec", "-i", container_id,
        "psql", "-X", "-w", "-v", "ON_ERROR_STOP=1",
        "-d", PGVECTOR_CONNINFO,
    ]

    try:
        result = runner(
            command,
            input=script,
            capture_output=True,
            text=True,
            check=False,
            timeout=60,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise ProvisioningError(
            "PostgreSQL role execution unavailable"
        ) from None

    if result.returncode != 0:
        # Never surface psql output: it may contain credential material.
        raise ProvisioningError("PostgreSQL role execution failed")

    return True


ROLE_VERIFY_SQL = """
SELECT CASE WHEN
    EXISTS (
        SELECT 1 FROM pg_roles
        WHERE rolname = 'daia_openwebui'
          AND rolcanlogin
          AND NOT rolsuper
          AND NOT rolcreatedb
          AND NOT rolcreaterole
          AND NOT rolreplication
          AND NOT rolbypassrls
    )
    AND EXISTS (
        SELECT 1
        FROM pg_namespace n
        JOIN pg_roles r ON r.oid = n.nspowner
        WHERE n.nspname = 'daia_openwebui'
          AND r.rolname = 'daia_openwebui'
    )
    AND EXISTS (
        SELECT 1
        FROM pg_db_role_setting s
        JOIN pg_roles r ON r.oid = s.setrole
        JOIN pg_database d ON d.oid = s.setdatabase
        WHERE r.rolname = 'daia_openwebui'
          AND d.datname = 'daia_rag'
          AND 'search_path=daia_openwebui, public' =
              ANY(s.setconfig)
    )
    AND has_database_privilege(
        'daia_openwebui', 'daia_rag', 'CONNECT'
    )
    AND NOT has_schema_privilege(
        'daia_openwebui', 'public', 'CREATE'
    )
THEN 'ok' ELSE 'fail' END;
"""


def verify_role_state(container_id, runner=subprocess.run):
    """Read-only verification of the dedicated database role."""
    validate_container_id(container_id)

    command = [
        "docker", "exec", container_id,
        "psql", "-X", "-w", "-A", "-t",
        "-v", "ON_ERROR_STOP=1",
        "-d", PGVECTOR_CONNINFO,
        "-c", ROLE_VERIFY_SQL,
    ]

    try:
        result = runner(
            command,
            capture_output=True,
            text=True,
            check=False,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired):
        raise ProvisioningError(
            "PostgreSQL role verification unavailable"
        ) from None

    if result.returncode != 0 or result.stdout != "ok\n":
        raise ProvisioningError(
            "PostgreSQL application role contract mismatch"
        )

    return True



def read_role_verifier(container_id, runner=subprocess.run):
    """Read the existing role verifier from authorized PostgreSQL."""
    validate_container_id(container_id)

    sql = (
        "SELECT rolpassword FROM pg_authid "
        "WHERE rolname = 'daia_openwebui';"
    )
    command = [
        "docker", "exec", container_id,
        "psql", "-X", "-w", "-q", "-A", "-t",
        "-v", "ON_ERROR_STOP=1",
        "-d", PGVECTOR_CONNINFO,
        "-c", sql,
    ]

    try:
        result = runner(
            command,
            capture_output=True,
            text=True,
            check=False,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ProvisioningError(
            "PostgreSQL role lookup unavailable"
        ) from error

    if result.returncode != 0 or not isinstance(result.stdout, str):
        raise ProvisioningError("PostgreSQL role lookup failed")

    output = result.stdout
    if output == "":
        return None

    if (
        len(output) > 512
        or not output.endswith("\n")
        or output.count("\n") != 1
    ):
        raise ProvisioningError("Invalid PostgreSQL role lookup result")

    return output[:-1]


def verify_role_password(password, verifier):
    """Verify stored credential against PostgreSQL SCRAM-SHA-256."""
    import base64
    import binascii
    import hashlib
    import hmac

    if (
        not isinstance(password, bytes)
        or re.fullmatch(rb"[A-Za-z0-9_-]{32,256}", password) is None
        or not isinstance(verifier, str)
    ):
        raise ProvisioningError("Invalid role credential proof")

    match = re.fullmatch(
        r"SCRAM-SHA-256\$([0-9]{4,7}):"
        r"([A-Za-z0-9+/]+={0,2})\$"
        r"([A-Za-z0-9+/]+={0,2}):"
        r"([A-Za-z0-9+/]+={0,2})",
        verifier,
    )
    if match is None:
        raise ProvisioningError("Unsupported PostgreSQL role verifier")

    iterations = int(match.group(1))
    if not 4096 <= iterations <= 1000000:
        raise ProvisioningError("Invalid SCRAM iteration count")

    try:
        def decode(value):
            return base64.b64decode(
                value + "=" * (-len(value) % 4),
                validate=True,
            )

        salt = decode(match.group(2))
        expected_stored = decode(match.group(3))
        expected_server = decode(match.group(4))
    except (ValueError, binascii.Error) as error:
        raise ProvisioningError("Malformed SCRAM verifier") from error

    if (
        not 8 <= len(salt) <= 64
        or len(expected_stored) != 32
        or len(expected_server) != 32
    ):
        raise ProvisioningError("Invalid SCRAM verifier dimensions")

    salted = hashlib.pbkdf2_hmac(
        "sha256", password, salt, iterations
    )
    client_key = hmac.new(
        salted, b"Client Key", hashlib.sha256
    ).digest()
    stored_key = hashlib.sha256(client_key).digest()
    server_key = hmac.new(
        salted, b"Server Key", hashlib.sha256
    ).digest()

    if not (
        hmac.compare_digest(stored_key, expected_stored)
        and hmac.compare_digest(server_key, expected_server)
    ):
        raise ProvisioningError(
            "Existing PostgreSQL role credential mismatch"
        )

    return True


def provision(tika_id=None):
    if os.geteuid() != 0:
        raise ProvisioningError("Root privileges required")

    if tika_id is None:
        container_id = authorize_container()
    else:
        container_id = authorize_container(
            tika_id=validate_container_id(tika_id)
        )
    password = validate_application_credential()
    verify_postgres_ready(container_id)

    existing_verifier = read_role_verifier(container_id)

    if existing_verifier is not None:
        # Existing roles require credential and privilege verification.
        verify_role_password(password, existing_verifier)
        verify_role_state(container_id)
        return True

    execute_role_sql(
        container_id, password, ROLE_SQL_STATEMENTS
    )
    verify_role_state(container_id)

    created_verifier = read_role_verifier(container_id)
    verify_role_password(password, created_verifier)
    return True



def verify_existing_role(expected_container_id, webui_id=None, tika_id=None):
    """Read-only proof for one authorized PostgreSQL container.

    Never creates roles, changes credentials, or adopts an unverified
    database identity.
    """
    if os.geteuid() != 0:
        raise ProvisioningError("Root privileges required")

    validate_container_id(expected_container_id)
    if webui_id is not None:
        webui_id = validate_container_id(webui_id)
    if tika_id is not None:
        tika_id = validate_container_id(tika_id)

    if tika_id is not None:
        container_id = authorize_container(
            webui_id=webui_id, tika_id=tika_id
        )
    elif webui_id is not None:
        container_id = authorize_container(webui_id=webui_id)
    else:
        container_id = authorize_container()

    if container_id != expected_container_id:
        raise ProvisioningError(
            "PostgreSQL container identity changed"
        )

    password = validate_application_credential()
    verify_postgres_ready(container_id)

    verifier = read_role_verifier(container_id)
    if verifier is None:
        raise ProvisioningError(
            "Open WebUI database role is missing"
        )

    verify_role_password(password, verifier)
    verify_role_state(container_id)
    return True


def main(argv=None):
    if argv is None:
        argv = sys.argv[1:]

    try:
        if argv == []:
            provision()
        elif len(argv) == 2 and argv[0] == "--tika-id":
            provision(tika_id=validate_container_id(argv[1]))
        elif len(argv) == 2 and argv[0] == "--verify-only":
            verify_existing_role(argv[1])
        elif len(argv) == 3 and argv[0] == "--verify-only":
            verify_existing_role(argv[1], validate_container_id(argv[2]))
        elif (
            len(argv) == 4
            and argv[0] == "--verify-only"
            and argv[2] == "--tika-id"
        ):
            verify_existing_role(
                argv[1], tika_id=validate_container_id(argv[3])
            )
        elif (
            len(argv) == 5
            and argv[0] == "--verify-only"
            and argv[3] == "--tika-id"
        ):
            verify_existing_role(
                argv[1],
                validate_container_id(argv[2]),
                tika_id=validate_container_id(argv[4]),
            )
        else:
            raise ProvisioningError(
                "Unexpected RAG provisioning arguments"
            )
    except (ProvisioningError, OSError) as exc:
        print(
            "DAIA RAG provisioning blocked: " + str(exc),
            file=sys.stderr,
        )
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
