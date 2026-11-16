#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
export PROJECT_ROOT

exec python3 - "$@" <<'CLEANUP_PY'
import os
import sys
import stat
import shutil
import fcntl
from pathlib import Path

root = Path(os.environ["PROJECT_ROOT"])
work = root / "work"
args = sys.argv[1:]

def refuse(message):
    raise SystemExit("CLEANUP REFUSED: " + message)

def mounts():
    result = set()
    for line in Path("/proc/self/mountinfo").read_text().splitlines():
        fields = line.split()
        if len(fields) >= 5:
            result.add(fields[4].replace("\\040", " "))
    return result

def safe_directory(path):
    if path.is_symlink() or not path.is_dir():
        refuse("unsafe or missing directory: " + str(path))
    if path.resolve() != path:
        refuse("noncanonical directory: " + str(path))

def validate(path, mounted):
    if path.parent != work:
        refuse("workspace must be a direct child of work/")
    if not (
        path.name.startswith("daia-fresh-")
        or path.name.startswith("daia-refresh-")
    ):
        refuse("workspace name is not approved")
    safe_directory(path)
    prefix = str(path) + "/"
    if any(m == str(path) or m.startswith(prefix) for m in mounted):
        refuse("workspace contains a mounted filesystem: " + path.name)

def verify_removal_access(target):
    # Conservative preflight. This does not make deletion atomic.
    # Refuse inaccessible directories before starting rmtree.
    for current, directories, files in os.walk(
        target, topdown=True, followlinks=False
    ):
        directory = Path(current)
        metadata = directory.lstat()
        if not stat.S_ISDIR(metadata.st_mode):
            refuse("unsafe cleanup directory: " + str(directory))
        if not os.access(directory, os.R_OK | os.W_OK | os.X_OK):
            refuse("cleanup permissions insufficient: " + str(directory))
        for name in directories:
            child = directory / name
            metadata = child.lstat()
            if stat.S_ISLNK(metadata.st_mode):
                continue
            if not stat.S_ISDIR(metadata.st_mode):
                refuse("unsafe cleanup entry: " + str(child))
            if not os.access(child, os.R_OK | os.W_OK | os.X_OK):
                refuse("cleanup permissions insufficient: " + str(child))


def usage():
    print("Usage:")
    print("  ./builder.sh clean --dry-run")
    print("  ./builder.sh clean --workspace ABSOLUTE_PATH")
    print("  ./builder.sh clean --workspace ABSOLUTE_PATH --execute --confirm DELETE")
    print("  ./builder.sh clean --execute --confirm DELETE")
    raise SystemExit(2)

try:
    execute = False
    confirmed = False
    selected = None
    lock_held = False
    auto_cleanup = False
    i = 0
    while i < len(args):
        arg = args[i]
        if arg == "--dry-run":
            pass
        elif arg == "--lock-held":
            lock_held = True
        elif arg == "--auto-clean":
            auto_cleanup = True
        elif arg == "--workspace":
            i += 1
            if i >= len(args) or selected is not None:
                usage()
            selected = args[i]
        elif arg == "--execute":
            execute = True
        elif arg == "--confirm":
            i += 1
            if i >= len(args) or args[i] != "DELETE":
                refuse("confirmation token must be DELETE")
            confirmed = True
        else:
            usage()
        i += 1

    if execute != confirmed:
        refuse("--execute requires --confirm DELETE")
    if auto_cleanup and selected is not None:
        refuse("automatic cleanup does not accept --workspace")
    if auto_cleanup and execute and not lock_held:
        refuse("automatic deletion requires inherited build lock")

    safe_directory(root)
    safe_directory(work)

    lock_path = work / ".daia-build-cleanup.lock"
    lock_fd = os.open(
        lock_path,
        os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_CLOEXEC,
        0o600,
    )
    lock_stat = os.fstat(lock_fd)
    if not stat.S_ISREG(lock_stat.st_mode):
        refuse("cleanup lock is not a regular file")
    if lock_stat.st_uid != os.geteuid():
        refuse("cleanup lock has unexpected ownership")
    if lock_stat.st_mode & 0o077:
        refuse("cleanup lock permissions are unsafe")

    if lock_held:
        inherited = os.environ.get("DAIA_CLEANUP_LOCK_FD", "")
        if inherited != "9":
            refuse("inherited cleanup lock descriptor is required")
        try:
            held = os.fstat(9)
        except OSError:
            refuse("inherited cleanup lock descriptor is missing")
        if (held.st_dev, held.st_ino) != (
            lock_stat.st_dev, lock_stat.st_ino
        ):
            refuse("inherited cleanup lock does not match")
        # An inherited descriptor must also carry an active flock.
        # flock locks are associated with the open file description.
        probe = os.open(
            lock_path,
            os.O_RDWR | os.O_NOFOLLOW | os.O_CLOEXEC,
        )
        try:
            try:
                fcntl.flock(probe, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                pass
            else:
                fcntl.flock(probe, fcntl.LOCK_UN)
                refuse("inherited descriptor does not hold the lock")
        finally:
            os.close(probe)
    else:
        try:
            fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            refuse("another build or cleanup holds the workspace lock")

    mounted = mounts()

    active_override = os.environ.get("DAIA_ACTIVE_WORKSPACE", "")
    if active_override and not Path(active_override).is_absolute():
        refuse("DAIA_ACTIVE_WORKSPACE must be absolute")

    laptop_workspaces = sorted(
        (p for p in work.iterdir()
         if p.name.startswith("daia-fresh-laptop-")
         and p.is_dir() and not p.is_symlink()),
        key=lambda p: p.name
    )

    failed = []
    for candidate in laptop_workspaces:
        marker = candidate / ".daia-build-status"
        if marker.is_symlink():
            refuse("unsafe build-status marker: " + str(marker))
        if marker.is_file():
            status = marker.read_text().strip()
            if status not in ("running", "failed", "succeeded"):
                refuse("invalid build status: " + str(marker))
            if status == "failed":
                failed.append(candidate)

    protected = set(failed[-1:])
    protected.update(
        p for p in laptop_workspaces
        if not (p / ".daia-build-status").exists()
    )
    protected.update(
        p for p in laptop_workspaces
        if (p / ".daia-build-status").is_file()
        and (p / ".daia-build-status").read_text().strip() == "running"
    )

    if active_override:
        protected.add(Path(active_override))

    if selected:
        path = Path(selected)
        if not path.is_absolute() or str(path) != os.path.normpath(str(path)):
            refuse("workspace path must be absolute and normalized")
        candidates = [path]
    else:
        candidates = sorted(
            p for p in work.iterdir()
            if p.is_dir() and not p.is_symlink()
            and (p.name.startswith("daia-fresh-")
                 or p.name.startswith("daia-refresh-"))
        )

    print("CLEANUP_MODE=" + ("EXECUTE" if execute else "DRY_RUN"))
    print("ACTIVE_PROTECTION=" +
          ", ".join(str(p) for p in sorted(protected)))

    approved = []
    for path in candidates:
        if path in protected:
            print("PRESERVE_ACTIVE=" + str(path))
            continue
        validate(path, mounted)
        approved.append(path)
        print("CANDIDATE=" + str(path))

    print("CANDIDATE_COUNT=" + str(len(approved)))

    if auto_cleanup:
        eligible = []
        for target in approved:
            marker = target / ".daia-build-status"
            if marker.is_symlink() or not marker.is_file():
                continue
            status = marker.read_text().strip()
            if status not in ("failed", "succeeded"):
                continue
            eligible.append(target)

        print("AUTO_ELIGIBLE_COUNT=" + str(len(eligible)))

        if execute:
            work_fd = os.open(
                work,
                os.O_RDONLY | os.O_DIRECTORY |
                os.O_NOFOLLOW | os.O_CLOEXEC,
            )
            try:
                for target in eligible:
                    validate(target, mounts())

                    before = os.stat(
                        target.name, dir_fd=work_fd,
                        follow_symlinks=False,
                    )
                    if not stat.S_ISDIR(before.st_mode):
                        refuse("auto-clean target is not a directory")

                    marker = target / ".daia-build-status"
                    if marker.is_symlink() or not marker.is_file():
                        refuse("unsafe auto-clean status marker")
                    if marker.read_text().strip() not in (
                        "failed", "succeeded"
                    ):
                        refuse("auto-clean status changed")

                    validate(target, mounts())
                    after = os.stat(
                        target.name, dir_fd=work_fd,
                        follow_symlinks=False,
                    )
                    if (before.st_dev, before.st_ino) != (
                        after.st_dev, after.st_ino
                    ):
                        refuse("auto-clean target changed")

                    verify_removal_access(target)
                    shutil.rmtree(target.name, dir_fd=work_fd)
                    print("AUTO_DELETED=" + str(target))
            finally:
                os.close(work_fd)
        else:
            for target in eligible:
                print("AUTO_CANDIDATE=" + str(target))
            print("NO_CHANGES_MADE=True")

        raise SystemExit(0)

    if execute:
        if selected is None:
            refuse("bulk deletion is disabled; select one workspace")
        if len(approved) != 1:
            refuse("no eligible workspace selected")

        target = approved[0]
        validate(target, mounts())

        work_fd = os.open(
            work,
            os.O_RDONLY | os.O_DIRECTORY |
            os.O_NOFOLLOW | os.O_CLOEXEC,
        )
        try:
            parent_stat = os.fstat(work_fd)
            current_parent = os.stat(work, follow_symlinks=False)
            if (parent_stat.st_dev, parent_stat.st_ino) != (
                current_parent.st_dev, current_parent.st_ino
            ):
                refuse("workspace parent changed")

            before = os.stat(
                target.name, dir_fd=work_fd,
                follow_symlinks=False,
            )
            if not stat.S_ISDIR(before.st_mode):
                refuse("target is not a directory")

            validate(target, mounts())
            after = os.stat(
                target.name, dir_fd=work_fd,
                follow_symlinks=False,
            )
            if (before.st_dev, before.st_ino) != (
                after.st_dev, after.st_ino
            ):
                refuse("target changed during validation")

            verify_removal_access(target)
            shutil.rmtree(target.name, dir_fd=work_fd)
        finally:
            os.close(work_fd)

        print("DELETED=" + str(target))
    else:
        print("NO_CHANGES_MADE=True")

except (OSError, ValueError) as exc:
    refuse(str(exc))
CLEANUP_PY
