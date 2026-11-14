#!/usr/bin/env python3
"""Provision the separate Open WebUI pgvector credential."""

import os
import secrets
import stat
from pathlib import Path

STATE_DIRECTORY = Path("/var/lib/daia/pgvector")
SECRET_NAME = "openwebui-password"


def open_existing_state_directory(path):
    """Open an existing root-owned directory without following symlinks."""
    if (
        not isinstance(path, str)
        or not path.startswith("/")
        or path == "/"
        or path.endswith("/")
        or "//" in path
        or any(part in ("", ".", "..") for part in path.split("/")[1:])
    ):
        raise ValueError("Invalid credential directory path")

    flags = (
        os.O_RDONLY | os.O_DIRECTORY |
        os.O_NOFOLLOW | os.O_CLOEXEC
    )
    fd = os.open("/", flags)
    try:
        for component in path.split("/")[1:]:
            metadata = os.fstat(fd)
            if (
                not stat.S_ISDIR(metadata.st_mode)
                or metadata.st_uid != 0
                or metadata.st_gid != 0
                or metadata.st_mode & 0o022
            ):
                raise PermissionError("Untrusted credential directory parent")

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
            raise PermissionError("Unsafe credential directory")

        return fd
    except BaseException:
        os.close(fd)
        raise


def provision(directory=STATE_DIRECTORY):
    """Create the application secret without following directory symlinks."""
    directory_path = os.fspath(directory)
    directory_fd = open_existing_state_directory(directory_path)
    directory = Path(directory_path)

    try:
        metadata = os.fstat(directory_fd)
        if not stat.S_ISDIR(metadata.st_mode):
            raise ValueError("Invalid credential directory")
        if metadata.st_uid != 0:
            raise PermissionError("Credential directory must be root-owned")
        if metadata.st_mode & 0o077:
            raise PermissionError("Credential directory is too permissive")

        password = secrets.token_urlsafe(48).encode("ascii")
        fd = os.open(
            SECRET_NAME,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL |
            os.O_NOFOLLOW | os.O_CLOEXEC,
            0o600,
            dir_fd=directory_fd,
        )

        published = False
        try:
            os.fchmod(fd, 0o600)
            with os.fdopen(fd, "wb", closefd=False) as secret:
                secret.write(password)
                secret.flush()
                os.fsync(fd)
            os.fsync(directory_fd)
            published = True
            return directory / SECRET_NAME
        finally:
            os.close(fd)
            if not published:
                os.unlink(SECRET_NAME, dir_fd=directory_fd)
    finally:
        os.close(directory_fd)



def read_existing_credential(directory=STATE_DIRECTORY):
    """Validate an existing root-owned application credential."""
    import re

    directory_fd = open_existing_state_directory(os.fspath(directory))
    try:
        fd = os.open(
            SECRET_NAME,
            os.O_RDONLY | os.O_NOFOLLOW |
            os.O_CLOEXEC | os.O_NONBLOCK,
            dir_fd=directory_fd,
        )
        try:
            before = os.fstat(fd)
            if (
                not stat.S_ISREG(before.st_mode)
                or before.st_uid != 0
                or before.st_gid != 0
                or stat.S_IMODE(before.st_mode) != 0o600
                or before.st_nlink != 1
                or not 32 <= before.st_size <= 256
            ):
                raise PermissionError("Unsafe existing application credential")

            data = os.read(fd, 257)
            after = os.fstat(fd)

            identity = lambda item: (
                item.st_dev, item.st_ino, item.st_size,
                item.st_mtime_ns, item.st_ctime_ns
            )
            if identity(before) != identity(after):
                raise PermissionError("Application credential changed")

            if (
                len(data) != before.st_size
                or re.fullmatch(rb"[A-Za-z0-9_-]{32,256}", data) is None
            ):
                raise ValueError("Invalid existing application credential")

            return data
        finally:
            os.close(fd)
    finally:
        os.close(directory_fd)


def ensure_credential(directory=STATE_DIRECTORY):
    """Create once or reuse only a validated existing credential."""
    if os.geteuid() != 0:
        raise PermissionError("Application credential requires root")

    try:
        read_existing_credential(directory)
    except FileNotFoundError:
        # Creation is exclusive. A concurrent creator causes a safe failure.
        provision(directory)
        read_existing_credential(directory)

    return Path(directory) / SECRET_NAME


if __name__ == "__main__":
    ensure_credential()
