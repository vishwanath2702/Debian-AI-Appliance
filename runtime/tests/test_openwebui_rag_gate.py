"""Open WebUI must refuse incomplete RAG authorization."""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/openwebui.sh"


class OpenWebUIRAGGateTests(unittest.TestCase):
    def test_authorization_precedes_docker_mutation(self):
        source = MODULE.read_text()
        gate = source.index(
            "/usr/libexec/daia-openwebui-rag-authorize"
        )
        image_load = source.index("docker load --input")
        container_create = source.index("docker container create")

        self.assertLess(gate, image_load)
        self.assertLess(gate, container_create)
        self.assertNotIn("docker run", source)

    def test_authorization_fails_closed(self):
        source = MODULE.read_text()
        self.assertIn(
            'if [[ ! -x /usr/libexec/daia-openwebui-rag-authorize ]]',
            source,
        )
        self.assertIn(
            'if ! /usr/libexec/daia-openwebui-rag-authorize "${tika_flags[@]}"; then',
            source,
        )

    def test_no_database_password_in_docker_environment(self):
        source = MODULE.read_text()
        self.assertNotIn("--env PGVECTOR_DB_URL=", source)
        self.assertNotIn("--env DATABASE_URL=", source)


if __name__ == "__main__":
    unittest.main()
