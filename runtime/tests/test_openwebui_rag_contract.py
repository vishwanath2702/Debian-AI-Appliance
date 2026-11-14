"""DAIA Open WebUI Slim RAG integration contract tests."""

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
OPENWEBUI = ROOT / "runtime/modules/openwebui.sh"
PGVECTOR = ROOT / "runtime/modules/pgvector.sh"
AUTHORIZER = ROOT / "runtime/daia-pgvector-authorize-start"


class OpenWebUIRAGContractTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.webui = OPENWEBUI.read_text()
        cls.pgvector = PGVECTOR.read_text()
        cls.authorizer = AUTHORIZER.read_text()

    def test_openwebui_declares_pgvector_backend(self):
        self.assertIn("VECTOR_DB=pgvector", self.webui)

    def test_openwebui_configures_remote_pgvector(self):
        self.assertIn("PGVECTOR_DB_URL", (ROOT / "runtime/daia-openwebui-rag-entrypoint.py").read_text())

    def test_openwebui_preserves_sqlite_application_database(self):
        self.assertNotIn("--env DATABASE_URL=", self.webui)

    def test_openwebui_does_not_delete_existing_container(self):
        self.assertNotIn('docker rm -f "$container_name"', self.webui)

    def test_pgvector_no_longer_uses_disconnected_network(self):
        self.assertNotIn("--network none", self.pgvector)

    def test_authorizer_checks_restricted_network(self):
        self.assertIn("HostConfig.NetworkMode", self.authorizer)
        self.assertNotIn('"$network_mode" != "none"', self.authorizer)


if __name__ == "__main__":
    unittest.main()
