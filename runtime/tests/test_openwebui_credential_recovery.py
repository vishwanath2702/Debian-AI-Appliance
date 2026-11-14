"""Recovery must never overwrite or adopt unsafe credentials."""

import importlib.util
import os
import stat
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch

SOURCE = (
    Path(__file__).resolve().parents[1]
    / "daia-openwebui-pgvector-credentials.py"
)
spec = importlib.util.spec_from_file_location(
    "pragna_credential_recovery", SOURCE
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class RecoveryTests(unittest.TestCase):
    def test_nonroot_rejected_before_access(self):
        with (
            patch.object(module.os, "geteuid", return_value=1000),
            patch.object(module, "read_existing_credential") as reader,
        ):
            with self.assertRaises(PermissionError):
                module.ensure_credential()
            reader.assert_not_called()

    def test_valid_existing_secret_is_reused(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module, "read_existing_credential",
                         return_value=b"A" * 64) as reader,
            patch.object(module, "provision") as creator,
        ):
            result = module.ensure_credential()
            self.assertEqual(
                result, module.STATE_DIRECTORY / module.SECRET_NAME
            )
            reader.assert_called_once()
            creator.assert_not_called()

    def test_missing_secret_created_then_verified(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(
                module, "read_existing_credential",
                side_effect=[FileNotFoundError(), b"A" * 64],
            ) as reader,
            patch.object(module, "provision") as creator,
        ):
            module.ensure_credential()
            creator.assert_called_once()
            self.assertEqual(reader.call_count, 2)

    def test_invalid_secret_never_replaced(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(
                module, "read_existing_credential",
                side_effect=PermissionError("unsafe"),
            ),
            patch.object(module, "provision") as creator,
        ):
            with self.assertRaises(PermissionError):
                module.ensure_credential()
            creator.assert_not_called()

    def test_creation_race_fails_closed(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(
                module, "read_existing_credential",
                side_effect=FileNotFoundError(),
            ),
            patch.object(
                module, "provision",
                side_effect=FileExistsError(),
            ),
        ):
            with self.assertRaises(FileExistsError):
                module.ensure_credential()

    def test_existing_secret_requires_regular_private_file(self):
        source = SOURCE.read_text(encoding="utf-8")
        for required in (
            "stat.S_ISREG(before.st_mode)",
            "before.st_uid != 0",
            "before.st_gid != 0",
            "before.st_nlink != 1",
            "item.st_mtime_ns",
            "item.st_ctime_ns",
            "os.O_NOFOLLOW",
            "os.O_NONBLOCK",
        ):
            with self.subTest(required=required):
                self.assertIn(required, source)

    def test_create_only_function_is_preserved(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertIn("os.O_EXCL", source)
        self.assertIn("def provision(", source)

    def test_no_implicit_database_role_adoption(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertNotIn("CREATE ROLE", source)
        self.assertNotIn("ALTER ROLE", source)
        self.assertNotIn("docker exec", source)


if __name__ == "__main__":
    unittest.main()
