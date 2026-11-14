"""Security contracts for privileged Open WebUI registration."""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "runtime/daia-openwebui-register-container"


class OpenWebUIRegistrationSecurityContractTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        if not HELPER.is_file() or HELPER.is_symlink():
            raise AssertionError(
                "Privileged Open WebUI registration helper is missing"
            )
        cls.source = HELPER.read_text()

    def test_requires_root(self):
        self.assertIn("geteuid", self.source)

    def test_independently_inspects_docker(self):
        self.assertIn('"container", "inspect",', self.source)
        self.assertIn("docker", self.source)

    def test_pins_immutable_image(self):
        self.assertIn(
            "sha256:e29d4d2457b54bb14434bee2666420e822cb0e9389466f02405d69fa7fd8d06c",
            self.source,
        )

    def test_verifies_ownership_and_security(self):
        for required in (
            "org.daia.component",
            "org.daia.managed",
            "no-new-privileges:true",
            "daia-rag-internal",
        ):
            with self.subTest(required=required):
                self.assertIn(required, self.source)

    def test_verifies_all_three_mounts(self):
        for required in (
            "/app/backend/data",
            "/run/secrets/daia-openwebui-pgvector-password",
            "/usr/local/bin/daia-openwebui-rag-entrypoint.py",
        ):
            with self.subTest(required=required):
                self.assertIn(required, self.source)

    def test_independently_verifies_volume_ownership(self):
        self.assertIn('"volume", "inspect",', self.source)
        self.assertIn("openwebui", self.source)

    def test_rechecks_container_identity(self):
        self.assertIn("container_id", self.source)
        self.assertIn("verified_id", self.source)

    def test_provisions_private_registration_directory(self):
        self.assertIn(
            "provision_registration_directory",
            self.source,
        )

    def test_publishes_through_authorized_registration(self):
        self.assertIn(
            "register_authorized_container",
            self.source,
        )

    def test_does_not_mutate_docker(self):
        for forbidden in (
            "docker container start",
            "docker container create",
            "docker container rm",
            "docker volume create",
            "docker volume rm",
        ):
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, self.source)


if __name__ == "__main__":
    unittest.main()
