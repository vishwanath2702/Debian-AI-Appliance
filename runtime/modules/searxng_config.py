"""Fail-closed, read-only SearXNG feature configuration.

This module never executes the configuration as shell code.
The production entry point uses a fixed absolute path.
"""

import os
import re
import stat

CONFIG_PATH = "/opt/daia/config/pragna.conf"
MAX_BYTES = 1024 * 1024

_ASSIGNMENT = re.compile(
    r"""^[ \t]*DAIA_ENABLE_SEARXNG[ \t]*=[ \t]*
    (?:"(true|false)"|'(true|false)'|(true|false))[ \t]*$""",
    re.VERBOSE,
)


class ConfigError(RuntimeError):
    pass


def _check_directory(fd):
    info = os.fstat(fd)
    if (
        not stat.S_ISDIR(info.st_mode)
        or info.st_uid != 0
        or info.st_mode & 0o022
    ):
        raise ConfigError("Untrusted configuration directory")


def _read_config(path):
    """Internal descriptor-based reader; not a production path override."""
    if not os.path.isabs(path):
        raise ConfigError("Configuration path must be absolute")

    components = path.split("/")[1:]
    if not components or any(
        not part or part in (".", "..") for part in components
    ):
        raise ConfigError("Invalid configuration path")

    directory_flags = (
        os.O_RDONLY
        | os.O_DIRECTORY
        | os.O_NOFOLLOW
        | getattr(os, "O_CLOEXEC", 0)
    )

    fd = None

    try:
        fd = os.open("/", directory_flags)
        _check_directory(fd)

        for component in components[:-1]:
            child = os.open(component, directory_flags, dir_fd=fd)
            os.close(fd)
            fd = child
            _check_directory(fd)

        file_flags = (
            os.O_RDONLY
            | os.O_NOFOLLOW
            | os.O_NONBLOCK
            | getattr(os, "O_CLOEXEC", 0)
        )

        config_fd = os.open(
            components[-1],
            file_flags,
            dir_fd=fd,
        )

        try:
            info = os.fstat(config_fd)

            if (
                not stat.S_ISREG(info.st_mode)
                or info.st_uid != 0
                or info.st_gid != 0
                or stat.S_IMODE(info.st_mode) != 0o644
                or info.st_size > MAX_BYTES
            ):
                raise ConfigError("Untrusted configuration file")

            with os.fdopen(
                os.dup(config_fd),
                "rb",
            ) as stream:
                data = stream.read(MAX_BYTES + 1)

            if len(data) > MAX_BYTES:
                raise ConfigError("Configuration too large")

        finally:
            os.close(config_fd)

    except OSError as exc:
        raise ConfigError("Configuration access failed") from exc

    finally:
        if fd is not None:
            os.close(fd)

    try:
        return data.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise ConfigError("Configuration is not UTF-8") from exc


def _parse(contents):
    values = []

    for line in contents.splitlines():
        stripped = line.strip()

        if not stripped or stripped.startswith("#"):
            continue

        if not stripped.startswith("DAIA_ENABLE_SEARXNG"):
            continue

        match = _ASSIGNMENT.fullmatch(line)

        if match is None:
            raise ConfigError("Malformed SearXNG flag")

        values.append(
            next(value for value in match.groups() if value is not None)
        )

    if len(values) != 1:
        raise ConfigError("SearXNG flag must appear exactly once")

    return values[0] == "true"


def enabled():
    """Production entry point: root-only and fixed configuration path."""
    if os.geteuid() != 0:
        raise ConfigError("Root required")

    return _parse(_read_config(CONFIG_PATH))
