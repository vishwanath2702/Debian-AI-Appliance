#!/usr/bin/env python3
"""Launch Open WebUI with a credential-safe pgvector connection."""

import os
import sys
from pathlib import Path
from urllib.parse import quote

# Separate application credential: never reuse PostgreSQL bootstrap secrets.
SECRET_PATH = Path("/run/secrets/daia-openwebui-pgvector-password")
PGVECTOR_HOST = "daia-pgvector"
PGVECTOR_PORT = 5432
PGVECTOR_DATABASE = "daia_rag"
PGVECTOR_USER = "daia_openwebui"


def read_password(path=SECRET_PATH):
    with open(path, "rb") as secret:
        password = secret.read()

    if not password or len(password) > 4096:
        raise ValueError("Invalid pgvector credential length")

    if b"\\n" in password or b"\\r" in password or b"\\x00" in password:
        raise ValueError("Invalid pgvector credential encoding")

    return password.decode("utf-8")


def build_pgvector_url(password):
    encoded = quote(password, safe="")
    return (
        f"postgresql://{PGVECTOR_USER}:{encoded}@"
        f"{PGVECTOR_HOST}:{PGVECTOR_PORT}/{PGVECTOR_DATABASE}"
    )


def main():
    try:
        password = read_password()
        url = build_pgvector_url(password)
    except (OSError, UnicodeError, ValueError) as error:
        print(
            f"Open WebUI pgvector credential unavailable: "
            f"{type(error).__name__}",
            file=sys.stderr,
        )
        return 1

    os.environ["VECTOR_DB"] = "pgvector"
    os.environ["PGVECTOR_DB_URL"] = url

    # Preserve the application's SQLite database configuration.
    os.environ.setdefault(
        "DATABASE_URL",
        "sqlite:////app/backend/data/webui.db",
    )

    os.chdir("/app/backend")
    os.execvp("bash", ["bash", "start.sh"])


if __name__ == "__main__":
    sys.exit(main())
