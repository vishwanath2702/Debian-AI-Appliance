#!/usr/bin/env python3
"""Root-only, create-only SearXNG registration initialization."""

import os
import stat

DEFAULT_DIRECTORY = "/var/lib/daia/searxng"


class InitializationError(RuntimeError):
    pass


def require(condition, message):
    if not condition:
        raise InitializationError(message)


def _components(directory):
    require(
        isinstance(directory, str)
        and directory.startswith("/")
        and directory != "/"
        and "//" not in directory,
        "Invalid absolute path",
    )
    parts = directory.split("/")[1:]
    require(
        len(parts) >= 2
        and all(part not in ("", ".", "..") for part in parts),
        "Unsafe path component",
    )
    return parts


def _open_parent(directory):
    parts = _components(directory)
    flags = (
        os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
        | getattr(os, "O_CLOEXEC", 0)
    )
    fd = os.open("/", flags)

    try:
        root_info = os.fstat(fd)
        require(
            stat.S_ISDIR(root_info.st_mode)
            and root_info.st_uid == 0
            and not (stat.S_IMODE(root_info.st_mode) & 0o022),
            "Untrusted filesystem root",
        )

        for part in parts[:-1]:
            next_fd = os.open(part, flags, dir_fd=fd)
            os.close(fd)
            fd = next_fd

            info = os.fstat(fd)
            require(
                stat.S_ISDIR(info.st_mode)
                and info.st_uid == 0
                and not (stat.S_IMODE(info.st_mode) & 0o022),
                "Untrusted registration ancestor",
            )

        return fd, parts[-1]

    except BaseException:
        os.close(fd)
        raise


def _initialize(directory):
    parent_fd, name = _open_parent(directory)

    try:
        os.mkdir(name, mode=0o700, dir_fd=parent_fd)

        flags = (
            os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
            | getattr(os, "O_CLOEXEC", 0)
        )
        child_fd = os.open(name, flags, dir_fd=parent_fd)

        try:
            info = os.fstat(child_fd)
            entry = os.stat(
                name,
                dir_fd=parent_fd,
                follow_symlinks=False,
            )

            require(
                stat.S_ISDIR(info.st_mode)
                and info.st_uid == 0
                and stat.S_IMODE(info.st_mode) == 0o700
                and (info.st_dev, info.st_ino)
                == (entry.st_dev, entry.st_ino),
                "Created registration directory failed verification",
            )

            require(
                not os.listdir(child_fd),
                "New registration directory unexpectedly populated",
            )

            os.fsync(child_fd)
            os.fsync(parent_fd)

        finally:
            os.close(child_fd)

    finally:
        os.close(parent_fd)


def initialize(directory=DEFAULT_DIRECTORY):
    require(os.geteuid() == 0, "Root required")
    require(
        directory == DEFAULT_DIRECTORY,
        "Production registration path is fixed",
    )
    _initialize(directory)
