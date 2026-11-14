"""Credential-safe PostgreSQL role SQL transport tests."""

import importlib.util
import subprocess
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock

ROOT = Path(__file__).resolve().parents[2]

def load(name, relative):
    spec = importlib.util.spec_from_file_location(
        name, ROOT / relative
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

provision = load(
    "rag_sql_transport",
    "runtime/daia-openwebui-rag-provision.py",
)
role = load(
    "rag_role_contract",
    "runtime/daia-openwebui-pgvector-role.py",
)

IDENTITY = "a" * 64
PASSWORD = b"Secure_application_password_1234567890"
STATEMENTS = role.validate_role_contract()


class SQLTransportTests(unittest.TestCase):
    def test_password_only_in_standard_input(self):
        calls = []

        def runner(argv, **kwargs):
            calls.append((argv, kwargs))
            return SimpleNamespace(returncode=0)

        self.assertTrue(provision.execute_role_sql(
            IDENTITY, PASSWORD, STATEMENTS, runner=runner
        ))
        self.assertEqual(len(calls), 1)
        argv, kwargs = calls[0]

        self.assertEqual(argv[:4], [
            "docker", "exec", "-i", IDENTITY
        ])
        self.assertIn("require_auth=scram-sha-256", argv[-1])
        self.assertIn(
            "passfile=/run/secrets/daia-pgvector-pgpass",
            argv[-1],
        )
        self.assertNotIn(PASSWORD.decode(), " ".join(argv))
        self.assertIn(PASSWORD.decode(), kwargs["input"])
        self.assertIn("BEGIN;", kwargs["input"])
        self.assertIn("COMMIT;", kwargs["input"])
        self.assertTrue(kwargs["capture_output"])
        self.assertTrue(kwargs["text"])
        self.assertFalse(kwargs["check"])
        self.assertEqual(kwargs["timeout"], 60)
        self.assertNotIn("env", kwargs)
        self.assertNotIn("shell", kwargs)

    def test_invalid_identity_prevents_execution(self):
        runner = Mock()
        with self.assertRaises(provision.ProvisioningError):
            provision.execute_role_sql(
                "daia-pgvector", PASSWORD, STATEMENTS, runner
            )
        runner.assert_not_called()

    def test_invalid_password_prevents_execution(self):
        for value in (
            b"", b"short", b"invalid password with spaces" * 2,
            b"a" * 257, "not-bytes",
        ):
            runner = Mock()
            with self.subTest(value=repr(value)[:30]):
                with self.assertRaises(provision.ProvisioningError):
                    provision.execute_role_sql(
                        IDENTITY, value, STATEMENTS, runner
                    )
                runner.assert_not_called()

    def test_unvalidated_statement_shape_rejected(self):
        runner = Mock()
        for statements in (
            (),
            ("DROP DATABASE daia_rag;",),
            ["BEGIN;", "COMMIT;"],
        ):
            with self.assertRaises(provision.ProvisioningError):
                provision.execute_role_sql(
                    IDENTITY, PASSWORD, statements, runner
                )
        runner.assert_not_called()

    def test_database_error_does_not_expose_password(self):
        def runner(argv, **kwargs):
            return SimpleNamespace(
                returncode=1,
                stdout=PASSWORD.decode(),
                stderr=PASSWORD.decode(),
            )

        with self.assertRaises(
            provision.ProvisioningError
        ) as caught:
            provision.execute_role_sql(
                IDENTITY, PASSWORD, STATEMENTS, runner
            )

        self.assertNotIn(
            PASSWORD.decode(), str(caught.exception)
        )

    def test_timeout_fails_closed(self):
        def runner(argv, **kwargs):
            raise subprocess.TimeoutExpired(argv, 60)

        with self.assertRaises(provision.ProvisioningError):
            provision.execute_role_sql(
                IDENTITY, PASSWORD, STATEMENTS, runner
            )

    def test_os_error_fails_closed(self):
        def runner(argv, **kwargs):
            raise OSError("Docker unavailable")

        with self.assertRaises(provision.ProvisioningError):
            provision.execute_role_sql(
                IDENTITY, PASSWORD, STATEMENTS, runner
            )


if __name__ == "__main__":
    unittest.main()
