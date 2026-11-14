"""Security contract for DAIA Open WebUI registration."""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/openwebui_registration.py"


class OpenWebUIRegistrationContractTests(unittest.TestCase):
    def test_registration_module_exists(self):
        self.assertTrue(MODULE.is_file())

    def test_uses_existing_trusted_registration_primitives(self):
        source = MODULE.read_text()
        self.assertIn("pgvector_registration", source)
        self.assertIn("verify_trusted_registration", source)
        self.assertIn("publish_trusted_registration", source)

    def test_uses_dedicated_state_directory(self):
        source = MODULE.read_text()
        self.assertIn("/var/lib/daia/openwebui", source)

    def test_registration_is_not_shared_with_postgresql(self):
        source = MODULE.read_text()
        self.assertNotIn(
            'STATE_DIRECTORY = "/var/lib/daia/pgvector"',
            source,
        )


if __name__ == "__main__":
    unittest.main()
