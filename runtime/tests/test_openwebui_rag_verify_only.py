"""Read-only RAG role verification and immutable identity binding."""

import importlib.util
import inspect
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

SOURCE = (
    Path(__file__).resolve().parents[1]
    / "daia-openwebui-rag-provision.py"
)

spec = importlib.util.spec_from_file_location(
    "pragna_rag_verify_only", SOURCE
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

IDENTITY = "a" * 64
OTHER_IDENTITY = "b" * 64
PASSWORD = b"Secure_Pr agna_password".replace(b" ", b"") + b"_1234567890"


class VerifyOnlyTests(unittest.TestCase):
    def test_verified_role_requires_all_checks_in_order(self):
        calls = Mock()

        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(
                module, "authorize_container",
                side_effect=lambda: (
                    calls.authorize(), IDENTITY
                )[1],
            ),
            patch.object(
                module, "validate_application_credential",
                side_effect=lambda: (
                    calls.credential(), PASSWORD
                )[1],
            ),
            patch.object(
                module, "verify_postgres_ready",
                side_effect=lambda identity: calls.ready(identity),
            ),
            patch.object(
                module, "read_role_verifier",
                side_effect=lambda identity: (
                    calls.lookup(identity), "SCRAM-VERIFIER"
                )[1],
            ),
            patch.object(
                module, "verify_role_password",
                side_effect=lambda *args: calls.password(*args),
            ),
            patch.object(
                module, "verify_role_state",
                side_effect=lambda identity: calls.state(identity),
            ),
            patch.object(module, "execute_role_sql") as execute,
        ):
            self.assertTrue(
                module.verify_existing_role(IDENTITY)
            )
            execute.assert_not_called()

        self.assertEqual(
            [call[0] for call in calls.mock_calls],
            [
                "authorize", "credential", "ready",
                "lookup", "password", "state",
            ],
        )

    def test_nonroot_fails_before_authorization(self):
        with (
            patch.object(module.os, "geteuid", return_value=1000),
            patch.object(module, "authorize_container") as authorize,
        ):
            with self.assertRaises(module.ProvisioningError):
                module.verify_existing_role(IDENTITY)
            authorize.assert_not_called()

    def test_changed_identity_blocks_secret_access(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(
                module, "authorize_container",
                return_value=OTHER_IDENTITY,
            ),
            patch.object(
                module, "validate_application_credential"
            ) as credential,
        ):
            with self.assertRaisesRegex(
                module.ProvisioningError, "identity changed"
            ):
                module.verify_existing_role(IDENTITY)
            credential.assert_not_called()

    def test_missing_role_blocks_authorization(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(
                module, "authorize_container",
                return_value=IDENTITY,
            ),
            patch.object(
                module, "validate_application_credential",
                return_value=PASSWORD,
            ),
            patch.object(
                module, "verify_postgres_ready",
                return_value=True,
            ),
            patch.object(
                module, "read_role_verifier",
                return_value=None,
            ),
            patch.object(module, "verify_role_state") as state,
            patch.object(module, "execute_role_sql") as execute,
        ):
            with self.assertRaisesRegex(
                module.ProvisioningError, "role is missing"
            ):
                module.verify_existing_role(IDENTITY)
            state.assert_not_called()
            execute.assert_not_called()

    def test_password_mismatch_blocks_privilege_check(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(
                module, "authorize_container",
                return_value=IDENTITY,
            ),
            patch.object(
                module, "validate_application_credential",
                return_value=PASSWORD,
            ),
            patch.object(
                module, "verify_postgres_ready",
                return_value=True,
            ),
            patch.object(
                module, "read_role_verifier",
                return_value="SCRAM-VERIFIER",
            ),
            patch.object(
                module, "verify_role_password",
                side_effect=module.ProvisioningError("mismatch"),
            ),
            patch.object(module, "verify_role_state") as state,
        ):
            with self.assertRaises(module.ProvisioningError):
                module.verify_existing_role(IDENTITY)
            state.assert_not_called()

    def test_privilege_mismatch_blocks_authorization(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(
                module, "authorize_container",
                return_value=IDENTITY,
            ),
            patch.object(
                module, "validate_application_credential",
                return_value=PASSWORD,
            ),
            patch.object(
                module, "verify_postgres_ready",
                return_value=True,
            ),
            patch.object(
                module, "read_role_verifier",
                return_value="SCRAM-VERIFIER",
            ),
            patch.object(
                module, "verify_role_password",
                return_value=True,
            ),
            patch.object(
                module, "verify_role_state",
                side_effect=module.ProvisioningError("privileges"),
            ),
        ):
            with self.assertRaises(module.ProvisioningError):
                module.verify_existing_role(IDENTITY)

    def test_cli_verify_mode_uses_exact_identity(self):
        with (
            patch.object(
                module, "verify_existing_role",
                return_value=True,
            ) as verify,
            patch.object(module, "provision") as provision,
        ):
            self.assertEqual(
                module.main(["--verify-only", IDENTITY]), 0
            )
            verify.assert_called_once_with(IDENTITY)
            provision.assert_not_called()

    def test_cli_rejects_unexpected_arguments(self):
        with (
            patch.object(module, "verify_existing_role") as verify,
            patch.object(module, "provision") as provision,
        ):
            self.assertEqual(
                module.main(["--verify-only", IDENTITY, "extra"]),
                1,
            )
            verify.assert_not_called()
            provision.assert_not_called()

    def test_default_cli_preserves_provisioning(self):
        with (
            patch.object(module, "provision") as provision,
            patch.object(module, "verify_existing_role") as verify,
        ):
            self.assertEqual(module.main([]), 0)
            provision.assert_called_once_with()
            verify.assert_not_called()

    def test_verification_function_has_no_mutation_path(self):
        source = inspect.getsource(
            module.verify_existing_role
        )
        self.assertNotIn("execute_role_sql(", source)
        self.assertNotIn("provision(", source)
        self.assertNotIn("docker start", source)
        self.assertNotIn("docker create", source)

    def test_authorizer_requires_verify_only_helper(self):
        authorizer = (
            SOURCE.parent / "daia-openwebui-rag-authorize"
        ).read_text(encoding="utf-8")
        self.assertIn(
            '"--verify-only"',
            authorizer,
        )


if __name__ == "__main__":
    unittest.main()
