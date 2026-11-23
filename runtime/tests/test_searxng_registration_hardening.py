import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/searxng_registration.py"
spec = importlib.util.spec_from_file_location("searxng_hardened", MODULE)
registration = importlib.util.module_from_spec(spec)
spec.loader.exec_module(registration)

CID = "a" * 64


class HardeningTests(unittest.TestCase):
    def root_metadata(self):
        real_fstat = os.fstat

        def mock_fstat(fd):
            info = real_fstat(fd)
            values = list(info)
            values[4] = 0
            return os.stat_result(values)

        return patch.object(
            registration.os, "fstat", side_effect=mock_fstat
        )

    def test_directory_symlink_rejected(self):
        with tempfile.TemporaryDirectory() as root:
            real = Path(root) / "real"
            real.mkdir(mode=0o700)
            link = Path(root) / "link"
            link.symlink_to(real, target_is_directory=True)
            with self.assertRaises(OSError):
                registration._open_trusted_directory(link)

    def test_trusted_verifier_is_fixed(self):
        with patch.object(
            registration.subprocess, "run",
            return_value=type("Result", (), {
                "returncode": 0,
                "stdout": CID + "\n",
            })(),
        ) as invoked:
            self.assertEqual(registration._verify_container(CID), CID)
            self.assertEqual(
                invoked.call_args.args[0][0],
                "/usr/libexec/daia-searxng-verify-container",
            )

    def test_failed_verification_does_not_publish(self):
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root) / "searxng"
            directory.mkdir(mode=0o700)
            with (
                patch("os.geteuid", return_value=0),
                self.root_metadata(),
                patch.object(
                    registration, "_verify_container",
                    side_effect=PermissionError("denied"),
                ),
            ):
                with self.assertRaises(PermissionError):
                    registration.publish_registration(CID, directory)
            self.assertFalse((directory / "container-id").exists())

    def test_exclusive_publication_and_readback(self):
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root) / "searxng"
            directory.mkdir(mode=0o700)
            with (
                patch("os.geteuid", return_value=0),
                self.root_metadata(),
                patch.object(
                    registration, "_verify_container",
                    return_value=CID,
                ),
            ):
                self.assertEqual(
                    registration.publish_registration(CID, directory),
                    CID,
                )
                self.assertEqual(
                    registration.read_registration(directory), CID
                )
                with self.assertRaises(FileExistsError):
                    registration.publish_registration(CID, directory)


if __name__ == "__main__":
    unittest.main()
