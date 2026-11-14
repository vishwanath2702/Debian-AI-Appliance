"""Secure Open WebUI role provisioning flow contracts."""

import importlib.util
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "runtime/daia-openwebui-rag-provision.py"
spec = importlib.util.spec_from_file_location("rag_flow", SOURCE)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ProvisioningFlowTests(unittest.TestCase):
    def test_rejects_nonroot_before_any_dependency(self):
        with (
            patch.object(module.os, "geteuid", return_value=1000),
            patch.object(module, "authorize_container") as authorize,
        ):
            with self.assertRaises(module.ProvisioningError):
                module.provision()
            authorize.assert_not_called()

    def test_authorizer_uses_runtime_identity(self):
        class Result:
            returncode = 0
            stdout = "a" * 64 + "\n"

        calls = []
        def runner(argv, **kwargs):
            calls.append(argv)
            return Result()

        self.assertEqual(
            module.authorize_container(runner=runner), "a" * 64
        )
        self.assertEqual(
            calls, [[module.AUTHORIZER, "runtime"]]
        )

    def test_no_dynamic_credential_import(self):
        import inspect
        source = inspect.getsource(
            module.validate_application_credential
        )
        self.assertNotIn("importlib", source)
        self.assertNotIn("sys.path", source)
        self.assertNotIn("spec_from_file_location", source)

    def test_directory_path_is_fixed(self):
        with patch.object(
            module, "STATE_DIRECTORY", "/tmp/untrusted"
        ):
            with self.assertRaises(module.ProvisioningError):
                module.open_state_directory()

    def test_unavailable_state_directory_fails_closed(self):
        with patch.object(
            module.os, "open",
            side_effect=OSError("missing state"),
        ):
            with self.assertRaises(OSError):
                module.validate_application_credential()

    def test_sql_execution_failure_blocks_verification(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module, "authorize_container",
                         return_value="a" * 64),
            patch.object(module, "validate_application_credential",
                         return_value=b"A" * 48),
            patch.object(module, "verify_postgres_ready",
                         return_value=True),
            patch.object(
                module, "read_role_verifier",
                return_value=None,
            ),
            patch.object(
                module, "execute_role_sql",
                side_effect=module.ProvisioningError("SQL failed")
            ),
            patch.object(module, "verify_role_state") as verify,
        ):
            with self.assertRaises(module.ProvisioningError):
                module.provision()
            verify.assert_not_called()

    def test_approved_role_sql_matches_pinned_digest(self):
        import hashlib
        contract = "\n".join(module.ROLE_SQL_STATEMENTS)
        self.assertEqual(
            hashlib.sha256(contract.encode()).hexdigest(),
            module.ROLE_SQL_SHA256,
        )

    def test_authorizer_requires_read_only_proof(self):
        authorizer = (
            ROOT / "runtime/daia-openwebui-rag-authorize"
        ).read_text(encoding="utf-8")
        self.assertIn(
            '"--verify-only"',
            authorizer,
        )


if __name__ == "__main__":
    unittest.main()
