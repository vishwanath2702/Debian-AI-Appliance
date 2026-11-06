#!/usr/bin/env python3
"""Trusted container registration primitives for DAIA pgvector."""

import re
import os
from pgvector_credentials import open_existing_state_directory
import stat


_CONTAINER_ID_PATTERN = re.compile(r"[0-9a-f]{64}", re.ASCII)


def validate_container_id(container_id: str) -> str:
    """Require a full lowercase hexadecimal Docker container ID."""
    if not isinstance(container_id, str):
        raise ValueError("Container ID must be text")

    if _CONTAINER_ID_PATTERN.fullmatch(container_id) is None:
        raise ValueError("Invalid Docker container ID")

    return container_id


def parse_registration(content: bytes) -> str:
    """Parse one container ID with an optional final newline."""
    if not isinstance(content, bytes):
        raise ValueError("Registration content must be bytes")

    if len(content) not in (64, 65):
        raise ValueError("Invalid registration length")

    if len(content) == 65:
        if not content.endswith(b"\n"):
            raise ValueError("Invalid registration terminator")
        content = content[:-1]

    try:
        container_id = content.decode("ascii")
    except UnicodeDecodeError as error:
        raise ValueError("Registration must contain ASCII") from error

    return validate_container_id(container_id)


def read_registration(directory_fd: int) -> str:
    """Read a trusted registration file relative to an open directory."""
    flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC

    fd = os.open(
        "container-id",
        flags,
        dir_fd=directory_fd,
    )

    try:
        metadata = os.fstat(fd)

        if not stat.S_ISREG(metadata.st_mode):
            raise ValueError("Registration must be a regular file")

        if metadata.st_uid != 0 or metadata.st_gid != 0:
            raise ValueError("Registration must be root-owned")

        if stat.S_IMODE(metadata.st_mode) != 0o600:
            raise ValueError("Registration must have mode 0600")

        if metadata.st_nlink != 1:
            raise ValueError("Registration must have exactly one hard link")

        if metadata.st_size not in (64, 65):
            raise ValueError("Invalid registration file size")

        content = os.read(fd, 66)

        if len(content) != metadata.st_size:
            raise ValueError("Registration size changed during reading")

        if os.fstat(fd).st_size != metadata.st_size:
            raise ValueError("Registration size changed during reading")

        return parse_registration(content)

    finally:
        os.close(fd)



def read_trusted_registration(state_directory: str) -> str:
    """Read a container ID from a verified root-owned state directory."""
    directory_fd = open_existing_state_directory(state_directory)

    try:
        return read_registration(directory_fd)
    finally:
        os.close(directory_fd)



def publish_registration(directory_fd: int, container_id: str) -> None:
    """Publish a verified container identity without replacing an existing one."""
    import fcntl
    import secrets

    container_id = validate_container_id(container_id)
    validate_registration_directory_fd(directory_fd)

    fcntl.flock(directory_fd, fcntl.LOCK_EX)

    try:
        try:
            existing = read_registration(directory_fd)
        except FileNotFoundError:
            existing = None

        if existing is not None:
            if existing != container_id:
                raise ValueError("Conflicting container registration")
            return

        temporary_name = ".container-id-" + secrets.token_hex(16)

        flags = (
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | os.O_NOFOLLOW
            | os.O_CLOEXEC
        )

        temporary_fd = os.open(
            temporary_name,
            flags,
            0o600,
            dir_fd=directory_fd,
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
                raise ValueError("Temporary registration metadata is unsafe")

            content = (container_id + chr(10)).encode("ascii")

            with os.fdopen(os.dup(temporary_fd), "wb") as stream:
                stream.write(content)
                stream.flush()
                os.fsync(stream.fileno())

            try:
                os.link(
                    temporary_name,
                    "container-id",
                    src_dir_fd=directory_fd,
                    dst_dir_fd=directory_fd,
                    follow_symlinks=False,
                )
            except FileExistsError:
                existing = read_registration(directory_fd)
                if existing != container_id:
                    raise ValueError("Conflicting container registration")
                return

            os.unlink(temporary_name, dir_fd=directory_fd)
            os.fsync(directory_fd)

            if read_registration(directory_fd) != container_id:
                raise ValueError("Published registration verification failed")

        finally:
            os.close(temporary_fd)
            try:
                os.unlink(temporary_name, dir_fd=directory_fd)
            except FileNotFoundError:
                pass

    finally:
        fcntl.flock(directory_fd, fcntl.LOCK_UN)



def validate_registration_directory_fd(directory_fd: int) -> None:
    """Require a root-owned private directory descriptor."""
    metadata = os.fstat(directory_fd)

    if not stat.S_ISDIR(metadata.st_mode):
        raise ValueError("Registration descriptor must reference a directory")

    if metadata.st_uid != 0 or metadata.st_gid != 0:
        raise ValueError("Registration directory must be root-owned")

    if stat.S_IMODE(metadata.st_mode) != 0o700:
        raise ValueError("Registration directory must have mode 0700")



def publish_trusted_registration(
    state_directory: str,
    container_id: str,
) -> None:
    """Publish a container ID through a verified state-directory path."""
    container_id = validate_container_id(container_id)

    directory_fd = open_existing_state_directory(state_directory)

    try:
        publish_registration(directory_fd, container_id)
    finally:
        os.close(directory_fd)


def verify_trusted_registration(
    state_directory: str,
    inspected_container_id: str,
) -> None:
    """Require Docker identity to match trusted persistent registration."""
    inspected_container_id = validate_container_id(
        inspected_container_id
    )

    registered_container_id = read_trusted_registration(
        state_directory
    )

    if registered_container_id != inspected_container_id:
        raise PermissionError(
            "Docker container identity does not match trusted registration"
        )
