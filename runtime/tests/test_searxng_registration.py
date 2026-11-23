import importlib.util
import os
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/searxng_registration.py"

spec = importlib.util.spec_from_file_location(
    "searxng_registration", MODULE
)
registration = importlib.util.module_from_spec(spec)
spec.loader.exec_module(registration)

CID = "a" * 64


class RegistrationTests(unittest.TestCase):
    def make_registration(self, root):
        directory = Path(root) / "searxng"
        directory.mkdir(mode=0o700)
        directory.chmod(0o700)

        record = directory / "container-id"
        record.write_bytes(CID.encode("ascii") + bytes([10]))
        record.chmod(0o600)
        return directory, record

    def simulated_root_ownership(self):
        real_fstat = os.fstat

        def mock_fstat(fd):
            info = real_fstat(fd)
            values = list(info)
            values[4] = 0
            return os.stat_result(values)

        return (
            patch.object(registration.os, "fstat",
                         side_effect=mock_fstat),
            patch("os.geteuid", return_value=0),
        )


    def test_valid_registration(self):
        with tempfile.TemporaryDirectory() as root:
            directory, _ = self.make_registration(root)
            a, b = self.simulated_root_ownership()
            with a, b:
                self.assertEqual(
                    registration.read_registration(directory),
                    CID,
                )

    def test_missing_registration(self):
        with tempfile.TemporaryDirectory() as root:
            with self.assertRaises(OSError):
                registration.read_registration(root)

    def test_symlinked_registration_rejected(self):
        with tempfile.TemporaryDirectory() as root:
            directory, record = self.make_registration(root)
            record.rename(directory / "actual-id")
            record.symlink_to(directory / "actual-id")
            a, b = self.simulated_root_ownership()
            with a, b:
                with self.assertRaises(OSError):
                    registration.read_registration(directory)

    def test_wrong_file_permissions(self):
        with tempfile.TemporaryDirectory() as root:
            directory, record = self.make_registration(root)
            record.chmod(0o644)
            a, b = self.simulated_root_ownership()
            with a, b:
                with self.assertRaises(PermissionError):
                    registration.read_registration(directory)

    def test_wrong_directory_permissions(self):
        with tempfile.TemporaryDirectory() as root:
            directory, _ = self.make_registration(root)
            directory.chmod(0o755)
            a, b = self.simulated_root_ownership()
            with a, b:
                with self.assertRaises(PermissionError):
                    registration.read_registration(directory)

    def test_non_root_directory_rejected(self):
        if os.geteuid() == 0:
            self.skipTest("Requires unprivileged test account")
        with tempfile.TemporaryDirectory() as root:
            directory, _ = self.make_registration(root)
            with self.assertRaises(PermissionError):
                registration.read_registration(directory)

    def test_non_root_file_rejected(self):
        if os.geteuid() == 0:
            self.skipTest("Requires unprivileged test account")
        with tempfile.TemporaryDirectory() as root:
            directory, _ = self.make_registration(root)
            real_fstat = os.fstat

            def directory_only_root(fd):
                info = real_fstat(fd)
                values = list(info)
                if stat.S_ISDIR(info.st_mode):
                    values[4] = 0
                return os.stat_result(values)

            with patch.object(registration.os, "fstat",
                              side_effect=directory_only_root):
                with self.assertRaises(PermissionError):
                    registration.read_registration(directory)


    def test_malformed_identity(self):
        with tempfile.TemporaryDirectory() as root:
            directory, record = self.make_registration(root)
            record.write_bytes(b"invalid" + bytes([10]))
            a, b = self.simulated_root_ownership()
            with a, b:
                with self.assertRaises(PermissionError):
                    registration.read_registration(directory)

    def test_identity_mismatch(self):
        with tempfile.TemporaryDirectory() as root:
            directory, _ = self.make_registration(root)
            a, b = self.simulated_root_ownership()
            with a, b, patch("os.geteuid", return_value=0):
                with self.assertRaises(PermissionError):
                    registration.verify_registration(
                        "b" * 64, directory,
                    )

    def test_verifier_failure(self):
        with tempfile.TemporaryDirectory() as root:
            directory, _ = self.make_registration(root)
            a, b = self.simulated_root_ownership()

            class Result:
                returncode = 1
                stdout = ""

            with a, b, patch("os.geteuid", return_value=0):
                with patch.object(
                    registration.subprocess,
                    "run",
                    return_value=Result(),
                ):
                    with self.assertRaises(PermissionError):
                        registration.verify_registration(
                            CID, directory,
                        )

    def test_valid_verifier_response(self):
        with tempfile.TemporaryDirectory() as root:
            directory, _ = self.make_registration(root)
            a, b = self.simulated_root_ownership()

            class Result:
                returncode = 0
                stdout = CID + chr(10)

            with a, b, patch("os.geteuid", return_value=0):
                with patch.object(
                    registration.subprocess,
                    "run",
                    return_value=Result(),
                ):
                    self.assertEqual(
                        registration.verify_registration(
                            CID, directory,
                        ),
                        CID,
                    )


if __name__ == "__main__":
    unittest.main()
