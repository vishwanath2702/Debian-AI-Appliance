"""Filesystem trust checks for privileged DAIA Python imports.

Validation alone does not authorize importing by pathname.
A race-resistant import mechanism is required separately.
"""

import os
import stat
import sys
import types
from pathlib import Path


MODULES = (
    "openwebui_registration.py",
    "pgvector_registration.py",
    "pgvector_credentials.py",
)


def _check_metadata(metadata, *, directory, expected_uid):
    if directory:
        if not stat.S_ISDIR(metadata.st_mode):
            raise PermissionError("Trusted import path is not a directory")
    elif not stat.S_ISREG(metadata.st_mode):
        raise PermissionError("Trusted import module is not a regular file")

    if metadata.st_uid != expected_uid:
        raise PermissionError("Trusted import ownership mismatch")

    if metadata.st_mode & 0o022:
        raise PermissionError("Trusted import path is group/world writable")


def validate_trusted_runtime_imports(
    directory="/opt/daia", *, expected_uid=0, extra_modules=()
):
    """Validate trusted directory and module metadata without following symlinks.

    This is a preliminary validator, not a race-resistant import operation.
    """
    if extra_modules not in ((), ("tika_registration",)):
        raise ValueError("Unsupported trusted module selection")
    module_files = MODULES + tuple(
        name + ".py" for name in extra_modules
    )
    directory = Path(directory)

    if not directory.is_absolute():
        raise ValueError("Trusted import directory must be absolute")

    if any(part in (".", "..") for part in str(directory).split("/")):
        raise ValueError("Trusted import directory is not normalized")

    flags = (
        os.O_RDONLY
        | os.O_DIRECTORY
        | os.O_NOFOLLOW
        | os.O_CLOEXEC
    )

    directory_fd = os.open("/", flags)

    try:
        root_metadata = os.fstat(directory_fd)
        if (
            not stat.S_ISDIR(root_metadata.st_mode)
            or root_metadata.st_uid != 0
            or root_metadata.st_mode & 0o022
        ):
            raise PermissionError("Filesystem root is not trusted")

        components = directory.parts[1:]

        for index, component in enumerate(components):
            next_fd = os.open(
                component,
                flags,
                dir_fd=directory_fd,
            )
            os.close(directory_fd)
            directory_fd = next_fd

            metadata = os.fstat(directory_fd)
            is_final = index == len(components) - 1

            if not stat.S_ISDIR(metadata.st_mode):
                raise PermissionError(
                    "Trusted import ancestor is not a directory"
                )

            if expected_uid == 0:
                _check_metadata(
                    metadata,
                    directory=True,
                    expected_uid=0,
                )
            elif is_final:
                _check_metadata(
                    metadata,
                    directory=True,
                    expected_uid=expected_uid,
                )
            elif (
                metadata.st_uid == 0
                and stat.S_IMODE(metadata.st_mode) == 0o1777
                and directory.parts[:index + 2] == ("/", "tmp")
            ):
                # Test-only allowance for the system temporary directory.
                continue
            elif (
                metadata.st_uid in (0, expected_uid)
                and not metadata.st_mode & 0o022
            ):
                continue
            else:
                raise PermissionError(
                    "Trusted import ancestor ownership or mode mismatch"
                )

        _check_metadata(
            os.fstat(directory_fd),
            directory=True,
            expected_uid=expected_uid,
        )

        for name in module_files:
            file_fd = os.open(
                name,
                os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC,
                dir_fd=directory_fd,
            )
            try:
                metadata = os.fstat(file_fd)
                _check_metadata(
                    metadata,
                    directory=False,
                    expected_uid=expected_uid,
                )
                if metadata.st_nlink != 1:
                    raise PermissionError(
                        "Trusted import module has multiple hard links"
                    )
            finally:
                os.close(file_fd)
    finally:
        os.close(directory_fd)


def read_trusted_module_source(file_fd, *, expected_uid=0):
    """Read source from an already-open, validated regular file."""
    metadata = os.fstat(file_fd)

    _check_metadata(
        metadata,
        directory=False,
        expected_uid=expected_uid,
    )

    if metadata.st_nlink != 1:
        raise PermissionError("Trusted module has multiple hard links")

    if metadata.st_size > 1024 * 1024:
        raise ValueError("Trusted module exceeds source size limit")

    source = bytearray()
    offset = 0

    while offset < metadata.st_size:
        chunk = os.pread(
            file_fd,
            min(65536, metadata.st_size - offset),
            offset,
        )

        if not chunk:
            raise OSError("Trusted module source truncated")

        source.extend(chunk)
        offset += len(chunk)

    after = os.fstat(file_fd)

    if (
        after.st_dev != metadata.st_dev
        or after.st_ino != metadata.st_ino
        or after.st_size != metadata.st_size
        or after.st_mtime_ns != metadata.st_mtime_ns
        or after.st_ctime_ns != metadata.st_ctime_ns
    ):
        raise PermissionError("Trusted module changed during reading")

    return bytes(source)


def load_trusted_runtime_modules(directory="/opt/daia", *, expected_uid=0, extra_modules=()):
    """Load the fixed DAIA module set from verified descriptors.

    This is not yet approved for privileged service activation.
    """
    if extra_modules:
        validate_trusted_runtime_imports(
            directory,
            expected_uid=expected_uid,
            extra_modules=extra_modules,
        )
    else:
        validate_trusted_runtime_imports(
            directory,
            expected_uid=expected_uid,
        )

    module_files = MODULES + tuple(
        name + ".py" for name in extra_modules
    )
    directory = Path(directory)
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC

    directory_fd = os.open(directory, flags)

    sources = {}

    try:
        for name in module_files:
            file_fd = os.open(
                name,
                os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC,
                dir_fd=directory_fd,
            )

            try:
                sources[name[:-3]] = read_trusted_module_source(
                    file_fd,
                    expected_uid=expected_uid,
                )
            finally:
                os.close(file_fd)
    finally:
        os.close(directory_fd)

    order = (
        "pgvector_credentials",
        "pgvector_registration",
        "openwebui_registration",
    ) + extra_modules

    previous = {
        name: sys.modules.get(name)
        for name in order
    }

    loaded = {}

    try:
        for name in order:
            module = types.ModuleType(name)
            module.__file__ = f"<trusted-daia:{name}>"
            module.__package__ = ""
            sys.modules[name] = module

            code = compile(
                sources[name],
                module.__file__,
                "exec",
            )

            exec(code, module.__dict__)
            loaded[name] = module

    except BaseException:
        for name, original in previous.items():
            if original is None:
                sys.modules.pop(name, None)
            else:
                sys.modules[name] = original
        raise

    return loaded
