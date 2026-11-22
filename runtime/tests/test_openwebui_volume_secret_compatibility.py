"""Offline filesystem compatibility tests for Open WebUI data volumes.

These tests use temporary directories only. They do not use Docker,
access real volumes, change ownership, or modify application data.
"""

import importlib.util
import os
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
ENTRY = ROOT / "runtime/daia-openwebui-rag-entrypoint.py"

spec = importlib.util.spec_from_file_location(
    "daia_openwebui_volume_secret_compatibility",
    ENTRY,
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class OpenWebUIVolumeSecretCompatibilityTests(unittest.TestCase):

    def invoke(self, path):
        with patch.dict(
            os.environ,
            {"WEBUI_SECRET_KEY_FILE": str(path)},
            clear=True,
        ):
            module.ensure_webui_secret_file()
            return os.environ["WEBUI_SECRET_KEY"]

    def test_fresh_root_style_0755_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            original_mode = stat.S_IMODE(root.stat().st_mode)
            try:
                root.chmod(0o755)
                secret_path = root / ".webui_secret_key"

                first = self.invoke(secret_path)
                second = self.invoke(secret_path)

                self.assertEqual(first, second)
                self.assertEqual(
                    stat.S_IMODE(secret_path.stat().st_mode),
                    0o600,
                )
                self.assertEqual(
                    secret_path.stat().st_uid,
                    os.geteuid(),
                )
            finally:
                root.chmod(original_mode)

    def test_existing_volume_data_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            data = root / "webui.db"
            data.write_bytes(b"existing-user-database-fixture")
            original = data.read_bytes()

            secret_path = root / ".webui_secret_key"
            self.invoke(secret_path)

            self.assertEqual(data.read_bytes(), original)
            self.assertTrue(data.is_file())

    def test_existing_secret_preserved_across_launches(self):
        with tempfile.TemporaryDirectory() as directory:
            secret_path = Path(directory) / ".webui_secret_key"
            original = b"persisted-existing-secret-value-1234567890"
            secret_path.write_bytes(original)
            secret_path.chmod(0o600)

            first = self.invoke(secret_path)
            second = self.invoke(secret_path)

            self.assertEqual(first, original.decode("ascii"))
            self.assertEqual(second, first)
            self.assertEqual(secret_path.read_bytes(), original)

    def test_legacy_group_writable_directory_fails_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            original_mode = stat.S_IMODE(root.stat().st_mode)
            data = root / "webui.db"
            data.write_bytes(b"legacy-user-data")
            secret_path = root / ".webui_secret_key"

            try:
                root.chmod(0o775)

                with self.assertRaises(PermissionError):
                    self.invoke(secret_path)

                self.assertEqual(data.read_bytes(), b"legacy-user-data")
                self.assertFalse(secret_path.exists())
                self.assertEqual(
                    stat.S_IMODE(root.stat().st_mode),
                    0o775,
                )
            finally:
                root.chmod(original_mode)

    def test_legacy_other_writable_directory_fails_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            original_mode = stat.S_IMODE(root.stat().st_mode)
            secret_path = root / ".webui_secret_key"

            try:
                root.chmod(0o757)

                with self.assertRaises(PermissionError):
                    self.invoke(secret_path)

                self.assertFalse(secret_path.exists())
                self.assertEqual(
                    stat.S_IMODE(root.stat().st_mode),
                    0o757,
                )
            finally:
                root.chmod(original_mode)

    def test_existing_insecure_secret_is_not_repaired(self):
        with tempfile.TemporaryDirectory() as directory:
            secret_path = Path(directory) / ".webui_secret_key"
            original = b"legacy-insecure-secret-value-123456789"
            secret_path.write_bytes(original)
            secret_path.chmod(0o644)

            with self.assertRaises(PermissionError):
                self.invoke(secret_path)

            self.assertEqual(secret_path.read_bytes(), original)
            self.assertEqual(
                stat.S_IMODE(secret_path.stat().st_mode),
                0o644,
            )

    def test_explicit_secret_override_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            secret_path = Path(directory) / ".webui_secret_key"

            with patch.dict(
                os.environ,
                {
                    "WEBUI_SECRET_KEY_FILE": str(secret_path),
                    "WEBUI_SECRET_KEY": "user-controlled-secret",
                },
                clear=True,
            ):
                module.ensure_webui_secret_file()
                self.assertEqual(
                    os.environ["WEBUI_SECRET_KEY"],
                    "user-controlled-secret",
                )

            self.assertFalse(secret_path.exists())

    def test_no_automatic_directory_permission_repair(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            original_mode = stat.S_IMODE(root.stat().st_mode)

            try:
                root.chmod(0o777)

                with self.assertRaises(PermissionError):
                    self.invoke(root / ".webui_secret_key")

                self.assertEqual(
                    stat.S_IMODE(root.stat().st_mode),
                    0o777,
                )
            finally:
                root.chmod(original_mode)


if __name__ == "__main__":
    unittest.main()
