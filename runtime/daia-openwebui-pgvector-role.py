#!/usr/bin/env python3
"""Fixed PostgreSQL application-role provisioning contract.

Produces SQL only. Does not connect to PostgreSQL or read credentials.
"""

ROLE = "daia_openwebui"
DATABASE = "daia_rag"
SCHEMA = "daia_openwebui"


def role_sql():
    """Return the exact approved psql provisioning script."""
    return (
        r"\set ON_ERROR_STOP on",
        r"\set ECHO none",
        r"\set QUIET on",
        "BEGIN;",
        "DO $$ BEGIN "
        "IF EXISTS (SELECT 1 FROM pg_roles "
        "WHERE rolname = 'daia_openwebui') THEN "
        "RAISE EXCEPTION 'DAIA Open WebUI role already exists'; "
        "END IF; END $$;",
        "SELECT format("
        "'CREATE ROLE %I LOGIN NOSUPERUSER NOCREATEDB "
        "NOCREATEROLE NOREPLICATION PASSWORD %L', "
        "'daia_openwebui', :'app_password') "
        r"\gexec",
        "GRANT CONNECT ON DATABASE daia_rag TO daia_openwebui;",
        "CREATE SCHEMA daia_openwebui AUTHORIZATION daia_openwebui;",
        "ALTER ROLE daia_openwebui IN DATABASE daia_rag "
        "SET search_path = daia_openwebui, public;",
        "COMMIT;",
    )


def validate_role_contract():
    statements = role_sql()

    expected = (
        r"\set ON_ERROR_STOP on",
        r"\set ECHO none",
        r"\set QUIET on",
        "BEGIN;",
        "DO $$ BEGIN "
        "IF EXISTS (SELECT 1 FROM pg_roles "
        "WHERE rolname = 'daia_openwebui') THEN "
        "RAISE EXCEPTION 'DAIA Open WebUI role already exists'; "
        "END IF; END $$;",
        "SELECT format("
        "'CREATE ROLE %I LOGIN NOSUPERUSER NOCREATEDB "
        "NOCREATEROLE NOREPLICATION PASSWORD %L', "
        "'daia_openwebui', :'app_password') "
        r"\gexec",
        "GRANT CONNECT ON DATABASE daia_rag TO daia_openwebui;",
        "CREATE SCHEMA daia_openwebui AUTHORIZATION daia_openwebui;",
        "ALTER ROLE daia_openwebui IN DATABASE daia_rag "
        "SET search_path = daia_openwebui, public;",
        "COMMIT;",
    )

    if statements != expected:
        raise ValueError("Unexpected PostgreSQL role contract")

    return statements


if __name__ == "__main__":
    validate_role_contract()
