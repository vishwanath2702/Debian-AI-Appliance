import unittest
from pathlib import Path


class OllamaBridgePackagingTests(unittest.TestCase):
    def test_bridge_helper_staged_executable(self):
        root = Path(__file__).resolve().parents[2]
        builder = (root / "build/build-payload.sh").read_text()
        self.assertIn(
            '"$DAIA_WORKSPACE/usr/libexec/daia-ollama-bridge.py"',
            builder,
        )
        self.assertIn(
            '"$DAIA_RUNTIME_SOURCE/daia-ollama-bridge.py"',
            builder,
        )
        self.assertIn(
            'Required Ollama bridge helper is missing or unsafe',
            builder,
        )

    def test_bridge_unit_staged_without_activation(self):
        root = Path(__file__).resolve().parents[2]
        builder = (root / "build/build-payload.sh").read_text()
        self.assertIn(
            '"$DAIA_SYSTEMD_TARGET/daia-ollama-bridge.service"',
            builder,
        )
        self.assertIn(
            'Ollama bridge systemd service staged without activation.',
            builder,
        )
        self.assertNotIn(
            'systemctl enable daia-ollama-bridge.service',
            builder,
        )
        self.assertNotIn(
            'systemctl start daia-ollama-bridge.service',
            builder,
        )


if __name__ == "__main__":
    unittest.main()
