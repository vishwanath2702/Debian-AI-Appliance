"""DAIA Open WebUI RAG identity and isolation contracts."""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WRAPPER = ROOT / "runtime/daia-openwebui-rag-entrypoint.py"
MODULE = ROOT / "runtime/modules/openwebui.sh"


class OpenWebUIIdentityContractTests(unittest.TestCase):

    def test_dedicated_database_identity(self):
        source = WRAPPER.read_text()
        self.assertIn('PGVECTOR_USER = "daia_openwebui"', source)
        self.assertIn(
            'daia-openwebui-pgvector-password',
            source,
        )
        self.assertNotIn(
            'SECRET_PATH = Path("/run/secrets/daia-pgvector-password")',
            source,
        )

    def test_bootstrap_password_not_mounted(self):
        source = MODULE.read_text()
        self.assertIn(
            "src=/var/lib/daia/pgvector/openwebui-password,"
            "dst=/run/secrets/daia-openwebui-pgvector-password,readonly",
            source,
        )
        self.assertNotIn(
            "src=/var/lib/daia/pgvector/postgres-password,"
            "dst=/run/secrets/daia-pgvector-password",
            source,
        )

    def test_installation_remains_fail_closed(self):
        source = MODULE.read_text()
        self.assertIn(
            "/usr/libexec/daia-openwebui-rag-authorize",
            source,
        )

    def test_no_database_secret_in_docker_environment(self):
        source = MODULE.read_text()
        self.assertNotIn("--env PGVECTOR_DB_URL=", source)
        self.assertNotIn("--env DATABASE_URL=", source)


if __name__ == "__main__":
    unittest.main()
