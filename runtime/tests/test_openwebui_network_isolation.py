"""Required Open WebUI network isolation contracts."""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WEBUI = ROOT / "runtime/modules/openwebui.sh"
OLLAMA = ROOT / "runtime/services/ollama.service"
PGVECTOR = ROOT / "runtime/modules/pgvector.sh"


class OpenWebUINetworkIsolationTests(unittest.TestCase):
    def test_ollama_remains_loopback_only(self):
        service = OLLAMA.read_text()
        self.assertIn(
            "Environment=OLLAMA_HOST=127.0.0.1:11434",
            service,
        )
        self.assertNotIn(
            "Environment=OLLAMA_HOST=0.0.0.0:11434",
            service,
        )

    def test_postgresql_remains_on_private_network(self):
        source = PGVECTOR.read_text()
        self.assertIn("--network daia-rag-internal", source)
        self.assertNotIn("--network host", source)

    def test_openwebui_must_not_use_host_network(self):
        source = WEBUI.read_text()
        self.assertNotIn("--network host", source)

    def test_openwebui_must_not_mount_bootstrap_secret(self):
        source = WEBUI.read_text()
        self.assertNotIn(
            "src=/var/lib/daia/pgvector/postgres-password,",
            source,
        )

    def test_openwebui_must_not_publish_postgresql(self):
        source = WEBUI.read_text()
        self.assertNotIn("--publish 5432:", source)
        self.assertNotIn("-p 5432:", source)


if __name__ == "__main__":
    unittest.main()
