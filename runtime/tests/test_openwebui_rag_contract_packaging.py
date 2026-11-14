"""Pin exact Open WebUI role SQL and verify ISO packaging."""

import importlib.util
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock

ROOT = Path(__file__).resolve().parents[2]


def load(name, relative):
    path = ROOT / relative
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


provision = load(
    "rag_pinned_provision",
    "runtime/daia-openwebui-rag-provision.py",
)
role = load(
    "rag_pinned_role",
    "runtime/daia-openwebui-pgvector-role.py",
)

IDENTITY = "c" * 64
PASSWORD = b"Secure_application_password_1234567890"


class ContractAndPackagingTests(unittest.TestCase):
    def test_approved_contract_executes_through_stdin(self):
        calls = []

        def runner(argv, **kwargs):
            calls.append((argv, kwargs))
            return SimpleNamespace(returncode=0)

        self.assertTrue(provision.execute_role_sql(
            IDENTITY, PASSWORD,
            role.validate_role_contract(),
            runner=runner,
        ))
        self.assertEqual(len(calls), 1)
        self.assertNotIn(
            PASSWORD.decode(), " ".join(calls[0][0])
        )

    def test_modified_middle_statement_is_rejected(self):
        original = list(role.validate_role_contract())
        self.assertGreater(len(original), 7)
        original[5] = (
            "GRANT ALL PRIVILEGES ON DATABASE daia_rag "
            "TO daia_openwebui;"
        )

        runner = Mock()
        with self.assertRaises(provision.ProvisioningError):
            provision.execute_role_sql(
                IDENTITY, PASSWORD, tuple(original),
                runner=runner,
            )
        runner.assert_not_called()

    def test_approved_contract_digest_is_stable(self):
        import hashlib

        contract = "\n".join(role.validate_role_contract())
        self.assertEqual(
            hashlib.sha256(
                contract.encode("utf-8")
            ).hexdigest(),
            provision.ROLE_SQL_SHA256,
        )

    def test_packaging_stages_required_helpers(self):
        build = (
            ROOT / "build/build-payload.sh"
        ).read_text(encoding="utf-8")

        for helper in (
            "daia-openwebui-rag-provision.py",
            "daia-openwebui-pgvector-role.py",
            "daia-openwebui-pgvector-credentials.py",
        ):
            with self.subTest(helper=helper):
                self.assertIn(
                    f'"$DAIA_RUNTIME_SOURCE/{helper}"',
                    build,
                )
                self.assertIn(
                    f'"$DAIA_WORKSPACE/usr/libexec/{helper}"',
                    build,
                )

    def test_provisioning_requires_post_execution_verification(self):
        source = (
            ROOT / "runtime/daia-openwebui-rag-provision.py"
        ).read_text(encoding="utf-8")
        self.assertIn("verify_role_state(container_id)", source)
        self.assertIn("execute_role_sql(", source)
        self.assertNotIn(
            "PostgreSQL provisioning execution is not enabled",
            source,
        )


if __name__ == "__main__":
    unittest.main()
