"""Administrator-authorized cleanup integration tests.

Run as a normal development user with explicit sudo approval:

    sudo -v
    DAIA_TEST_PRIVILEGED_CLEANUP=1 \
        python3 -B build/tests/test_cleanup_privileged.py -v

All deletion tests operate on disposable temporary fixtures.
Never run these tests against the actual build workspace.
"""

import fcntl
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

SOURCE = Path(__file__).resolve().parents[1] / "clean.sh"


@unittest.skipUnless(
    os.environ.get("DAIA_TEST_PRIVILEGED_CLEANUP") == "1",
    "Explicit privileged test approval required"
)
class PrivilegedCleanupTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        if os.geteuid() == 0:
            raise RuntimeError("Run as a normal sudo-capable user")
        subprocess.run(["sudo", "-n", "true"], check=True)

    def setUp(self):
        self.root = Path(tempfile.mkdtemp(
            prefix="loom-cleanup-regression-"
        )).resolve()

        self.addCleanup(self.remove_fixture)

        self.work = self.root / "work"
        self.work.mkdir(mode=0o700)

        build = self.root / "build"
        build.mkdir(mode=0o700)

        self.script = build / "clean.sh"
        shutil.copyfile(SOURCE, self.script)

        self.lock = self.work / ".daia-build-cleanup.lock"
        self.lock.touch(mode=0o600, exist_ok=False)

        self.target = self.work / "daia-fresh-privileged-test"
        self.target.mkdir()

        self.rootfs = self.target / "rootfs"

        subprocess.run([
            "sudo", "-n", "mkdir", "-m", "755",
            str(self.rootfs)
        ], check=True)

        subprocess.run([
            "sudo", "-n", "touch",
            str(self.rootfs / "root-owned")
        ], check=True)

        self.protected = self.work / "daia-fresh-laptop-9999"
        self.protected.mkdir()

        (self.protected / ".daia-build-status").write_text(
            "failed"
        )

        self.sentinel = self.root / "preserve.txt"
        self.sentinel.write_text("preserve")

        (self.target / "outside-link").symlink_to(self.sentinel)

    def command(self, target, privileged=False):
        args = [
            "bash", str(self.script),
            "--workspace", str(target),
            "--execute", "--confirm", "DELETE"
        ]

        if privileged:
            args = ["sudo", "-n", *args, "--privileged"]

        return args

    def execute(self, args):
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
            args,
            env=env,
            text=True,
            capture_output=True,
            timeout=60
        )

    def assert_refused(self, result, message):
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            message, result.stdout + result.stderr
        )

    def remove_fixture(self):
        # Use Loom itself to remove the root-owned fixture.
        # Never use a broad privileged recursive deletion.
        if hasattr(self, "target") and self.target.exists():
            result = self.execute(
                self.command(self.target, privileged=True)
            )
            if result.returncode:
                raise RuntimeError(
                    f"Fixture retained at {self.root}: "
                    + result.stdout + result.stderr
                )

        shutil.rmtree(self.root)

    def test_unprivileged_rootfs_refusal(self):
        result = self.execute(self.command(self.target))
        self.assert_refused(
            result, "cleanup permissions insufficient"
        )
        self.assertTrue(self.rootfs.is_dir())

    def test_privileged_concurrent_lock_refusal(self):
        with self.lock.open("r+") as held:
            fcntl.flock(held, fcntl.LOCK_EX | fcntl.LOCK_NB)

            result = self.execute(
                self.command(self.target, privileged=True)
            )

            self.assert_refused(
                result, "holds the workspace lock"
            )

        self.assertTrue(self.target.is_dir())

    def test_privileged_protected_workspace_refusal(self):
        result = self.execute(
            self.command(self.protected, privileged=True)
        )

        self.assert_refused(
            result, "no eligible workspace selected"
        )

        self.assertTrue(self.protected.is_dir())

    def test_privileged_mounted_workspace_refusal(self):
        result = self.execute([
            "sudo", "-n", "unshare", "--mount",
            "--propagation", "private",
            "bash", "-c",
            'mount --bind "$1" "$1" && shift && exec "$@"',
            "fixture-test", str(self.rootfs),
            "bash", str(self.script),
            "--workspace", str(self.target),
            "--privileged", "--execute", "--confirm", "DELETE"
        ])

        self.assert_refused(
            result, "workspace contains a mounted filesystem"
        )

        self.assertTrue(self.target.is_dir())

    def test_privileged_deletion_and_preservation(self):
        before = self.lock.stat()

        result = self.execute(
            self.command(self.target, privileged=True)
        )

        self.assertEqual(
            result.returncode, 0,
            result.stdout + result.stderr
        )

        self.assertFalse(os.path.lexists(self.target))
        self.assertTrue(self.protected.is_dir())
        self.assertEqual(self.sentinel.read_text(), "preserve")

        after = self.lock.stat()

        for attr in ("st_ino", "st_dev", "st_uid", "st_mode"):
            self.assertEqual(
                getattr(before, attr), getattr(after, attr)
            )


if __name__ == "__main__":
    unittest.main()
