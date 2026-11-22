"""Fail-closed security tests for pinned Open WebUI middleware."""
import hashlib
import importlib.machinery
import importlib.util
import os
import stat
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
RUNTIME = ROOT / "runtime"
REGISTER = RUNTIME / "daia-openwebui-register-container"
MODULE = ROOT / "payload/patches/open-webui/v0.13.0/patched-middleware.py"

sys.path.insert(0, str(RUNTIME))

loader = importlib.machinery.SourceFileLoader(
    "openwebui_middleware_security", str(REGISTER)
)
spec = importlib.util.spec_from_loader(loader.name, loader)
registration = importlib.util.module_from_spec(spec)
loader.exec_module(registration)


class MiddlewareIntegritySecurityTests(unittest.TestCase):
    def trusted_metadata(self, *, uid=0, gid=0, mode=0o100644):
        real_fstat = os.fstat

        def fake_fstat(fd):
            original = real_fstat(fd)
            fields = list(original)
            fields[0] = mode
            fields[4] = uid
            fields[5] = gid
            return os.stat_result(fields)

        return patch.object(
            registration.os, "fstat", side_effect=fake_fstat
        )

    def test_pinned_digest(self):
        self.assertEqual(
            hashlib.sha256(MODULE.read_bytes()).hexdigest(),
            registration.OPENWEBUI_MIDDLEWARE_SHA256,
        )

    def test_missing_file_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(OSError):
                registration.verify_middleware_module_integrity(
                    Path(tmp) / "missing.py"
                )

    def test_symlink_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            link = Path(tmp) / "link.py"
            link.symlink_to(MODULE)
            with self.assertRaises(OSError):
                registration.verify_middleware_module_integrity(link)

    def test_valid_pinned_file_accepted(self):
        with self.trusted_metadata():
            self.assertTrue(
                registration.verify_middleware_module_integrity(MODULE)
            )

    def test_nonroot_owner_rejected(self):
        with self.trusted_metadata(uid=1000):
            with self.assertRaisesRegex(
                PermissionError, "ownership mismatch"
            ):
                registration.verify_middleware_module_integrity(MODULE)

    def test_nonroot_group_rejected(self):
        with self.trusted_metadata(gid=1000):
            with self.assertRaisesRegex(
                PermissionError, "ownership mismatch"
            ):
                registration.verify_middleware_module_integrity(MODULE)

    def test_group_writable_rejected(self):
        with self.trusted_metadata(mode=stat.S_IFREG | 0o664):
            with self.assertRaisesRegex(
                PermissionError, "permissions unsafe"
            ):
                registration.verify_middleware_module_integrity(MODULE)

    def test_world_writable_rejected(self):
        with self.trusted_metadata(mode=stat.S_IFREG | 0o646):
            with self.assertRaisesRegex(
                PermissionError, "permissions unsafe"
            ):
                registration.verify_middleware_module_integrity(MODULE)

    def test_nonregular_file_rejected(self):
        with self.trusted_metadata(mode=stat.S_IFDIR | 0o755):
            with self.assertRaisesRegex(
                PermissionError, "not regular"
            ):
                registration.verify_middleware_module_integrity(MODULE)

    def test_tampered_content_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            altered = Path(tmp) / "middleware.py"
            altered.write_bytes(
                MODULE.read_bytes() + b"\n# tampered\n"
            )
            with self.trusted_metadata():
                with self.assertRaisesRegex(
                    PermissionError, "checksum mismatch"
                ):
                    registration.verify_middleware_module_integrity(
                        altered
                    )

    def test_mount_inspection_requires_middleware_integrity(self):
        with patch.object(
            registration, "verify_retrieval_module_integrity",
            return_value=True,
        ):
            with patch.object(
                registration, "verify_middleware_module_integrity",
                side_effect=PermissionError("middleware rejected"),
            ) as check:
                with patch.object(
                    registration.subprocess, "run"
                ) as docker:
                    with self.assertRaisesRegex(
                        PermissionError, "middleware rejected"
                    ):
                        registration.inspect_container_mounts(
                            "a" * 64
                        )
                    check.assert_called_once_with()
                    docker.assert_not_called()

    def test_middleware_cli_requires_root(self):
        with patch.object(
            registration.os, "geteuid", return_value=1000
        ):
            with patch.object(
                registration.sys, "argv",
                ["register-container", "--verify-middleware"],
            ):
                with patch.object(
                    registration,
                    "verify_middleware_module_integrity"
                ) as check:
                    with self.assertRaises(PermissionError):
                        registration.main()
                    check.assert_not_called()


if __name__ == "__main__":
    unittest.main()
