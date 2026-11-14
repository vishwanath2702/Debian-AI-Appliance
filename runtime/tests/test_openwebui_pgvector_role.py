"""PostgreSQL application-role SQL contract tests."""
import importlib.util
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "runtime/daia-openwebui-pgvector-role.py"
spec = importlib.util.spec_from_file_location("openwebui_role", HELPER)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class OpenWebUIRoleTests(unittest.TestCase):
    def test_dedicated_identity(self):
        self.assertEqual(module.ROLE, "daia_openwebui")
        self.assertEqual(module.DATABASE, "daia_rag")
        self.assertEqual(module.SCHEMA, "daia_openwebui")

    def test_transactional_contract(self):
        statements = module.validate_role_contract()
        self.assertEqual(statements[3], "BEGIN;")
        self.assertEqual(statements[-1], "COMMIT;")

    def test_refuses_existing_role(self):
        self.assertTrue(any(
            "FROM pg_roles" in statement
            and "RAISE EXCEPTION" in statement
            for statement in module.role_sql()
        ))

    def test_password_is_safely_quoted(self):
        self.assertTrue(any(
            "format(" in statement
            and "%L" in statement
            and ":'app_password'" in statement
            and r"\gexec" in statement
            for statement in module.role_sql()
        ))

    def test_no_shared_schema_create_grant(self):
        self.assertFalse(any(
            "GRANT CREATE ON SCHEMA public" in statement
            for statement in module.role_sql()
        ))

    def test_psql_output_controls(self):
        statements = module.role_sql()
        self.assertIn(r"\set ON_ERROR_STOP on", statements)
        self.assertIn(r"\set ECHO none", statements)
        self.assertIn(r"\set QUIET on", statements)

    def test_no_division_by_zero_guard(self):
        self.assertFalse(any(
            "1/0" in statement for statement in module.role_sql()
        ))

    def test_exact_contract_enforced(self):
        with patch.object(
            module, "role_sql", return_value=("COMMIT;",)
        ):
            with self.assertRaises(ValueError):
                module.validate_role_contract()

    def test_helper_has_no_execution_entrypoint(self):
        self.assertFalse(hasattr(module, "provision"))
        self.assertFalse(hasattr(module, "execute"))
        self.assertFalse(hasattr(module, "connect"))


if __name__ == "__main__":
    unittest.main()
