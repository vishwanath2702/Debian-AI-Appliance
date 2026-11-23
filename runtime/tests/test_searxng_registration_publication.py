import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/searxng_registration.py"

spec = importlib.util.spec_from_file_location(
    "searxng_registration_publication", MODULE
)
registration = importlib.util.module_from_spec(spec)
spec.loader.exec_module(registration)

CID = "a" * 64


class PublicationTests(unittest.TestCase):
    def directory(self, root):
        path = Path(root) / "searxng"
        path.mkdir(mode=0o700)
        path.chmod(0o700)
        return path

    def simulated_root(self):
        real_fstat = os.fstat

        def mock_fstat(fd):
            info = real_fstat(fd)
            values = list(info)
            values[4] = 0
            return os.stat_result(values)

        return (
            patch("os.geteuid", return_value=0),
            patch.object(registration.os, "fstat",
                         side_effect=mock_fstat),
        )


    def test_valid_publication(self):
        with tempfile.TemporaryDirectory() as root:
            directory = self.directory(root)
            a, b = self.simulated_root()
            with a, b, patch.object(registration, '_verify_container', side_effect=lambda value: value):
                self.assertEqual(
                    registration.publish_registration(
                        CID, directory
                    ),
                    CID,
                )

            record = directory / "container-id"
            self.assertEqual(
                record.read_bytes(),
                CID.encode("ascii") + bytes([10]),
            )
            self.assertEqual(record.stat().st_mode & 0o777, 0o600)

    def test_existing_registration_not_overwritten(self):
        with tempfile.TemporaryDirectory() as root:
            directory = self.directory(root)
            record = directory / "container-id"
            record.write_text("existing", encoding="ascii")

            a, b = self.simulated_root()
            with a, b, patch.object(registration, '_verify_container', side_effect=lambda value: value):
                with self.assertRaises(FileExistsError):
                    registration.publish_registration(
                        CID, directory
                    )

            self.assertEqual(
                record.read_text(encoding="ascii"),
                "existing",
            )

    def test_verifier_rejection(self):
        with tempfile.TemporaryDirectory() as root:
            directory = self.directory(root)
            a, b = self.simulated_root()
            with a, b, patch.object(
                registration, "_verify_container",
                side_effect=PermissionError("verification rejected"),
            ):
                with self.assertRaises(PermissionError):
                    registration.publish_registration(CID, directory)
            self.assertFalse((directory / "container-id").exists())


    def test_invalid_identity(self):
        with tempfile.TemporaryDirectory() as root:
            directory = self.directory(root)
            a, b = self.simulated_root()

            with a, b:
                with self.assertRaises(PermissionError):
                    registration.publish_registration(
                        "invalid", directory
                    )

    def test_symlinked_directory_rejected(self):
        with tempfile.TemporaryDirectory() as root:
            real = self.directory(root)
            link = Path(root) / "linked"
            link.symlink_to(real, target_is_directory=True)

            a, b = self.simulated_root()
            with a, b:
                with self.assertRaises(OSError):
                    registration.publish_registration(
                        CID, link
                    )

    def test_non_root_directory_rejected(self):
        if os.geteuid() == 0:
            self.skipTest("Requires unprivileged account")

        with tempfile.TemporaryDirectory() as root:
            directory = self.directory(root)
            with patch("os.geteuid", return_value=0):
                with self.assertRaises(PermissionError):
                    registration.publish_registration(
                        CID, directory
                    )

    def test_existing_symlink_not_replaced(self):
        with tempfile.TemporaryDirectory() as root:
            directory = self.directory(root)
            target = directory / "target"
            target.write_text("preserve", encoding="ascii")
            (directory / "container-id").symlink_to(target)

            a, b = self.simulated_root()
            with a, b, patch.object(registration, '_verify_container', side_effect=lambda value: value):
                with self.assertRaises(FileExistsError):
                    registration.publish_registration(
                        CID, directory
                    )

            self.assertEqual(target.read_text(), "preserve")


if __name__ == "__main__":
    unittest.main()
