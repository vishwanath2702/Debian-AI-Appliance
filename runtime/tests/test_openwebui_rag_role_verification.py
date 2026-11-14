"""Read-only PostgreSQL application role verification tests."""

import importlib.util
import subprocess
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "runtime/daia-openwebui-rag-provision.py"

spec = importlib.util.spec_from_file_location(
    "rag_role_verification", SOURCE
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

IDENTITY = "b" * 64


class RoleVerificationTests(unittest.TestCase):
    def test_verified_role_uses_immutable_container(self):
        calls = []

        def runner(argv, **kwargs):
            calls.append((argv, kwargs))
            return SimpleNamespace(returncode=0, stdout="ok\n")

        self.assertTrue(
            module.verify_role_state(IDENTITY, runner=runner)
        )
        self.assertEqual(len(calls), 1)

        argv, options = calls[0]
        self.assertEqual(argv[:3], [
            "docker", "exec", IDENTITY
        ])
        self.assertIn(
            "require_auth=scram-sha-256",
            argv,
        ) if False else self.assertTrue(
            any(
                "require_auth=scram-sha-256" in value
                for value in argv
            )
        )
        self.assertNotIn("env", options)
        self.assertNotIn("shell", options)
        self.assertEqual(options["timeout"], 30)

    def test_role_requires_restricted_privileges(self):
        query = module.ROLE_VERIFY_SQL
        for condition in (
            "rolcanlogin",
            "NOT rolsuper",
            "NOT rolcreatedb",
            "NOT rolcreaterole",
            "NOT rolreplication",
            "NOT rolbypassrls",
        ):
            with self.subTest(condition=condition):
                self.assertIn(condition, query)

    def test_private_schema_and_search_path_required(self):
        query = module.ROLE_VERIFY_SQL
        self.assertIn("pg_namespace", query)
        self.assertIn("n.nspowner", query)
        self.assertIn(
            "search_path=daia_openwebui, public",
            query,
        )
        self.assertIn("has_database_privilege", query)
        self.assertIn("NOT has_schema_privilege", query)

    def test_invalid_identity_prevents_docker(self):
        runner = Mock()
        with self.assertRaises(module.ProvisioningError):
            module.verify_role_state(
                "daia-pgvector", runner=runner
            )
        runner.assert_not_called()

    def test_negative_verification_fails_closed(self):
        for output in ("fail\n", "", "ok\nextra\n", "OK\n"):
            with self.subTest(output=repr(output)):
                with self.assertRaises(module.ProvisioningError):
                    module.verify_role_state(
                        IDENTITY,
                        runner=lambda *a, **k: SimpleNamespace(
                            returncode=0, stdout=output
                        ),
                    )

    def test_psql_error_fails_closed(self):
        with self.assertRaises(module.ProvisioningError):
            module.verify_role_state(
                IDENTITY,
                runner=lambda *a, **k: SimpleNamespace(
                    returncode=1,
                    stdout="ok\n",
                    stderr="database error",
                ),
            )

    def test_timeout_fails_closed(self):
        def runner(argv, **kwargs):
            raise subprocess.TimeoutExpired(argv, 30)

        with self.assertRaises(module.ProvisioningError):
            module.verify_role_state(
                IDENTITY, runner=runner
            )

    def test_os_error_fails_closed(self):
        def runner(argv, **kwargs):
            raise OSError("Docker unavailable")

        with self.assertRaises(module.ProvisioningError):
            module.verify_role_state(
                IDENTITY, runner=runner
            )


if __name__ == "__main__":
    unittest.main()
