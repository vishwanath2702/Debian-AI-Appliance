"""Hardened SearXNG firstboot lock regression tests."""

import os
import stat
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

MODULES = Path(__file__).resolve().parents[1] / "modules"
sys.path.insert(0, str(MODULES))

import searxng_firstboot_lock as lock


class FirstbootLockTests(unittest.TestCase):

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)

        self.path = str(
            Path(self.temp.name) / "firstboot.lock"
        )
        self.parent = str(Path(self.path).parent)

        p = patch.object(lock, "LOCK_PATH", self.path)
        p.start()
        self.addCleanup(p.stop)

        p = patch.object(
            lock.os, "geteuid", return_value=0
        )
        p.start()
        self.addCleanup(p.stop)

        original_fstat = os.fstat

        def fixture_fstat(fd):
            info = original_fstat(fd)
            values = list(info)

            # Simulate root ownership without changing files.
            values[4] = 0

            # Normalize /tmp ancestor permissions only.
            # Preserve the final fixture directory's real mode.
            if stat.S_ISDIR(info.st_mode):
                descriptor = os.readlink(
                    "/proc/self/fd/" + str(fd)
                )

                if descriptor != self.parent:
                    values[0] = info.st_mode & ~0o022

            return os.stat_result(values)

        p = patch.object(
            lock.os,
            "fstat",
            side_effect=fixture_fstat,
        )
        p.start()
        self.addCleanup(p.stop)

    def test_exclusive_lock_acquired(self):
        with lock.exclusive_firstboot_lock():
            self.assertTrue(Path(self.path).is_file())

    def test_concurrent_lock_refused(self):
        with lock.exclusive_firstboot_lock():
            with self.assertRaises(lock.LockRefused):
                with lock.exclusive_firstboot_lock():
                    pass

    def test_lock_released_after_exit(self):
        with lock.exclusive_firstboot_lock():
            pass

        with lock.exclusive_firstboot_lock():
            pass

    def test_lock_released_after_exception(self):
        with self.assertRaises(ValueError):
            with lock.exclusive_firstboot_lock():
                raise ValueError("simulated failure")

        with lock.exclusive_firstboot_lock():
            pass

    def test_nonroot_refused(self):
        with patch.object(
            lock.os, "geteuid", return_value=1000
        ):
            with self.assertRaises(lock.LockRefused):
                with lock.exclusive_firstboot_lock():
                    pass

    def test_symlink_refused(self):
        destination = Path(self.temp.name) / "target"
        destination.write_text("preserve")
        Path(self.path).symlink_to(destination)

        with self.assertRaises(lock.LockRefused):
            with lock.exclusive_firstboot_lock():
                pass

        self.assertEqual(
            destination.read_text(), "preserve"
        )

    def test_wrong_permissions_refused(self):
        Path(self.path).touch()
        os.chmod(self.path, 0o644)

        with self.assertRaises(lock.LockRefused):
            with lock.exclusive_firstboot_lock():
                pass

    def test_hardlink_refused(self):
        original = Path(self.temp.name) / "original"
        original.write_text("preserve")
        os.chmod(original, 0o600)
        os.link(original, self.path)

        with self.assertRaises(lock.LockRefused):
            with lock.exclusive_firstboot_lock():
                pass

        self.assertEqual(
            original.read_text(), "preserve"
        )

    def test_replaced_lock_entry_refused(self):
        original_stat = os.stat
        replaced = False

        def replacing_stat(path, *args, **kwargs):
            nonlocal replaced

            if (
                path == "firstboot.lock"
                and kwargs.get("dir_fd") is not None
                and not replaced
            ):
                replaced = True

                os.unlink(self.path)

                fd = os.open(
                    self.path,
                    os.O_WRONLY | os.O_CREAT | os.O_EXCL,
                    0o600,
                )
                os.close(fd)

            return original_stat(
                path, *args, **kwargs
            )

        with patch.object(
            lock.os,
            "stat",
            side_effect=replacing_stat,
        ):
            with self.assertRaises(lock.LockRefused):
                with lock.exclusive_firstboot_lock():
                    pass

        self.assertTrue(replaced)

    def test_unsafe_parent_permissions_refused(self):
        parent = Path(self.parent)
        original_mode = stat.S_IMODE(
            parent.stat().st_mode
        )

        try:
            os.chmod(parent, 0o777)

            with self.assertRaises(lock.LockRefused):
                with lock.exclusive_firstboot_lock():
                    pass

        finally:
            os.chmod(parent, original_mode)

    def test_parent_symlink_refused(self):
        actual = Path(self.temp.name) / "actual"
        actual.mkdir(mode=0o700)

        link = Path(self.temp.name) / "linked"
        link.symlink_to(
            actual,
            target_is_directory=True,
        )

        with patch.object(
            lock,
            "LOCK_PATH",
            str(link / "firstboot.lock"),
        ):
            with self.assertRaises(lock.LockRefused):
                with lock.exclusive_firstboot_lock():
                    pass

    def test_invalid_path_refused(self):
        for path in (
            "/tmp/../unsafe.lock",
            "/tmp//unsafe.lock",
            "relative.lock",
        ):
            with self.subTest(path=path):
                with patch.object(
                    lock, "LOCK_PATH", path
                ):
                    with self.assertRaises(lock.LockRefused):
                        with lock.exclusive_firstboot_lock():
                            pass

    def test_no_provisioning_in_lock_module(self):
        source = (
            MODULES / "searxng_firstboot_lock.py"
        ).read_text()

        self.assertNotIn("subprocess", source)
        self.assertNotIn("docker ", source)
        self.assertNotIn("systemctl", source)


if __name__ == "__main__":
    unittest.main()
