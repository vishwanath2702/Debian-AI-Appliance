import importlib.util
import os
import unittest
from pathlib import Path
from unittest.mock import patch

SOURCE = (
    Path(__file__).resolve().parents[1]
    / "daia-openwebui-rag-entrypoint.py"
)

spec = importlib.util.spec_from_file_location(
    "daia_openwebui_rag_entrypoint_test", SOURCE
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class OpenWebUIEntrypointConfigTests(unittest.TestCase):
    def run_entrypoint(self, initial_env, password="test-only-password"):
        with patch.dict(os.environ, initial_env, clear=True):
            with patch.object(module, "read_password", return_value=password):
                with patch.object(module.os, "chdir") as chdir:
                    with patch.object(module.os, "execvp") as execvp:
                        with patch.object(module, "ensure_webui_secret_file"):
                            result = module.main()
                        environment = dict(os.environ)
        return result, environment, chdir, execvp

    def test_default_sqlite_database(self):
        result, env, chdir, execvp = self.run_entrypoint({})
        self.assertEqual(result, None)
        self.assertEqual(
            env["DATABASE_URL"],
            "sqlite:////app/backend/data/webui.db",
        )
        chdir.assert_called_once_with("/app/backend")
        execvp.assert_called_once_with(
            "bash", ["bash", "start.sh"]
        )

    def test_existing_database_url_preserved(self):
        original = "sqlite:////app/backend/data/custom.db"
        _, env, _, _ = self.run_entrypoint(
            {"DATABASE_URL": original}
        )
        self.assertEqual(env["DATABASE_URL"], original)

    def test_unrelated_user_environment_preserved(self):
        _, env, _, _ = self.run_entrypoint({
            "WEBUI_NAME": "My Pragna",
            "ENABLE_PERSISTENT_CONFIG": "True",
            "CUSTOM_USER_SETTING": "preserve-me",
        })
        self.assertEqual(env["WEBUI_NAME"], "My Pragna")
        self.assertEqual(
            env["ENABLE_PERSISTENT_CONFIG"], "True"
        )
        self.assertEqual(
            env["CUSTOM_USER_SETTING"], "preserve-me"
        )

    def test_pgvector_enforced(self):
        _, env, _, _ = self.run_entrypoint({
            "VECTOR_DB": "other",
            "PGVECTOR_DB_URL": "old-value",
        })
        self.assertEqual(env["VECTOR_DB"], "pgvector")
        self.assertTrue(
            env["PGVECTOR_DB_URL"].startswith(
                "postgresql://daia_openwebui:"
            )
        )
        self.assertIn("@daia-pgvector:5432/daia_rag",
                      env["PGVECTOR_DB_URL"])

    def test_password_url_encoding(self):
        _, env, _, _ = self.run_entrypoint(
            {}, password="test:@/?#value"
        )
        self.assertIn(
            "test%3A%40%2F%3F%23value",
            env["PGVECTOR_DB_URL"],
        )

    def test_missing_credential_fails_closed(self):
        with patch.dict(os.environ, {}, clear=True):
            with patch.object(
                module, "read_password",
                side_effect=FileNotFoundError()
            ):
                with patch.object(module.os, "chdir") as chdir:
                    with patch.object(module.os, "execvp") as execvp:
                        result = module.main()
        self.assertEqual(result, 1)
        chdir.assert_not_called()
        execvp.assert_not_called()

    def test_invalid_credential_fails_closed(self):
        with patch.dict(os.environ, {}, clear=True):
            with patch.object(
                module, "read_password",
                side_effect=ValueError("invalid credential")
            ):
                with patch.object(module.os, "chdir") as chdir:
                    with patch.object(module.os, "execvp") as execvp:
                        result = module.main()
        self.assertEqual(result, 1)
        chdir.assert_not_called()
        execvp.assert_not_called()

if __name__ == "__main__":
    unittest.main()
