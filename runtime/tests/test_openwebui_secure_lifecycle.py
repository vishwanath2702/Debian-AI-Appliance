"""Secure Open WebUI container lifecycle source contracts."""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/openwebui.sh"


class OpenWebUISecureLifecycleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = MODULE.read_text()
        cls.install = cls.source.split(
            "openwebui_install() {", 1
        )[1].split("\nopenwebui_verify() {", 1)[0]

    def test_create_without_immediate_start(self):
        self.assertIn("docker container create", self.install)
        self.assertNotIn("docker run", self.install)

    def test_pinned_immutable_image(self):
        self.assertIn(
            "sha256:e29d4d2457b54bb14434bee2666420e822cb0e9389466f02405d69fa7fd8d06c",
            self.install,
        )

    def test_daia_ownership_labels(self):
        self.assertIn("--label org.daia.component=openwebui", self.install)
        self.assertIn("--label org.daia.managed=true", self.install)

    def test_no_new_privileges(self):
        self.assertIn(
            "--security-opt no-new-privileges:true", self.install
        )

    def test_private_network(self):
        self.assertIn("--network daia-rag-internal", self.install)

    def test_registration_after_creation(self):
        creation = self.install.index("docker container create")
        registration = self.install.index(
            "/usr/libexec/daia-openwebui-register-container",
            creation,
        )
        self.assertLess(creation, registration)

    def test_no_start_before_registration(self):
        registration = self.install.index(
            "/usr/libexec/daia-openwebui-register-container"
        )
        for operation in ("docker start", "docker container start"):
            if operation in self.install:
                self.assertGreater(
                    self.install.index(operation), registration
                )


if __name__ == "__main__":
    unittest.main()
