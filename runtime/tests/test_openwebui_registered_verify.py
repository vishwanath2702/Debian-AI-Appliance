"""Trusted, immutable-ID Open WebUI verification contracts."""
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/openwebui.sh"
HELPER = ROOT / "runtime/daia-openwebui-register-container"

class RegisteredVerificationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = MODULE.read_text().split(
            "openwebui_verify() {", 1
        )[1]

    def test_root_and_trusted_helper_required(self):
        self.assertIn('"$EUID" -ne 0', self.source)
        self.assertIn('-L "$helper"', self.source)
        self.assertIn('! -x "$helper"', self.source)

    def test_name_resolved_once_to_immutable_id(self):
        self.assertIn(
            '--format \'{{.Id}}\' "$container_name"',
            self.source,
        )
        self.assertIn('^ [0-9a-f]{64}$'.replace(" ", ""),
                      self.source)

    def test_registered_running_proof_required(self):
        self.assertIn(
            '--verify-running "$container_id"', self.source
        )
        self.assertIn(
            '"$verified_id" != "$container_id"', self.source
        )

    def test_ollama_configuration_uses_id(self):
        self.assertIn('OLLAMA_BASE_URL=', self.source)
        self.assertIn('"$container_id")', self.source)

    def test_no_mutating_docker_operation(self):
        for operation in (
            "docker start", "docker stop",
            "docker rm", "docker container create",
        ):
            self.assertNotIn(operation, self.source)

    def test_mount_verification_is_independent(self):
        helper = HELPER.read_text()
        self.assertIn(
            "inspect_container_mounts(container_id)", helper
        )
        self.assertNotIn("(index .Mounts 0)", self.source)

if __name__ == "__main__":
    unittest.main()
