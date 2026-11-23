"""Read-only SearXNG registration bootstrap decision.

A missing registration directory may be eligible for create-only
initialization, but absence does not authorize Docker provisioning.
"""

import os
import stat
from enum import Enum

DEFAULT_DIRECTORY = "/var/lib/daia/searxng"


class BootstrapDecision(str, Enum):
    INITIALIZE_ELIGIBLE = "INITIALIZE_ELIGIBLE"
    OBSERVE_EXISTING = "OBSERVE_EXISTING"
    UNTRUSTED = "UNTRUSTED"


def decide(directory=DEFAULT_DIRECTORY):
    """Inspect registration path without creating or adopting anything.

    The parent path must be root-owned, directory-only, symlink-free,
    and not writable by group or others.

    INITIALIZE_ELIGIBLE is a preliminary decision only. Callers must
    independently verify Docker and network absence before mutation.
    """
    if os.geteuid() != 0:
        return BootstrapDecision.UNTRUSTED

    if directory != DEFAULT_DIRECTORY:
        return BootstrapDecision.UNTRUSTED

    flags = (
        os.O_RDONLY
        | os.O_DIRECTORY
        | os.O_NOFOLLOW
        | getattr(os, "O_CLOEXEC", 0)
    )

    fd = None

    try:
        fd = os.open("/", flags)

        for component in ("var", "lib", "daia"):
            parent = os.fstat(fd)

            if (
                not stat.S_ISDIR(parent.st_mode)
                or parent.st_uid != 0
                or parent.st_mode & 0o022
            ):
                return BootstrapDecision.UNTRUSTED

            child = os.open(component, flags, dir_fd=fd)
            os.close(fd)
            fd = child

        parent = os.fstat(fd)

        if (
            not stat.S_ISDIR(parent.st_mode)
            or parent.st_uid != 0
            or parent.st_mode & 0o022
        ):
            return BootstrapDecision.UNTRUSTED

        try:
            entry = os.stat(
                "searxng",
                dir_fd=fd,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            return BootstrapDecision.INITIALIZE_ELIGIBLE

        if (
            not stat.S_ISDIR(entry.st_mode)
            or entry.st_uid != 0
            or stat.S_IMODE(entry.st_mode) != 0o700
        ):
            return BootstrapDecision.UNTRUSTED

        child = os.open("searxng", flags, dir_fd=fd)

        try:
            opened = os.fstat(child)
            if (
                opened.st_dev != entry.st_dev
                or opened.st_ino != entry.st_ino
                or opened.st_uid != 0
                or stat.S_IMODE(opened.st_mode) != 0o700
            ):
                return BootstrapDecision.UNTRUSTED
        finally:
            os.close(child)

        return BootstrapDecision.OBSERVE_EXISTING

    except (OSError, ValueError, TypeError):
        return BootstrapDecision.UNTRUSTED

    finally:
        if fd is not None:
            os.close(fd)
