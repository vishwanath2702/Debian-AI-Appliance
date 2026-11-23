"""Fail-closed exclusive lock for SearXNG firstboot."""

import fcntl
import os
import stat
from contextlib import contextmanager

LOCK_PATH = "/run/daia-searxng-firstboot.lock"


class LockRefused(RuntimeError):
    pass


def _trusted_parent():
    """Traverse lock ancestors without following symlinks."""

    path = LOCK_PATH

    if (
        type(path) is not str
        or not path.startswith("/")
        or "//" in path
    ):
        raise LockRefused("Invalid lock path")

    parts = path.split("/")[1:]

    if (
        len(parts) < 2
        or any(part in ("", ".", "..") for part in parts)
    ):
        raise LockRefused("Unsafe lock path")

    flags = (
        os.O_RDONLY
        | os.O_DIRECTORY
        | os.O_NOFOLLOW
        | getattr(os, "O_CLOEXEC", 0)
    )

    fd = os.open("/", flags)

    try:
        def verify_directory():
            info = os.fstat(fd)

            if (
                not stat.S_ISDIR(info.st_mode)
                or info.st_uid != 0
                or stat.S_IMODE(info.st_mode) & 0o022
            ):
                raise LockRefused("Untrusted lock ancestor")

        verify_directory()

        for part in parts[:-1]:
            child = os.open(part, flags, dir_fd=fd)
            os.close(fd)
            fd = child
            verify_directory()

        return fd, parts[-1]

    except BaseException:
        os.close(fd)
        raise


@contextmanager
def exclusive_firstboot_lock():
    """Hold an exclusive nonblocking lock until context exit."""

    if os.geteuid() != 0:
        raise LockRefused("Root required")

    parent_fd = None
    fd = None

    try:
        parent_fd, name = _trusted_parent()

        flags = (
            os.O_RDWR
            | os.O_CREAT
            | os.O_NOFOLLOW
            | getattr(os, "O_CLOEXEC", 0)
        )

        fd = os.open(name, flags, 0o600, dir_fd=parent_fd)

        def verify_identity():
            opened = os.fstat(fd)
            entry = os.stat(
                name,
                dir_fd=parent_fd,
                follow_symlinks=False,
            )

            if (
                not stat.S_ISREG(opened.st_mode)
                or opened.st_uid != 0
                or stat.S_IMODE(opened.st_mode) != 0o600
                or opened.st_nlink != 1
                or not stat.S_ISREG(entry.st_mode)
                or (opened.st_dev, opened.st_ino)
                != (entry.st_dev, entry.st_ino)
            ):
                raise LockRefused("Untrusted lock identity")

        verify_identity()

        try:
            fcntl.flock(
                fd,
                fcntl.LOCK_EX | fcntl.LOCK_NB,
            )
        except BlockingIOError as error:
            raise LockRefused(
                "Concurrent firstboot operation"
            ) from error

        verify_identity()

        try:
            yield
        finally:
            verify_identity()

    except OSError as error:
        raise LockRefused(
            "Lock filesystem operation failed"
        ) from error

    finally:
        if fd is not None:
            os.close(fd)

        if parent_fd is not None:
            os.close(parent_fd)
