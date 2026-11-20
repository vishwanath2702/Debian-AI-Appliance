#!/usr/bin/env python3
"""Safely register a verified offline Ollama embedding model."""

import hashlib
import secrets
import time
import json
import os
from pathlib import Path
import pwd
import grp
import re
import stat
import subprocess
import sys
import tarfile
import tempfile

ARCHIVE = Path(
    "/opt/daia/payload/images/nomic-embed-text-ollama.tar"
)
STORE = Path("/var/lib/ollama/models")
MODEL = "nomic-embed-text:latest"

MANIFEST = (
    "manifests/registry.ollama.ai/library/"
    "nomic-embed-text/latest"
)

EXPECTED_HASH = (
    "c7c5ab21092400fe75137ed90efa931"
    "aaf804000c9fca6e04ac9d809575a3ebd"
)

DIGEST_RE = re.compile(r"sha256:([0-9a-f]{64})\Z")
BLOB_RE = re.compile(r"blobs/sha256-[0-9a-f]{64}\Z")

ALLOWED_DIRECTORIES = {
    "blobs",
    "manifests",
    "manifests/registry.ollama.ai",
    "manifests/registry.ollama.ai/library",
    "manifests/registry.ollama.ai/library/nomic-embed-text",
}


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as source:
        for chunk in iter(
            lambda: source.read(4 * 1024 * 1024),
            b"",
        ):
            digest.update(chunk)
    return digest.hexdigest()


def safe_directory(path, uid, gid, *, allow_test_sticky=False):
    """Return verified directory fd; caller must close it.

    Production rejects writable ancestors, including sticky ones.
    The test exception is never used by production call sites.
    """
    path = Path(path)

    if not path.is_absolute():
        raise RuntimeError("Model store path must be absolute")

    components = path.parts[1:]
    if any(part in ("", ".", "..") for part in components):
        raise RuntimeError("Unsafe model store path")

    flags = (
        os.O_RDONLY | os.O_DIRECTORY
        | os.O_NOFOLLOW | os.O_CLOEXEC
    )
    current_fd = os.open("/", flags)

    try:
        for component in components:
            parent_info = os.fstat(current_fd)
            parent_mode = stat.S_IMODE(parent_info.st_mode)

            sticky_exception = (
                allow_test_sticky
                and parent_info.st_uid == 0
                and bool(parent_info.st_mode & stat.S_ISVTX)
            )

            if parent_mode & 0o022 and not sticky_exception:
                raise RuntimeError(
                    f"Unsafe writable ancestor: {component}"
                )

            created = False

            try:
                next_fd = os.open(
                    component, flags, dir_fd=current_fd
                )
            except FileNotFoundError:
                # Do not create directories inside a parent that
                # the Ollama identity can rename or replace.
                if parent_info.st_uid != 0 and not (
                    os.geteuid() == uid
                    and parent_info.st_uid == uid
                    and parent_info.st_gid == gid
                ):
                    raise RuntimeError(
                        f"Untrusted parent for directory creation: {component}"
                    )

                try:
                    os.mkdir(
                        component, 0o755, dir_fd=current_fd
                    )
                    created = True
                except FileExistsError:
                    pass

                next_fd = os.open(
                    component, flags, dir_fd=current_fd
                )

            try:
                info = os.fstat(next_fd)

                if not stat.S_ISDIR(info.st_mode):
                    raise RuntimeError(
                        f"Unsafe directory component: {component}"
                    )

                if created:
                    # The parent is root-owned and not writable by
                    # untrusted identities. Confirm the directory
                    # entry still names the opened inode.
                    entry = os.stat(
                        component,
                        dir_fd=current_fd,
                        follow_symlinks=False,
                    )

                    if (
                        not stat.S_ISDIR(entry.st_mode)
                        or entry.st_dev != info.st_dev
                        or entry.st_ino != info.st_ino
                        or info.st_uid not in (0, uid)
                        or (
                            info.st_uid == uid
                            and info.st_gid != gid
                        )
                    ):
                        raise RuntimeError(
                            "Created directory identity conflict"
                        )

                    os.fchown(next_fd, uid, gid)
                    os.fchmod(next_fd, 0o755)
                    info = os.fstat(next_fd)

                if info.st_uid not in (0, uid):
                    raise RuntimeError(
                        f"Directory ownership conflict: {component}"
                    )

                if info.st_uid == uid and info.st_gid != gid:
                    raise RuntimeError(
                        f"Directory group conflict: {component}"
                    )

                mode = stat.S_IMODE(info.st_mode)

                current_sticky_exception = (
                    allow_test_sticky
                    and info.st_uid == 0
                    and bool(info.st_mode & stat.S_ISVTX)
                )

                if mode & 0o022 and not current_sticky_exception:
                    raise RuntimeError(
                        f"Unsafe writable directory: {component}"
                    )

                os.close(current_fd)
                current_fd = next_fd
                next_fd = None

            finally:
                if next_fd is not None:
                    os.close(next_fd)

        return current_fd

    except Exception:
        os.close(current_fd)
        raise




def publish(source, destination, expected_hash,
            expected_size, uid, gid):
    destination = Path(destination)
    parent_fd = safe_directory(destination.parent, uid, gid)
    temporary = None

    try:
        name = destination.name
        if name in ("", ".", "..") or "/" in name:
            raise RuntimeError("Unsafe destination filename")

        flags = (
            os.O_RDONLY | os.O_NOFOLLOW
            | os.O_CLOEXEC | os.O_NONBLOCK
        )

        def verify_existing():
            # The final name can briefly have two hard links
            # while a concurrent publisher removes its temporary
            # name. Retry only that exact verified condition.
            for attempt in range(25):
                fd = os.open(name, flags, dir_fd=parent_fd)

                try:
                    before = os.fstat(fd)

                    if not stat.S_ISREG(before.st_mode):
                        raise RuntimeError(
                            "Unsafe existing destination"
                        )

                    if before.st_uid != uid or before.st_gid != gid:
                        raise RuntimeError(
                            "Existing file ownership conflict"
                        )

                    if stat.S_IMODE(before.st_mode) != 0o644:
                        raise RuntimeError(
                            "Existing file permissions conflict"
                        )

                    if before.st_size != expected_size:
                        raise RuntimeError(
                            "Existing file size conflict"
                        )

                    digest = hashlib.sha256()

                    with os.fdopen(os.dup(fd), "rb") as stream:
                        for chunk in iter(
                            lambda: stream.read(4 * 1024 * 1024),
                            b"",
                        ):
                            digest.update(chunk)

                    if digest.hexdigest() != expected_hash:
                        raise RuntimeError(
                            "Existing file hash conflict"
                        )

                    after = os.fstat(fd)
                    current = os.stat(
                        name,
                        dir_fd=parent_fd,
                        follow_symlinks=False,
                    )

                    if (
                        after.st_dev != before.st_dev
                        or after.st_ino != before.st_ino
                        or after.st_size != before.st_size
                        or after.st_mtime_ns != before.st_mtime_ns
                        or current.st_dev != before.st_dev
                        or current.st_ino != before.st_ino
                    ):
                        raise RuntimeError(
                            "Existing file changed during verification"
                        )

                    links = after.st_nlink

                    if links == 1:
                        return "EXISTS_VERIFIED"

                    if links != 2:
                        raise RuntimeError(
                            "Existing file hardlink conflict"
                        )

                finally:
                    os.close(fd)

                if attempt == 24:
                    raise RuntimeError(
                        "Persistent existing file hardlink conflict"
                    )

                time.sleep(0.02)

            raise RuntimeError("Existing file verification exhausted")

        try:
            return verify_existing()
        except FileNotFoundError:
            pass

        for _ in range(10):
            candidate = ".daia-embedding-" + secrets.token_hex(16)

            try:
                fd = os.open(
                    candidate,
                    os.O_WRONLY | os.O_CREAT | os.O_EXCL
                    | os.O_NOFOLLOW | os.O_CLOEXEC,
                    0o600,
                    dir_fd=parent_fd,
                )
                temporary = candidate
                break
            except FileExistsError:
                continue
        else:
            raise RuntimeError("Temporary filename collision")

        digest = hashlib.sha256()
        total = 0

        with os.fdopen(fd, "wb") as output:
            while True:
                chunk = source.read(4 * 1024 * 1024)
                if not chunk:
                    break

                output.write(chunk)
                digest.update(chunk)
                total += len(chunk)

            output.flush()
            os.fsync(output.fileno())

        if total != expected_size:
            raise RuntimeError("Published size mismatch")

        if digest.hexdigest() != expected_hash:
            raise RuntimeError("Published checksum mismatch")

        metadata_fd = os.open(
            temporary,
            os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC,
            dir_fd=parent_fd,
        )

        try:
            os.fchown(metadata_fd, uid, gid)
            os.fchmod(metadata_fd, 0o644)
            os.fsync(metadata_fd)
        finally:
            os.close(metadata_fd)

        try:
            os.link(
                temporary, name,
                src_dir_fd=parent_fd,
                dst_dir_fd=parent_fd,
                follow_symlinks=False,
            )
        except FileExistsError:
            return verify_existing()

        return "INSTALLED"

    finally:
        if temporary is not None:
            os.unlink(temporary, dir_fd=parent_fd)
        os.close(parent_fd)



def validate_archive(archive, expected_hash):
    info = os.fstat(archive)
    if not stat.S_ISREG(info.st_mode):
        raise RuntimeError("Offline archive descriptor is unsafe")

    digest = hashlib.sha256()
    offset = 0
    while offset < info.st_size:
        chunk = os.pread(
            archive, min(4 * 1024 * 1024, info.st_size - offset), offset
        )
        if not chunk:
            raise RuntimeError("Offline archive truncated")
        digest.update(chunk)
        offset += len(chunk)

    after = os.fstat(archive)
    if (
        after.st_dev != info.st_dev
        or after.st_ino != info.st_ino
        or after.st_size != info.st_size
        or after.st_mtime_ns != info.st_mtime_ns
        or after.st_ctime_ns != info.st_ctime_ns
        or digest.hexdigest() != expected_hash
    ):
        raise RuntimeError("Offline archive checksum or identity mismatch")

    os.lseek(archive, 0, os.SEEK_SET)
    with tarfile.open(fileobj=os.fdopen(os.dup(archive), "rb"), mode="r:") as tar:
        files = {}

        for member in tar.getmembers():
            name = member.name

            if name.startswith("./"):
                name = name[2:]

            if member.isdir():
                if name not in ALLOWED_DIRECTORIES:
                    raise RuntimeError(
                        f"Unexpected archive directory: {name}"
                    )
                continue

            if not member.isfile():
                raise RuntimeError(
                    f"Unsafe archive member: {name}"
                )

            if name in files:
                raise RuntimeError(
                    f"Duplicate archive member: {name}"
                )

            if name != MANIFEST and not BLOB_RE.fullmatch(name):
                raise RuntimeError(
                    f"Unexpected archive member: {name}"
                )

            files[name] = member

        if MANIFEST not in files:
            raise RuntimeError("Official model manifest missing")

        manifest_member = files[MANIFEST]

        if manifest_member.size > 1024 * 1024:
            raise RuntimeError("Manifest exceeds size limit")

        manifest_bytes = tar.extractfile(
            manifest_member
        ).read()

        manifest = json.loads(manifest_bytes)

        descriptors = (
            [manifest["config"]] + manifest["layers"]
        )

        expected_blobs = {}

        for descriptor in descriptors:
            match = DIGEST_RE.fullmatch(
                descriptor["digest"]
            )

            if match is None:
                raise RuntimeError("Invalid blob digest")

            name = "blobs/sha256-" + match.group(1)
            size = descriptor["size"]

            if (
                not isinstance(size, int)
                or isinstance(size, bool)
                or size < 0
            ):
                raise RuntimeError("Invalid blob size")

            if (
                name in expected_blobs
                and expected_blobs[name] != size
            ):
                raise RuntimeError(
                    "Conflicting blob descriptors"
                )

            expected_blobs[name] = size

        if set(files) != set(expected_blobs) | {MANIFEST}:
            raise RuntimeError(
                "Archive blob inventory mismatch"
            )

        for name, size in expected_blobs.items():
            member = files[name]

            if member.size != size:
                raise RuntimeError(
                    f"Blob size mismatch: {name}"
                )

            digest = hashlib.sha256()
            stream = tar.extractfile(member)

            for chunk in iter(
                lambda: stream.read(4 * 1024 * 1024),
                b"",
            ):
                digest.update(chunk)

            expected = name.split("sha256-", 1)[1]

            if digest.hexdigest() != expected:
                raise RuntimeError(
                    f"Blob checksum mismatch: {name}"
                )

        return expected_blobs


def install(archive, store, expected_hash, uid, gid):
    expected_blobs = validate_archive(
        archive,
        expected_hash,
    )

    os.lseek(archive, 0, os.SEEK_SET)
    with tarfile.open(fileobj=os.fdopen(os.dup(archive), "rb"), mode="r:") as tar:
        members = {
            m.name.removeprefix("./"): m
            for m in tar.getmembers()
            if m.isfile()
        }

        os.close(safe_directory(store, uid, gid))

        for name, size in expected_blobs.items():
            result = publish(
                tar.extractfile(members[name]),
                Path(store) / name,
                name.split("sha256-", 1)[1],
                size,
                uid,
                gid,
            )

            print(
                f"BLOB={name} STATUS={result}",
                flush=True,
            )

        manifest_member = members[MANIFEST]
        manifest_bytes = tar.extractfile(
            manifest_member
        ).read()

        result = publish(
            tar.extractfile(manifest_member),
            Path(store) / MANIFEST,
            hashlib.sha256(manifest_bytes).hexdigest(),
            len(manifest_bytes),
            uid,
            gid,
        )

        print(
            f"MANIFEST={result}",
            flush=True,
        )


def verify_ollama():
    if os.geteuid() == 0:
        raise RuntimeError(
            "Ollama verification must run without root privileges"
        )

    command = [
        "/usr/local/bin/ollama",
        "show",
        MODEL,
    ]

    environment = os.environ.copy()
    environment.update({
        "HOME": "/var/lib/ollama",
        "OLLAMA_MODELS": "/var/lib/ollama/models",
        "OLLAMA_HOST": "127.0.0.1:11434",
    })

    result = subprocess.run(
        command,
        env=environment,
        capture_output=True,
        text=True,
        timeout=90,
        check=False,
    )

    if result.returncode:
        raise RuntimeError(
            "Ollama model recognition failed: "
            + result.stderr[:500]
        )

    print(
        "OLLAMA_MODEL_RECOGNITION=PASS",
        flush=True,
    )


def main():
    if os.geteuid() != 0:
        raise RuntimeError("Root privileges required")

    user = pwd.getpwnam("ollama")
    group = grp.getgrnam("ollama")
    uid, gid = user.pw_uid, group.gr_gid

    if uid == 0 or gid == 0:
        raise RuntimeError("Unsafe Ollama service identity")

    flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK
    archive_fd = os.open(ARCHIVE, flags)

    try:
        if not stat.S_ISREG(os.fstat(archive_fd).st_mode):
            raise RuntimeError("Offline archive must be a regular file")

        command = [
            "/usr/bin/setpriv",
            f"--reuid={uid}",
            f"--regid={gid}",
            "--clear-groups",
            "--no-new-privs",
            sys.executable,
            os.path.abspath(__file__),
            "--embedding-worker",
            str(archive_fd),
            str(uid),
            str(gid),
        ]

        result = subprocess.run(
            command,
            pass_fds=(archive_fd,),
            check=False,
        )

        if result.returncode:
            raise RuntimeError(
                f"Offline embedding worker failed: {result.returncode}"
            )
    finally:
        os.close(archive_fd)


def embedding_worker():
    if len(sys.argv) != 5:
        raise RuntimeError("Invalid embedding worker invocation")

    archive_fd = int(sys.argv[2])
    uid = int(sys.argv[3])
    gid = int(sys.argv[4])

    if (
        uid == 0 or gid == 0
        or os.geteuid() != uid
        or os.getegid() != gid
        or os.getgroups()
    ):
        raise RuntimeError("Invalid embedding worker identity")

    if not stat.S_ISREG(os.fstat(archive_fd).st_mode):
        raise RuntimeError("Invalid inherited archive descriptor")

    install(archive_fd, STORE, EXPECTED_HASH, uid, gid)
    verify_ollama()

    print("OFFLINE_EMBEDDING_REGISTRATION=PASS", flush=True)


if __name__ == "__main__":
    try:
        if len(sys.argv) > 1 and sys.argv[1] == "--embedding-worker":
            embedding_worker()
        else:
            main()
    except Exception as error:
        print(
            "OFFLINE_EMBEDDING_REGISTRATION=FAIL: "
            + str(error),
            file=sys.stderr,
        )
        sys.exit(1)
