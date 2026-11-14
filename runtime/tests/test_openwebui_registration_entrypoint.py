"""Source contracts for the privileged Open WebUI registration entry point."""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "runtime/daia-openwebui-register-container"


class OpenWebUIRegistrationEntrypointTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not HELPER.is_file() or HELPER.is_symlink():
            raise AssertionError("Trusted Open WebUI registration helper missing")
        cls.source = HELPER.read_text()

    def test_requires_root(self):
        self.assertIn("geteuid", self.source)

    def test_uses_pinned_image_identity(self):
        self.assertIn(
            "sha256:e29d4d2457b54bb14434bee2666420e822cb0e9389466f02405d69fa7fd8d06c",
            self.source,
        )

    def test_inspects_docker_independently(self):
        self.assertIn('"docker",', self.source)
        self.assertIn('"container", "inspect",', self.source)
        self.assertIn("subprocess.run(", self.source)

    def test_validates_ownership_and_security(self):
        self.assertIn("org.daia.component", self.source)
        self.assertIn("org.daia.managed", self.source)
        self.assertIn("no-new-privileges:true", self.source)
        self.assertIn("daia-rag-internal", self.source)

    def test_requires_mount_validation(self):
        self.assertIn("/app/backend/data", self.source)
        self.assertIn("/run/secrets/daia-openwebui-pgvector-password", self.source)
        self.assertIn("/usr/local/bin/daia-openwebui-rag-entrypoint.py", self.source)

    def test_uses_trusted_registration(self):
        self.assertIn("register_authorized_container", self.source)

    def test_rechecks_immutable_identity(self):
        self.assertIn("container_id", self.source)
        self.assertIn("verified_id", self.source)


if __name__ == "__main__":
    unittest.main()
