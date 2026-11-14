"""PostgreSQL role recovery and SCRAM credential binding."""

import base64
import hashlib
import hmac
import importlib.util
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch

SOURCE = (
    Path(__file__).resolve().parents[1]
    / "daia-openwebui-rag-provision.py"
)
spec = importlib.util.spec_from_file_location(
    "pragna_role_recovery", SOURCE
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

IDENTITY = "b" * 64
PASSWORD = b"Pragna_Recovery_Secret_0123456789_ABCDEF"


def scram_verifier(password=PASSWORD):
    salt = b"0123456789abcdef"
    salted = hashlib.pbkdf2_hmac(
        "sha256", password, salt, 4096
    )
    client = hmac.new(
        salted, b"Client Key", hashlib.sha256
    ).digest()
    stored = hashlib.sha256(client).digest()
    server = hmac.new(
        salted, b"Server Key", hashlib.sha256
    ).digest()

    encode = lambda data: base64.b64encode(data).decode("ascii")
    return (
        "SCRAM-SHA-256$4096:" + encode(salt) + "$"
        + encode(stored) + ":" + encode(server)
    )


class RoleRecoveryTests(unittest.TestCase):
    def test_valid_scram_password(self):
        self.assertTrue(
            module.verify_role_password(
                PASSWORD, scram_verifier()
            )
        )

    def test_wrong_password_fails_closed(self):
        with self.assertRaises(module.ProvisioningError):
            module.verify_role_password(
                b"Different_Recovery_Secret_0123456789_AB",
                scram_verifier(),
            )

    def test_rejects_malformed_verifier(self):
        for value in (
            "", "md5abc",
            "SCRAM-SHA-256$1:abc$abc:abc",
            "SCRAM-SHA-256$4096:abc$abc:abc",
        ):
            with self.subTest(value=value):
                with self.assertRaises(module.ProvisioningError):
                    module.verify_role_password(PASSWORD, value)

    def test_absent_role_is_distinguished(self):
        def runner(argv, **kwargs):
            return SimpleNamespace(returncode=0, stdout="")

        self.assertIsNone(
            module.read_role_verifier(IDENTITY, runner=runner)
        )

    def test_verifier_lookup_uses_immutable_identity(self):
        calls = []
        verifier = scram_verifier()

        def runner(argv, **kwargs):
            calls.append((argv, kwargs))
            return SimpleNamespace(
                returncode=0, stdout=verifier + "\n"
            )

        self.assertEqual(
            module.read_role_verifier(IDENTITY, runner=runner),
            verifier,
        )
        argv = calls[0][0]
        self.assertEqual(argv[:3], ["docker", "exec", IDENTITY])
        self.assertIn("pg_authid", " ".join(argv))
        self.assertNotIn(PASSWORD.decode(), " ".join(argv))

    def test_failed_lookup_never_claims_absence(self):
        runner = Mock(
            return_value=SimpleNamespace(
                returncode=1, stdout=""
            )
        )
        with self.assertRaises(module.ProvisioningError):
            module.read_role_verifier(IDENTITY, runner=runner)

    def test_existing_role_requires_password_and_state(self):
        calls = Mock()
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
                return_value=scram_verifier(),
            ),
            patch.object(
                module, "verify_role_password",
                side_effect=lambda *args: calls.password(*args),
            ),
            patch.object(
                module, "verify_role_state",
                side_effect=lambda *args: calls.state(*args),
            ),
            patch.object(module, "execute_role_sql") as execute,
        ):
            self.assertTrue(module.provision())
            execute.assert_not_called()

        self.assertEqual(
            [call[0] for call in calls.mock_calls],
            ["password", "state"],
        )

    def test_mismatched_role_never_mutated(self):
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
                return_value=scram_verifier(b"B" * 48),
            ),
            patch.object(module, "execute_role_sql") as execute,
            patch.object(module, "verify_role_state") as state,
        ):
            with self.assertRaises(module.ProvisioningError):
                module.provision()
            execute.assert_not_called()
            state.assert_not_called()

    def test_missing_role_uses_approved_creation_path(self):
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
                side_effect=[None, scram_verifier()],
            ) as lookup,
            patch.object(module, "execute_role_sql") as execute,
            patch.object(module, "verify_role_state") as state,
        ):
            self.assertTrue(module.provision())
            execute.assert_called_once_with(
                IDENTITY, PASSWORD, module.ROLE_SQL_STATEMENTS
            )
            state.assert_called_once_with(IDENTITY)
            self.assertEqual(lookup.call_count, 2)


if __name__ == "__main__":
    unittest.main()
