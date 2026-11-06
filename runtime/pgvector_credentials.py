#!/usr/bin/env python3
"""Secure PostgreSQL pgvector credential provisioning for DAIA."""

import fcntl
import os
import stat
from pathlib import Path


class CredentialError(Exception):
    """Credential provisioning cannot proceed safely."""


def validate_state_path(path: str) -> tuple[str, ...]:
    """Validate an absolute, normalized credential directory path."""
    if not path.startswith("/") or path == "/":
        raise CredentialError("Credential directory must be absolute and non-root")

    if "//" in path or path.endswith("/"):
        raise CredentialError("Credential directory is not normalized")

    components = tuple(path[1:].split("/"))

    if any(component in ("", ".", "..") for component in components):
        raise CredentialError("Credential directory contains an unsafe component")

    return components


def open_existing_state_directory(path: str) -> int:
    """Open an existing directory without following directory symlinks."""
    components = validate_state_path(path)

    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    directory_fd = os.open("/", flags)

    try:
        for component in components:
            parent_metadata = os.fstat(directory_fd)
            if (
                parent_metadata.st_uid != 0
                or parent_metadata.st_gid != 0
                or stat.S_IMODE(parent_metadata.st_mode) & 0o022
            ):
                raise CredentialError(
                    "Credential ancestor directory is not trusted"
                )

            next_fd = os.open(component, flags, dir_fd=directory_fd)
            os.close(directory_fd)
            directory_fd = next_fd

        metadata = os.fstat(directory_fd)

        if not stat.S_ISDIR(metadata.st_mode):
            raise CredentialError("Credential state path is not a directory")

        if metadata.st_uid != 0 or metadata.st_gid != 0:
            raise CredentialError("Credential state directory must be root-owned")

        if stat.S_IMODE(metadata.st_mode) != 0o700:
            raise CredentialError("Credential state directory must have mode 0700")

        return directory_fd

    except (OSError, CredentialError):
        os.close(directory_fd)
        raise



def create_state_directory(path: str) -> None:
    """Create only the final directory beneath a trusted parent."""
    components = validate_state_path(path)

    if len(components) < 2:
        raise CredentialError("Credential directory requires a parent")

    parent_path = "/" + "/".join(components[:-1])
    directory_name = components[-1]

    parent_fd = open_existing_parent_directory(parent_path)

    try:
        try:
            os.mkdir(directory_name, 0o700, dir_fd=parent_fd)
        except FileExistsError:
            pass

        directory_fd = open_existing_state_directory(path)
        os.close(directory_fd)
        os.fsync(parent_fd)

    finally:
        os.close(parent_fd)


def open_existing_parent_directory(path: str) -> int:
    """Open a trusted root-owned parent without following symlinks."""
    components = validate_state_path(path)
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    directory_fd = os.open("/", flags)

    try:
        for component in components:
            metadata = os.fstat(directory_fd)

            if (
                metadata.st_uid != 0
                or metadata.st_gid != 0
                or stat.S_IMODE(metadata.st_mode) & 0o022
            ):
                raise CredentialError("Credential ancestor is not trusted")

            next_fd = os.open(component, flags, dir_fd=directory_fd)
            os.close(directory_fd)
            directory_fd = next_fd

        metadata = os.fstat(directory_fd)

        if (
            metadata.st_uid != 0
            or metadata.st_gid != 0
            or stat.S_IMODE(metadata.st_mode) & 0o022
        ):
            raise CredentialError("Credential parent directory is not trusted")

        return directory_fd

    except (OSError, CredentialError):
        os.close(directory_fd)
        raise


def read_existing_credential(directory_fd: int) -> str:
    """Read and validate an existing credential without following symlinks."""
    flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC
    fd = os.open("postgres-password", flags, dir_fd=directory_fd)

    try:
        metadata = os.fstat(fd)

        if not stat.S_ISREG(metadata.st_mode):
            raise CredentialError("Credential must be a regular file")

        if metadata.st_uid != 0 or metadata.st_gid != 0:
            raise CredentialError("Credential must be root-owned")

        if stat.S_IMODE(metadata.st_mode) != 0o600:
            raise CredentialError("Credential permissions must be 0600")

        if metadata.st_nlink != 1:
            raise CredentialError("Credential must not have additional hard links")

        if metadata.st_size not in (64, 65):
            raise CredentialError("Credential size is invalid")

        data = os.read(fd, 66)

        if len(data) != metadata.st_size:
            raise CredentialError("Credential length changed during reading")

        if len(data) == 65:
            if not data.endswith(b"\n"):
                raise CredentialError("Credential terminator is invalid")
            data = data[:-1]

        if len(data) != 64 or any(
            byte not in b"0123456789abcdef" for byte in data
        ):
            raise CredentialError("Credential format is invalid")

        return data.decode("ascii")

    finally:
        os.close(fd)



def format_pgpass_entry(password: str) -> bytes:
    """Format the DAIA PostgreSQL client credential entry."""
    if (
        len(password) != 64
        or any(character not in "0123456789abcdef" for character in password)
    ):
        raise CredentialError("PostgreSQL password format is invalid")

    entry = (
        "127.0.0.1:5432:daia_rag:daia_rag:"
        + password
    )

    return entry.encode("ascii") + bytes([10])


def _create_or_reuse_credential_locked(directory_fd: int) -> str:
    """Publish a new credential without replacing an existing one."""
    import secrets

    try:
        return read_existing_credential(directory_fd)
    except FileNotFoundError:
        pass

    temporary_name = ".postgres-password-" + secrets.token_hex(16)
    flags = (
        os.O_WRONLY | os.O_CREAT | os.O_EXCL |
        os.O_NOFOLLOW | os.O_CLOEXEC
    )
    temporary_fd = os.open(
        temporary_name, flags, 0o600, dir_fd=directory_fd
    )

    try:
        metadata = os.fstat(temporary_fd)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != 0
            or metadata.st_gid != 0
            or stat.S_IMODE(metadata.st_mode) != 0o600
            or metadata.st_nlink != 1
        ):
            raise CredentialError("Temporary credential metadata is unsafe")

        password = secrets.token_hex(32).encode("ascii")
        data = password + b"\n"

        with os.fdopen(os.dup(temporary_fd), "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())

        try:
            os.link(
                temporary_name,
                "postgres-password",
                src_dir_fd=directory_fd,
                dst_dir_fd=directory_fd,
                follow_symlinks=False,
            )
        except FileExistsError:
            return read_existing_credential(directory_fd)

        os.unlink(temporary_name, dir_fd=directory_fd)
        os.fsync(directory_fd)
        return read_existing_credential(directory_fd)

    finally:
        os.close(temporary_fd)
        try:
            os.unlink(temporary_name, dir_fd=directory_fd)
        except FileNotFoundError:
            pass



def create_or_reuse_credential(directory_fd: int) -> str:
    """Create or reuse a credential under an exclusive directory lock."""
    fcntl.flock(directory_fd, fcntl.LOCK_EX)
    try:
        return _create_or_reuse_credential_locked(directory_fd)
    finally:
        fcntl.flock(directory_fd, fcntl.LOCK_UN)


def publish_pgpass_file(directory_fd: int) -> None:
    """Publish or verify a PostgreSQL client credential file."""
    import secrets

    password = read_existing_credential(directory_fd)
    expected = format_pgpass_entry(password)

    def verify_existing() -> None:
        flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC
        fd = os.open("pgpass", flags, dir_fd=directory_fd)
        try:
            metadata = os.fstat(fd)
            if (
                not stat.S_ISREG(metadata.st_mode)
                or metadata.st_uid != 0
                or metadata.st_gid != 0
                or stat.S_IMODE(metadata.st_mode) != 0o600
                or metadata.st_nlink != 1
                or metadata.st_size != len(expected)
            ):
                raise CredentialError("PGPASS file metadata is unsafe")

            if os.read(fd, len(expected) + 1) != expected:
                raise CredentialError("PGPASS file contents do not match")
        finally:
            os.close(fd)

    try:
        verify_existing()
        return
    except FileNotFoundError:
        pass

    temporary_name = ".pgpass-" + secrets.token_hex(16)
    flags = (
        os.O_WRONLY | os.O_CREAT | os.O_EXCL |
        os.O_NOFOLLOW | os.O_CLOEXEC
    )
    temporary_fd = os.open(
        temporary_name, flags, 0o600, dir_fd=directory_fd
    )

    try:
        metadata = os.fstat(temporary_fd)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != 0
            or metadata.st_gid != 0
            or stat.S_IMODE(metadata.st_mode) != 0o600
            or metadata.st_nlink != 1
        ):
            raise CredentialError("Temporary PGPASS metadata is unsafe")

        with os.fdopen(os.dup(temporary_fd), "wb") as stream:
            stream.write(expected)
            stream.flush()
            os.fsync(stream.fileno())

        try:
            os.link(
                temporary_name,
                "pgpass",
                src_dir_fd=directory_fd,
                dst_dir_fd=directory_fd,
                follow_symlinks=False,
            )
        except FileExistsError:
            verify_existing()
            return

        os.unlink(temporary_name, dir_fd=directory_fd)
        os.fsync(directory_fd)
        verify_existing()

    finally:
        os.close(temporary_fd)
        try:
            os.unlink(temporary_name, dir_fd=directory_fd)
        except FileNotFoundError:
            pass



def provision_credentials(directory_fd: int) -> None:
    """Serialize password and PGPASS publication."""
    fcntl.flock(directory_fd, fcntl.LOCK_EX)
    try:
        _create_or_reuse_credential_locked(directory_fd)
        publish_pgpass_file(directory_fd)
    finally:
        fcntl.flock(directory_fd, fcntl.LOCK_UN)


def main() -> int:
    """Provision or reuse a credential without exposing its value."""
    import argparse

    parser = argparse.ArgumentParser(
        description="DAIA pgvector credential provisioning"
    )
    parser.add_argument("--state-directory", required=True)
    args = parser.parse_args()

    if os.geteuid() != 0:
        raise CredentialError("Credential provisioning requires root")

    create_state_directory(args.state_directory)
    directory_fd = open_existing_state_directory(args.state_directory)
    try:
        provision_credentials(directory_fd)
    finally:
        os.close(directory_fd)

    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except CredentialError as error:
        print(f"DAIA pgvector credential error: {error}", file=__import__("sys").stderr)
        raise SystemExit(1)
