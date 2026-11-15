import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/openwebui.sh"


class GatewayIntegrationTests(unittest.TestCase):
    def test_gateway_helper_contract(self):
        source = MODULE.read_text()
        self.assertIn("openwebui_trusted_ollama_url() {", source)
        self.assertIn('python3 "$helper" --gateway', source)
        self.assertIn(
            'Configured Ollama URL conflicts with trusted gateway',
            source,
        )
        self.assertNotIn(
            "http://127.0.0.1:11434", source
        )

    def test_install_and_verify_use_same_discovery(self):
        source = MODULE.read_text()
        self.assertEqual(
            source.count('openwebui_trusted_ollama_url)" || return 1'),
            2,
        )
        self.assertIn('--env "OLLAMA_BASE_URL=${ollama_url}"', source)
        self.assertIn(
            '"OLLAMA_BASE_URL=${expected_ollama_url}"', source
        )

    def test_module_shell_syntax(self):
        result = subprocess.run(
            ["bash", "-n", str(MODULE)],
            capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
