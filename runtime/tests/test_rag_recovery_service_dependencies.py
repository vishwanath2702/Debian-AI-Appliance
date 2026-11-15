"""RAG proxy startup must depend on successful recovery."""
import configparser
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SERVICES = (
    "daia-ollama-bridge.service",
    "daia-openwebui-localhost.service",
)

class RecoveryDependencyTests(unittest.TestCase):
    def test_proxy_recovery_contract(self):
        for name in SERVICES:
            with self.subTest(service=name):
                text = (ROOT / "services" / name).read_text()
                requires = []
                after = []
                assertions = []
                for line in text.splitlines():
                    if line.startswith("Requires="):
                        requires.extend(line.partition("=")[2].split())
                    elif line.startswith("After="):
                        after.extend(line.partition("=")[2].split())
                    elif line.startswith("AssertPathExists="):
                        assertions.append(line.partition("=")[2])
                self.assertIn(
                    "daia-rag-recovery.service", requires
                )
                self.assertIn(
                    "daia-rag-recovery.service", after
                )
                self.assertEqual(
                    assertions,
                    ["/var/lib/daia/firstboot-complete"]
                )

    def test_recovery_is_not_automatically_restarted(self):
        unit = configparser.ConfigParser(interpolation=None)
        unit.read(ROOT / "services/daia-rag-recovery.service")
        self.assertEqual(unit["Service"]["Type"], "oneshot")
        self.assertNotIn("Restart", unit["Service"])

if __name__ == "__main__":
    unittest.main()
