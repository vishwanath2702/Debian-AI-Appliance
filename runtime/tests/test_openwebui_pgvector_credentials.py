"""Open WebUI application credential tests."""

import os
import tempfile
import unittest
from pathlib import Path
import importlib.util
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "runtime/daia-openwebui-pgvector-credentials.py"

spec = importlib.util.spec_from_file_location("credential_helper", HELPER)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class OpenWebUICredentialTests(unittest.TestCase):

    def setUp(self):
        from unittest.mock import patch
        import stat

        real_fstat = os.fstat
        tmp_metadata = os.stat("/tmp")

        def simulated_root_directory(fd):
            metadata = real_fstat(fd)
            if not stat.S_ISDIR(metadata.st_mode):
                return metadata
            fields = list(metadata)
            # Simulate a trusted /tmp parent only inside this test.
            # Preserve real directory traversal and O_NOFOLLOW.
            if (
                metadata.st_dev == tmp_metadata.st_dev
                and metadata.st_ino == tmp_metadata.st_ino
            ):
                fields[0] = stat.S_IFDIR | 0o755
            fields[4] = 0
            fields[5] = 0
            return os.stat_result(fields)

        ownership = patch.object(
            module.os, "fstat",
            side_effect=simulated_root_directory,
        )
        ownership.start()
        self.addCleanup(ownership.stop)

    def temporary_credential_directory(self):
        # /tmp is deliberately used only inside this isolated fixture.
        # Directory metadata is simulated; symlink traversal remains real.
        return tempfile.TemporaryDirectory(
            prefix="daia-credentials-"
        )

    def test_provisions_private_secret(self):
        with tempfile.TemporaryDirectory(
            prefix="daia-credentials-",
        ) as directory:
            root = Path(directory)
            root.chmod(0o700)

            destination = module.provision(root)

            self.assertEqual(destination.name, "openwebui-password")
            self.assertEqual(destination.stat().st_mode & 0o777, 0o600)
            self.assertGreater(len(destination.read_bytes()), 40)

    def test_refuses_existing_secret(self):
        with tempfile.TemporaryDirectory(
            prefix="daia-credentials-",
        ) as directory:
            root = Path(directory)
            root.chmod(0o700)

            module.provision(root)

            with self.assertRaises(FileExistsError):
                module.provision(root)

    def test_refuses_permissive_directory(self):
        with tempfile.TemporaryDirectory(
            prefix="daia-credentials-",
        ) as directory:
            root = Path(directory)
            root.chmod(0o755)

            with self.assertRaises(PermissionError):
                module.provision(root)

    def test_refuses_symlinked_parent(self):
        with tempfile.TemporaryDirectory(
            prefix="daia-credentials-",
        ) as directory:
            root = Path(directory)
            root.chmod(0o700)
            actual = root / "actual"
            actual.mkdir(mode=0o700)
            link = root / "linked"
            link.symlink_to(actual, target_is_directory=True)

            with self.assertRaises((OSError, ValueError)):
                module.provision(link / "child")

    def test_refuses_symlink_directory(self):
        with tempfile.TemporaryDirectory(
            prefix="daia-credentials-",
        ) as directory:
            root = Path(directory)
            root.chmod(0o700)
            link = root.parent / (root.name + "-link")
            link.symlink_to(root, target_is_directory=True)

            try:
                with self.assertRaises((OSError, ValueError)):
                    module.provision(link)
            finally:
                link.unlink()


if __name__ == "__main__":
    unittest.main()
