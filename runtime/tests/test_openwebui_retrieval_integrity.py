"""Security regressions for the pinned retrieval module."""

import hashlib
import importlib.machinery
import importlib.util
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
RUNTIME = ROOT / "runtime"
REGISTER = RUNTIME / "daia-openwebui-register-container"
MODULE = ROOT / "payload/patches/open-webui/v0.13.0/patched-utils.py"

sys.path.insert(0, str(RUNTIME))
loader = importlib.machinery.SourceFileLoader(
    "daia_openwebui_retrieval_integrity", str(REGISTER)
)
spec = importlib.util.spec_from_loader(loader.name, loader)
registration = importlib.util.module_from_spec(spec)
loader.exec_module(registration)


class RetrievalIntegrityTests(unittest.TestCase):
    def test_pinned_digest(self):
        self.assertEqual(
            hashlib.sha256(MODULE.read_bytes()).hexdigest(),
            registration.OPENWEBUI_RETRIEVAL_SHA256,
        )

    def test_missing_file_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(OSError):
                registration.verify_retrieval_module_integrity(
                    Path(directory) / "missing.py"
                )

    def test_symlink_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "target.py"
            target.write_text("hello")
            link = Path(directory) / "link.py"
            link.symlink_to(target)
            with self.assertRaises(OSError):
                registration.verify_retrieval_module_integrity(link)

    def test_nonroot_file_rejected(self):
        if os.geteuid() == 0:
            self.skipTest("requires nonroot test user")
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "utils.py"
            target.write_text("hello")
            with self.assertRaises(PermissionError):
                registration.verify_retrieval_module_integrity(target)

    def test_tampered_file_rejected(self):
        """A metadata-valid file must still fail SHA-256 verification."""
        from unittest.mock import patch

        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "utils.py"
            target.write_text("tampered")

            real_fstat = registration.os.fstat

            def trusted_fstat(fd):
                actual = real_fstat(fd)
                values = list(actual)
                values[0] = 0o100644  # Regular file, safe permissions
                values[4] = 0         # Root UID
                values[5] = 0         # Root GID
                return type(actual)(values)

            with patch.object(
                registration.os,
                "fstat",
                side_effect=trusted_fstat,
            ) as mocked_fstat:
                with self.assertRaisesRegex(
                    PermissionError,
                    "Retrieval module checksum mismatch",
                ):
                    registration.verify_retrieval_module_integrity(target)

                self.assertEqual(mocked_fstat.call_count, 2)

    def test_precreation_cli_rejects_untrusted_module(self):
        with patch.object(registration.os, "geteuid", return_value=0):
            with patch.object(
                registration.sys, "argv",
                ["register-container", "--verify-retrieval"]
            ):
                with patch.object(
                    registration, "verify_retrieval_module_integrity",
                    side_effect=PermissionError("rejected")
                ) as check:
                    with self.assertRaisesRegex(
                        PermissionError, "rejected"
                    ):
                        registration.main()
                    check.assert_called_once_with()

    def test_precreation_cli_accepts_verified_module(self):
        with patch.object(registration.os, "geteuid", return_value=0):
            with patch.object(
                registration.sys, "argv",
                ["register-container", "--verify-retrieval"]
            ):
                with patch.object(
                    registration, "verify_retrieval_module_integrity",
                    return_value=True
                ) as check:
                    self.assertEqual(registration.main(), 0)
                    check.assert_called_once_with()

    def test_precreation_cli_requires_root(self):
        with patch.object(registration.os, "geteuid", return_value=1000):
            with patch.object(
                registration.sys, "argv",
                ["register-container", "--verify-retrieval"]
            ):
                with patch.object(
                    registration, "verify_retrieval_module_integrity"
                ) as check:
                    with self.assertRaises(PermissionError):
                        registration.main()
                    check.assert_not_called()

    def test_middleware_cli_rejects_untrusted_module(self):
        with patch.object(registration.os, "geteuid", return_value=0):
            with patch.object(
                registration.sys, "argv",
                ["register-container", "--verify-middleware"]
            ):
                with patch.object(
                    registration, "verify_middleware_module_integrity",
                    side_effect=PermissionError("middleware rejected")
                ) as check:
                    with self.assertRaisesRegex(
                        PermissionError, "middleware rejected"
                    ):
                        registration.main()
                    check.assert_called_once_with()

    def test_middleware_cli_accepts_verified_module(self):
        with patch.object(registration.os, "geteuid", return_value=0):
            with patch.object(
                registration.sys, "argv",
                ["register-container", "--verify-middleware"]
            ):
                with patch.object(
                    registration, "verify_middleware_module_integrity",
                    return_value=True
                ) as check:
                    self.assertEqual(registration.main(), 0)
                    check.assert_called_once_with()

    def test_middleware_gate_precedes_container_creation(self):
        source = (
            ROOT / "runtime/modules/openwebui.sh"
        ).read_text()
        start = source.index("openwebui_install() {")
        end = source.index("\nopenwebui_verify()", start)
        block = source[start:end]
        self.assertEqual(block.count("--verify-middleware"), 1)
        self.assertLess(
            block.index("--verify-middleware"),
            block.index("docker container create")
        )

    def test_shell_gate_precedes_container_creation(self):
        source = (
            ROOT / "runtime/modules/openwebui.sh"
        ).read_text()
        start = source.index("openwebui_install() {")
        end = source.index("\nopenwebui_verify()", start)
        block = source[start:end]
        self.assertEqual(block.count("--verify-retrieval"), 1)
        self.assertLess(
            block.index("--verify-retrieval"),
            block.index("docker container create")
        )

    def test_mount_inspection_requires_integrity(self):
        with patch.object(
            registration, "verify_retrieval_module_integrity",
            side_effect=PermissionError("integrity rejected")
        ) as integrity_check:
            with self.assertRaisesRegex(
                PermissionError, "integrity rejected"
            ):
                registration.inspect_container_mounts("a" * 64)
            integrity_check.assert_called_once_with()


if __name__ == "__main__":
    unittest.main()
