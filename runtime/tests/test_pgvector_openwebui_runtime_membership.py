"""Fail-closed combined RAG network authorization contracts."""
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "runtime/daia-pgvector-authorize-start"

class CombinedNetworkTests(unittest.TestCase):
    def test_runtime_only_identity_gate(self):
        text = SOURCE.read_text()
        self.assertIn('"$auth_mode" != "runtime"', text)
        self.assertIn('"$2" =~ ^[0-9a-f]{64}$', text)

    def test_identity_verification_precedes_network(self):
        text = SOURCE.read_text()
        self.assertLess(
            text.index('--verify-identity "$webui_id"'),
            text.index('runtime "$container_id" "$webui_id"'),
        )

    def test_original_network_path_preserved(self):
        text = SOURCE.read_text()
        self.assertIn(
            '"$auth_mode" "$container_id" >/dev/null', text
        )

    def test_prestart_rejects_extra_identity(self):
        result = subprocess.run(
            ["bash", str(SOURCE), "prestart", "a" * 64],
            capture_output=True, text=True, timeout=10,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "Invalid Open WebUI runtime identity", result.stderr
        )

    def test_runtime_rejects_malformed_identity(self):
        result = subprocess.run(
            ["bash", str(SOURCE), "runtime", "invalid"],
            capture_output=True, text=True, timeout=10,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "Invalid Open WebUI runtime identity", result.stderr
        )

if __name__ == "__main__":
    unittest.main()
