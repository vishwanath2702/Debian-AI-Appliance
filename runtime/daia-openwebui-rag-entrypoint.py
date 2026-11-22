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




def ensure_webui_secret_file():
    """Create or load the persistent Open WebUI authentication secret."""
    import secrets
    import stat

    if os.environ.get("WEBUI_SECRET_KEY") or os.environ.get(
        "WEBUI_JWT_SECRET_KEY"
    ):
        return

    key_path = os.environ.get(
        "WEBUI_SECRET_KEY_FILE",
        "/app/backend/data/.webui_secret_key",
    )

    if not os.path.isabs(key_path):
        raise ValueError("Open WebUI secret path must be absolute")

    parent, filename = os.path.split(key_path)

    if filename in ("", ".", ".."):
        raise ValueError("Invalid Open WebUI secret filename")

    directory_flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
    directory_flags |= getattr(os, "O_CLOEXEC", 0)

    directory_fd = os.open(parent, directory_flags)

    try:
        directory_info = os.fstat(directory_fd)

        if not stat.S_ISDIR(directory_info.st_mode):
            raise ValueError("Secret parent must be a directory")

        if directory_info.st_uid != os.geteuid():
            raise PermissionError("Secret parent ownership mismatch")

        # The secret directory must not be writable by other users.
        # Mode 0755 is acceptable; group/other write access is not.
        if stat.S_IMODE(directory_info.st_mode) & 0o022:
            raise PermissionError(
                "Secret parent directory is group/other writable"
            )

        # Publish a complete, synced secret without replacing an
        # existing secret. Concurrent starters may race safely.
        try:
            os.stat(filename, dir_fd=directory_fd, follow_symlinks=False)
        except FileNotFoundError:
            temporary_name = (
                "." + filename + ".tmp-" + secrets.token_hex(16)
            )
            create_flags = (
                os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW
            )
            create_flags |= getattr(os, "O_CLOEXEC", 0)

            fd = os.open(
                temporary_name,
                create_flags,
                0o600,
                dir_fd=directory_fd,
            )
            temporary_info = os.fstat(fd)

            try:
                # Enforce the mode on the opened inode, not its path.
                os.fchmod(fd, 0o600)
                value = secrets.token_urlsafe(48).encode("ascii")
                with os.fdopen(fd, "wb", closefd=True) as stream:
                    stream.write(value)
                    stream.flush()
                    os.fsync(stream.fileno())

                try:
                    os.link(
                        temporary_name,
                        filename,
                        src_dir_fd=directory_fd,
                        dst_dir_fd=directory_fd,
                        follow_symlinks=False,
                    )
                    os.fsync(directory_fd)
                except FileExistsError:
                    # Another startup published first. Validate its
                    # completed secret using the normal read path.
                    pass
            finally:
                # Remove only the temporary inode created by us.
                try:
                    current = os.stat(
                        temporary_name,
                        dir_fd=directory_fd,
                        follow_symlinks=False,
                    )
                    if (
                        current.st_dev == temporary_info.st_dev
                        and current.st_ino == temporary_info.st_ino
                    ):
                        os.unlink(
                            temporary_name,
                            dir_fd=directory_fd,
                        )
                except FileNotFoundError:
                    pass

        read_flags = os.O_RDONLY | os.O_NOFOLLOW
        read_flags |= getattr(os, "O_CLOEXEC", 0)

        fd = os.open(filename, read_flags, dir_fd=directory_fd)

        try:
            info = os.fstat(fd)

            if not stat.S_ISREG(info.st_mode):
                raise ValueError("Secret must be a regular file")

            if info.st_uid != os.geteuid():
                raise PermissionError("Secret ownership mismatch")

            if stat.S_IMODE(info.st_mode) != 0o600:
                raise PermissionError("Secret permissions must be 0600")

            if info.st_size < 32 or info.st_size > 4096:
                raise ValueError("Secret length invalid")

            value = os.read(fd, 4097)

            if len(value) != info.st_size:
                raise ValueError("Secret changed during read")

            secret = value.decode("ascii")

            if not secret or any(
                character.isspace() for character in secret
            ):
                raise ValueError("Invalid secret content")

        finally:
            os.close(fd)

        os.environ["WEBUI_SECRET_KEY"] = secret

    finally:
        os.close(directory_fd)


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

    # Keep the generated WebUI authentication secret in the
    # persistent application data volume across container recreation.
    # Respect an explicitly supplied secret-file override.
    os.environ.setdefault(
        "WEBUI_SECRET_KEY_FILE",
        "/app/backend/data/.webui_secret_key",
    )
    ensure_webui_secret_file()

    os.chdir("/app/backend")
    os.execvp("bash", ["bash", "start.sh"])


if __name__ == "__main__":
    sys.exit(main())
