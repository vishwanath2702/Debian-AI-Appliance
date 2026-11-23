"""Static fail-closed SearXNG recovery contract tests."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "daia-rag-recover"

class SearxngRecoveryWiringTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = SOURCE.read_text()
        cls.section = cls.source.split(
            "# SearXNG recovery is optional", 1
        )[1]

    def test_trusted_feature_gate(self):
        self.assertIn("searxng_config.enabled()", self.section)
        self.assertIn('case "$searxng_enabled_status"', self.section)
        self.assertIn('2) ;;', self.section)

    def test_registration_precedes_start(self):
        section = self.section
        self.assertLess(
            section.index("daia-searxng-verify-registration"),
            section.index('searxng_start_registered "$searxng_id"'),
        )

    def test_existing_running_container_not_restarted(self):
        self.assertIn("searxng_verify_network_contract", self.section)
        self.assertIn(r"true\|false\|false\|running", self.section)

    def test_readiness_required(self):
        self.assertIn(
            'searxng_wait_ready "$searxng_id" 60 || exit 1',
            self.section,
        )

    def test_no_config_source(self):
        self.assertNotIn('source "$config"', self.section)

    def test_no_unregistered_container_creation(self):
        self.assertNotIn("docker create", self.section)
        self.assertNotIn("docker network create", self.section)

if __name__ == "__main__":
    unittest.main()
