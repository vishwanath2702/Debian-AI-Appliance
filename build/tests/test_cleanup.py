import fcntl
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

SOURCE = Path(__file__).resolve().parents[1] / "clean.sh"


@unittest.skipIf(os.geteuid() == 0, "Run normal tests as non-root")
class CleanupTests(unittest.TestCase):

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name).resolve()
        self.work = self.root / "work"
        self.work.mkdir()
        self.build = self.root / "build"
        self.build.mkdir()
        self.script = self.build / "clean.sh"
        shutil.copyfile(SOURCE, self.script)
        self.target = self.work / "daia-fresh-test"
        self.target.mkdir()
        (self.target / "test.txt").write_text("fixture")

    def tearDown(self):
        self.tmp.cleanup()

    def clean(self, *args):
        env = os.environ.copy()
        for key in (
            "DAIA_ACTIVE_WORKSPACE",
            "DAIA_CLEANUP_LOCK_FD",
            "SUDO_UID",
            "SUDO_USER",
            "SUDO_GID",
        ):
            env.pop(key, None)

        return subprocess.run(
            ["bash", str(self.script), *args],
            env=env,
            capture_output=True,
            text=True,
        )

    def execute(self, target, *extra):
        return self.clean(
            "--workspace", str(target),
            "--execute", "--confirm", "DELETE", *extra
        )

    def test_dry_run_preserves_workspace(self):
        result = self.clean("--dry-run")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(self.target.exists())

    def test_normal_cleanup(self):
        result = self.execute(self.target)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(self.target.exists())

    def test_bulk_cleanup_refused(self):
        result = self.clean("--execute", "--confirm", "DELETE")
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(self.target.exists())

    def test_privileged_requires_administrator(self):
        result = self.execute(self.target, "--privileged")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("administrator authorization", result.stderr)
        self.assertTrue(self.target.exists())

    def test_privileged_requires_explicit_target(self):
        result = self.clean(
            "--privileged", "--execute", "--confirm", "DELETE"
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(self.target.exists())

    def test_privileged_automatic_refused(self):
        result = self.execute(
            self.target, "--privileged", "--auto-clean"
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(self.target.exists())

    def test_latest_failed_preserved(self):
        failed = self.work / "daia-fresh-laptop-9999"
        failed.mkdir()
        (failed / ".daia-build-status").write_text("failed")
        result = self.execute(failed)
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(failed.exists())

    def test_outside_workspace_refused(self):
        outside = self.root / "daia-fresh-outside"
        outside.mkdir()
        result = self.execute(outside)
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(outside.exists())

    def test_symlink_target_refused(self):
        link = self.work / "daia-fresh-link"
        link.symlink_to(self.target, target_is_directory=True)
        result = self.execute(link)
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(self.target.exists())

    def test_nested_symlink_not_followed(self):
        outside = self.root / "preserve.txt"
        outside.write_text("preserve")
        (self.target / "link").symlink_to(outside)
        result = self.execute(self.target)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(outside.read_text(), "preserve")

    def test_concurrent_lock_refused(self):
        lock = self.work / ".daia-build-cleanup.lock"
        fd = os.open(lock, os.O_CREAT | os.O_RDWR, 0o600)
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            result = self.execute(self.target)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("holds the workspace lock", result.stderr)
            self.assertTrue(self.target.exists())
        finally:
            os.close(fd)

    def test_confirmation_required(self):
        result = self.clean(
            "--workspace", str(self.target), "--execute"
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(self.target.exists())


if __name__ == "__main__":
    unittest.main()
