"""Open WebUI secure RAG launch contract."""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/openwebui.sh"


class OpenWebUIRAGLaunchTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = MODULE.read_text()

    def test_secret_is_mounted_read_only(self):
        self.assertIn(
            "src=/var/lib/daia/pgvector/openwebui-password,"
            "dst=/run/secrets/daia-openwebui-pgvector-password,readonly",
            self.source,
        )

    def test_wrapper_is_mounted_read_only(self):
        self.assertIn(
            "src=/usr/libexec/daia-openwebui-rag-entrypoint.py,"
            "dst=/usr/local/bin/daia-openwebui-rag-entrypoint.py,readonly",
            self.source,
        )

    def test_wrapper_replaces_original_command(self):
        self.assertIn(
            '"$expected_image" \\\n'
            '        python3 /usr/local/bin/daia-openwebui-rag-entrypoint.py',
            self.source,
        )

    def test_no_database_url_in_docker_environment(self):
        self.assertNotIn("--env PGVECTOR_DB_URL=", self.source)
        self.assertNotIn("--env DATABASE_URL=", self.source)


if __name__ == "__main__":
    unittest.main()
