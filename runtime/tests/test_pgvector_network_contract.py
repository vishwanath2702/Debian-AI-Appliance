"""Security contract for pgvector Docker network authorization."""

from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "runtime/daia-pgvector-authorize-start"


class PgvectorNetworkContractTests(unittest.TestCase):
    def test_authorization_inspects_network_mode(self):
        source = HELPER.read_text()
        self.assertIn(".HostConfig.NetworkMode", source)

    def test_authorization_rejects_unapproved_network(self):
        source = HELPER.read_text()
        self.assertIn("network", source.lower())
        self.assertIn("exit 1", source)


if __name__ == "__main__":
    unittest.main()
