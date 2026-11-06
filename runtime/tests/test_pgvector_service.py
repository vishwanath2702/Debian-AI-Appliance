"""Static security contract for the DAIA pgvector service."""

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SERVICE = ROOT / "runtime/services/daia-pgvector.service"


class PgvectorServiceContractTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        if not SERVICE.is_file() or SERVICE.is_symlink():
            raise AssertionError(
                "Pgvector service definition is missing or unsafe"
            )

        cls.source = SERVICE.read_text()

    def test_requires_docker_service(self):
        self.assertIn("Requires=docker.service", self.source)
        self.assertIn("After=docker.service", self.source)

    def test_uses_dedicated_startup_command(self):
        self.assertIn("ExecStart=", self.source)
        self.assertIn("daia-pgvector", self.source)

    def test_does_not_embed_database_password(self):
        self.assertNotIn("POSTGRES_PASSWORD=", self.source)
        self.assertNotIn("PGPASSWORD=", self.source)

    def test_does_not_remove_database_container(self):
        self.assertNotIn("docker rm", self.source)
        self.assertNotIn("docker volume rm", self.source)
        self.assertNotIn("docker volume prune", self.source)

    def test_does_not_use_docker_restart_policy(self):
        self.assertNotIn("--restart always", self.source)
        self.assertNotIn("--restart unless-stopped", self.source)

    def test_service_is_not_prematurely_enabled(self):
        self.assertNotIn("WantedBy=multi-user.target", self.source)


if __name__ == "__main__":
    unittest.main()
