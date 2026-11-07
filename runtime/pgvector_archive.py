#!/usr/bin/env python3
"""Open and verify an installed DAIA pgvector archive without pathname races.

Usage: python3 pgvector_archive.py HASH PATH; streams validated bytes to stdout.
Only for root-owned installed payloads. Does not load Docker images.
"""
import hashlib
import os
import stat
import sys

EXPECTED_SHA = "5d903322c843abad607f7fa9349b0b7a1c1a53c02b1306eada13451d2a367228"


def verified_archive(path, expected_sha=EXPECTED_SHA):
    if os.geteuid() != 0:
        raise PermissionError("root required")
    if not isinstance(path, str) or not path.startswith("/") or path == "/" or "//" in path:
        raise ValueError("unsafe archive path")
    parts = path[1:].split("/")
    if any(p in ("", ".", "..") for p in parts):
        raise ValueError("unsafe archive path component")
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    parent = os.open("/", flags)
    fd = None
    try:
        for part in parts[:-1]:
            metadata = os.fstat(parent)
            if not stat.S_ISDIR(metadata.st_mode) or metadata.st_uid != 0 or metadata.st_mode & 0o022:
                raise PermissionError("unsafe archive ancestor")
            child = os.open(part, flags, dir_fd=parent)
            os.close(parent)
            parent = child
        metadata = os.fstat(parent)
        if not stat.S_ISDIR(metadata.st_mode) or metadata.st_uid != 0 or metadata.st_mode & 0o022:
            raise PermissionError("unsafe archive parent")
        fd = os.open(parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK, dir_fd=parent)
        metadata = os.fstat(fd)
        if (not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != 0
                or metadata.st_mode & 0o022 or metadata.st_nlink != 1):
            raise PermissionError("unsafe archive metadata")
        # Snapshot into a private anonymous temporary file so hashing and
        # downstream consumption use exactly the same bytes.
        import tempfile
        snapshot = tempfile.TemporaryFile(mode="w+b")
        digest = hashlib.sha256()
        size = 0
        try:
            while True:
                block = os.read(fd, 1024 * 1024)
                if not block:
                    break
                digest.update(block)
                snapshot.write(block)
                size += len(block)
            after = os.fstat(fd)
            if (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns) != (metadata.st_dev, metadata.st_ino, metadata.st_size, metadata.st_mtime_ns, metadata.st_ctime_ns):
                raise PermissionError("archive changed during verification")
            if size != metadata.st_size or digest.hexdigest() != expected_sha:
                raise ValueError("archive checksum mismatch")
            snapshot.seek(0)
            return snapshot
        except BaseException:
            snapshot.close()
            raise
    finally:
        if fd is not None:
            os.close(fd)
        os.close(parent)


def main():
    if len(sys.argv) != 3 or sys.argv[1] not in ('stream', 'load'):
        return 2
    try:
        with verified_archive(sys.argv[2]) as stream:
            if sys.argv[1] == 'load':
                import subprocess
                # Docker reads the already verified private snapshot, never
                # the original mutable pathname. No shell pipeline involved.
                result = subprocess.run(
                    ['docker', 'load'], stdin=stream,
                    stdout=subprocess.DEVNULL, check=False,
                )
                return 0 if result.returncode == 0 else 1
            while True:
                block = stream.read(1024 * 1024)
                if not block:
                    break
                sys.stdout.buffer.write(block)
        return 0
    except (OSError, ValueError) as exc:
        print('pgvector archive rejected: ' + str(exc), file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
