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
    "daia_openwebui_secret_entrypoint", SOURCE
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class SecretPersistenceTests(unittest.TestCase):
    def run_entrypoint(self, initial):
        with patch.dict(os.environ, initial, clear=True):
            with patch.object(
                module, "read_password",
                return_value="test-only-password"
            ):
                with patch.object(module.os, "chdir"):
                    with patch.object(module.os, "execvp"):
                        with patch.object(module, "ensure_webui_secret_file"):
                            result = module.main()
                        environment = dict(os.environ)
        return result, environment

    def test_default_secret_file_is_persistent(self):
        result, env = self.run_entrypoint({})
        self.assertIsNone(result)
        self.assertEqual(
            env["WEBUI_SECRET_KEY_FILE"],
            "/app/backend/data/.webui_secret_key",
        )

    def test_explicit_secret_file_override_preserved(self):
        custom = "/custom/managed-secret"
        _, env = self.run_entrypoint({
            "WEBUI_SECRET_KEY_FILE": custom
        })
        self.assertEqual(env["WEBUI_SECRET_KEY_FILE"], custom)

    def test_explicit_secret_environment_preserved(self):
        _, env = self.run_entrypoint({
            "WEBUI_SECRET_KEY": "test-existing-secret"
        })
        self.assertEqual(
            env["WEBUI_SECRET_KEY"], "test-existing-secret"
        )

    def test_database_configuration_preserved(self):
        custom = "sqlite:////app/backend/data/custom.db"
        _, env = self.run_entrypoint({
            "DATABASE_URL": custom
        })
        self.assertEqual(env["DATABASE_URL"], custom)

    def test_repeated_launch_uses_same_secret_path(self):
        _, first = self.run_entrypoint({})
        _, second = self.run_entrypoint({})
        self.assertEqual(
            first["WEBUI_SECRET_KEY_FILE"],
            second["WEBUI_SECRET_KEY_FILE"],
        )


if __name__ == "__main__":
    unittest.main()
