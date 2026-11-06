"""Security contract for the pgvector registration verifier executable."""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "runtime/daia-pgvector-verify-registration"


class PgvectorRegistrationCliContractTests(unittest.TestCase):

    def test_executable_exists(self):
        self.assertTrue(HELPER.is_file())
        self.assertFalse(HELPER.is_symlink())

    def test_uses_fixed_registration_path(self):
        source = HELPER.read_text()
        self.assertIn("/var/lib/daia/pgvector", source)

    def test_uses_trusted_registration_verifier(self):
        source = HELPER.read_text()
        self.assertIn("verify_trusted_registration", source)

    def test_does_not_use_environment_path_override(self):
        source = HELPER.read_text()
        self.assertNotIn("os.environ", source)
        self.assertNotIn("os.getenv", source)
        self.assertNotIn("getenv(", source)

    def test_does_not_invoke_docker(self):
        source = HELPER.read_text()
        self.assertNotIn("subprocess", source)
        self.assertNotIn("docker ", source)


if __name__ == "__main__":
    unittest.main()
