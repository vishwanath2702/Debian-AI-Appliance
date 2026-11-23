#!/usr/bin/env python3
"""Fail-closed SearXNG registration reader and verifier."""

import os
import re
import stat
import subprocess

DEFAULT_DIRECTORY = "/var/lib/daia/searxng"
REGISTRATION_NAME = "container-id"


def valid_id(value):
    return (
        isinstance(value, str)
        and re.fullmatch("[0-9a-f]{64}", value) is not None
    )


def require(condition, message):
    if not condition:
        raise PermissionError(message)



def _open_trusted_directory(directory):
    flags = (os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW |
             getattr(os, "O_CLOEXEC", 0))
    fd = os.open(os.fspath(directory), flags)
    try:
        info = os.fstat(fd)
        require(
            stat.S_ISDIR(info.st_mode)
            and info.st_uid == 0
            and stat.S_IMODE(info.st_mode) == 0o700,
            "Untrusted registration directory",
        )
        return fd
    except BaseException:
        os.close(fd)
        raise


def _read_registration_fd(directory_fd):
    flags = (os.O_RDONLY | os.O_NOFOLLOW |
             getattr(os, "O_CLOEXEC", 0))
    fd = os.open(REGISTRATION_NAME, flags, dir_fd=directory_fd)
    try:
        info = os.fstat(fd)
        require(
            stat.S_ISREG(info.st_mode)
            and info.st_uid == 0
            and stat.S_IMODE(info.st_mode) == 0o600
            and info.st_nlink == 1
            and info.st_size == 65,
            "Untrusted registration file",
        )
        content = os.read(fd, 66)
        require(
            len(content) == 65 and content.endswith(b"\n"),
            "Invalid registration encoding",
        )
        value = content[:-1].decode("ascii")
        require(valid_id(value), "Invalid registered identity")
        return value
    finally:
        os.close(fd)


def _verify_container(expected_id):
    result = subprocess.run(
        ["/usr/libexec/daia-searxng-verify-container", expected_id],
        capture_output=True, text=True, timeout=30, check=False,
    )
    require(
        result.returncode == 0
        and result.stdout == expected_id + "\n",
        "Trusted SearXNG container verification failed",
    )
    return expected_id

def read_registration(directory=DEFAULT_DIRECTORY):
    directory_fd = _open_trusted_directory(directory)
    try:
        return _read_registration_fd(directory_fd)
    finally:
        os.close(directory_fd)



def verify_registration(expected_id, directory=DEFAULT_DIRECTORY):
    require(os.geteuid() == 0, "Root required")
    require(valid_id(expected_id), "Invalid expected identity")
    registered = read_registration(directory)
    require(registered == expected_id,
            "Trusted registration identity mismatch")
    return _verify_container(expected_id)



def publish_registration(expected_id, directory=DEFAULT_DIRECTORY):
    require(os.geteuid() == 0, "Root required")
    require(valid_id(expected_id), "Invalid expected identity")

    directory_fd = _open_trusted_directory(directory)
    try:
        _verify_container(expected_id)

        flags = (os.O_WRONLY | os.O_CREAT | os.O_EXCL |
                 os.O_NOFOLLOW | getattr(os, "O_CLOEXEC", 0))

        fd = os.open(
            REGISTRATION_NAME, flags, 0o600,
            dir_fd=directory_fd,
        )

        complete = False
        try:
            os.fchmod(fd, 0o600)
            payload = expected_id.encode("ascii") + b"\n"
            offset = 0
            while offset < len(payload):
                count = os.write(fd, payload[offset:])
                require(count > 0, "Registration write failed")
                offset += count
            os.fsync(fd)
            complete = True
        finally:
            os.close(fd)
            if not complete:
                os.unlink(REGISTRATION_NAME, dir_fd=directory_fd)

        os.fsync(directory_fd)
        return expected_id
    finally:
        os.close(directory_fd)
